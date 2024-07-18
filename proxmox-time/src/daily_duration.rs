use std::cmp::{Ordering, PartialOrd};

use anyhow::Error;
use nom::{
    bytes::complete::tag, character::complete::space0, error::context, multi::separated_list1,
};

use crate::parse_helpers::{parse_complete_line, parse_error, parse_hm_time, IResult};
use crate::{parse_weekdays_range, WeekDays};

#[cfg(not(target_arch = "wasm32"))]
use crate::TmEditor;

/// Time of Day (hour with minute)
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HmTime {
    pub hour: u32,
    pub minute: u32,
}

impl PartialOrd for HmTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut order = self.hour.cmp(&other.hour);
        if order == Ordering::Equal {
            order = self.minute.cmp(&other.minute);
        }
        Some(order)
    }
}

/// Defines a period of time for on or more [WeekDays]
#[derive(Default, Clone, Debug, PartialEq)]
pub struct DailyDuration {
    /// the days in a week this duration should trigger
    pub days: WeekDays,
    pub start: HmTime,
    pub end: HmTime,
}

#[cfg(not(target_arch = "wasm32"))]
impl DailyDuration {
    /// Test it time is within this frame
    pub fn time_match(&self, epoch: i64, utc: bool) -> Result<bool, Error> {
        let t = TmEditor::with_epoch(epoch, utc)?;

        Ok(self.time_match_with_tm_editor(&t))
    }

    /// Like time_match, but use [TmEditor] to specify the time
    ///
    /// Note: This function returns bool (not Result<bool, Error>). It
    /// simply returns ''false' if passed time 't' contains invalid values.
    pub fn time_match_with_tm_editor(&self, t: &TmEditor) -> bool {
        let all_days = self.days.is_empty() || self.days.is_all();

        if !all_days {
            // match day first
            match u32::try_from(t.day_num()) {
                Ok(day_num) => match WeekDays::from_bits(1 << day_num) {
                    Some(day) => {
                        if !self.days.contains(day) {
                            return false;
                        }
                    }
                    None => return false,
                },
                Err(_) => return false,
            }
        }

        let hour = t.hour().try_into();
        let minute = t.min().try_into();

        match (hour, minute) {
            (Ok(hour), Ok(minute)) => {
                let ctime = HmTime { hour, minute };
                ctime >= self.start && ctime < self.end
            }
            _ => false,
        }
    }
}

/// Parse a [DailyDuration]
pub fn parse_daily_duration(i: &str) -> Result<DailyDuration, Error> {
    parse_complete_line("daily duration", i, parse_daily_duration_incomplete)
}

fn parse_daily_duration_incomplete(mut i: &str) -> IResult<&str, DailyDuration> {
    let mut duration = DailyDuration::default();

    if i.starts_with(|c: char| char::is_ascii_alphabetic(&c)) {
        let (n, range_list) = context(
            "weekday range list",
            separated_list1(tag(","), parse_weekdays_range),
        )(i)?;

        i = space0(n)?.0;

        for range in range_list {
            duration.days.insert(range);
        }
    }

    let (i, start) = parse_hm_time(i)?;

    let i = space0(i)?.0;

    let (i, _) = tag("-")(i)?;

    let i = space0(i)?.0;

    let end_time_start = i;

    let (i, end) = parse_hm_time(i)?;

    if start > end {
        return Err(parse_error(end_time_start, "end time before start time"));
    }

    duration.start = start;
    duration.end = end;

    Ok((i, duration))
}

#[cfg(test)]
mod test {

    use anyhow::{bail, Error};

    use super::*;

    fn test_parse(
        duration_str: &str,
        start_h: u32,
        start_m: u32,
        end_h: u32,
        end_m: u32,
        days: &[usize],
    ) -> Result<(), Error> {
        let mut day_bits = 0;
        for day in days {
            day_bits |= 1 << day;
        }
        let expected_days = WeekDays::from_bits(day_bits).unwrap();

        let duration = parse_daily_duration(duration_str)?;

        if duration.start.hour != start_h {
            bail!(
                "start hour mismatch, extected {}, got {:?}",
                start_h,
                duration
            );
        }
        if duration.start.minute != start_m {
            bail!(
                "start minute mismatch, extected {}, got {:?}",
                start_m,
                duration
            );
        }
        if duration.end.hour != end_h {
            bail!("end hour mismatch, extected {}, got {:?}", end_h, duration);
        }
        if duration.end.minute != end_m {
            bail!(
                "end minute mismatch, extected {}, got {:?}",
                end_m,
                duration
            );
        }

        if duration.days != expected_days {
            bail!(
                "weekday mismatch, extected {:?}, got {:?}",
                expected_days,
                duration
            );
        }

        Ok(())
    }

    const fn make_test_time(mday: i32, hour: i32, min: i32) -> i64 {
        (mday * 3600 * 24 + hour * 3600 + min * 60) as i64
    }

    #[test]
    fn test_daily_duration_parser() -> Result<(), Error> {
        assert!(parse_daily_duration("").is_err());
        assert!(parse_daily_duration(" 8-12").is_err());
        assert!(parse_daily_duration("8:60-12").is_err());
        assert!(parse_daily_duration("8-25").is_err());
        assert!(parse_daily_duration("12-8").is_err());

        test_parse("8-12", 8, 0, 12, 0, &[])?;
        test_parse("8:0-12:0", 8, 0, 12, 0, &[])?;
        test_parse("8:00-12:00", 8, 0, 12, 0, &[])?;
        test_parse("8:05-12:20", 8, 5, 12, 20, &[])?;
        test_parse("8:05 - 12:20", 8, 5, 12, 20, &[])?;

        test_parse("mon 8-12", 8, 0, 12, 0, &[0])?;
        test_parse("tue..fri 8-12", 8, 0, 12, 0, &[1, 2, 3, 4])?;
        test_parse("sat,tue..thu,fri 8-12", 8, 0, 12, 0, &[1, 2, 3, 4, 5])?;

        Ok(())
    }

    #[test]
    fn test_time_match() -> Result<(), Error> {
        const THURSDAY_80_00: i64 = make_test_time(0, 8, 0);
        const THURSDAY_12_00: i64 = make_test_time(0, 12, 0);
        const DAY: i64 = 3600 * 24;

        let duration = parse_daily_duration("thu..fri 8:05-12")?;

        assert!(!duration.time_match(THURSDAY_80_00, true)?);
        assert!(!duration.time_match(THURSDAY_80_00 + DAY, true)?);
        assert!(!duration.time_match(THURSDAY_80_00 + 2 * DAY, true)?);

        assert!(duration.time_match(THURSDAY_80_00 + 5 * 60, true)?);
        assert!(duration.time_match(THURSDAY_80_00 + 5 * 60 + DAY, true)?);
        assert!(!duration.time_match(THURSDAY_80_00 + 5 * 60 + 2 * DAY, true)?);

        assert!(duration.time_match(THURSDAY_12_00 - 1, true)?);
        assert!(duration.time_match(THURSDAY_12_00 - 1 + DAY, true)?);
        assert!(!duration.time_match(THURSDAY_12_00 - 1 + 2 * DAY, true)?);

        assert!(!duration.time_match(THURSDAY_12_00, true)?);
        assert!(!duration.time_match(THURSDAY_12_00 + DAY, true)?);
        assert!(!duration.time_match(THURSDAY_12_00 + 2 * DAY, true)?);

        Ok(())
    }
}
