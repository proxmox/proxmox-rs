use anyhow::Error;
use nom::{
    bytes::complete::tag, character::complete::space0, error::context, multi::separated_list1,
};

use crate::parse_helpers::{parse_complete_line, parse_error, parse_hm_time, IResult};
use crate::{parse_weekdays_range, WeekDays};

#[cfg(not(target_arch = "wasm32"))]
use crate::TmEditor;

/// Time of day as hour and minute.
///
/// Supports natural ordering (compares hour first, then minute).
///
/// # Examples
///
/// ```
/// use proxmox_time::HmTime;
///
/// let morning = HmTime { hour: 8, minute: 30 };
/// let evening = HmTime { hour: 20, minute: 0 };
/// assert!(morning < evening);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct HmTime {
    pub hour: u32,
    pub minute: u32,
}

/// A time-of-day window, optionally restricted to certain [`WeekDays`].
///
/// Specifies a half-open `[start, end)` time range. When `days` is empty or
/// [`WeekDays::all()`], the window applies to every day of the week.
///
/// # Examples
///
/// ```
/// use proxmox_time::{parse_daily_duration, WeekDays};
///
/// let d = parse_daily_duration("mon..fri 8:00-17:00").unwrap();
/// assert!(d.days.contains(WeekDays::MONDAY));
/// assert!(!d.days.contains(WeekDays::SATURDAY));
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DailyDuration {
    /// The weekdays this window applies to (empty means all days).
    pub days: WeekDays,
    /// Start of the window (inclusive).
    pub start: HmTime,
    /// End of the window (exclusive).
    pub end: HmTime,
}

#[cfg(not(target_arch = "wasm32"))]
impl DailyDuration {
    /// Test if the given epoch falls within this time window.
    pub fn time_match(&self, epoch: i64, utc: bool) -> Result<bool, Error> {
        let t = TmEditor::with_epoch(epoch, utc)?;

        Ok(self.time_match_with_tm_editor(&t))
    }

    /// Like [`time_match`](Self::time_match), but takes a [`TmEditor`] directly.
    ///
    /// Returns `false` if the time fields in `t` contain invalid values.
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

/// Parse a [`DailyDuration`] from a string.
///
/// The format is `[weekdays] start-end`, where times are `H:MM` or `H` and
/// weekdays are comma-separated names or `..` ranges (e.g. `mon..fri`).
///
/// # Examples
///
/// ```
/// use proxmox_time::parse_daily_duration;
///
/// // Time-only (applies to all days)
/// let d = parse_daily_duration("8:00-17:00").unwrap();
/// assert!(d.days.is_empty());
///
/// // With weekday constraint
/// let d = parse_daily_duration("sat,sun 10-14").unwrap();
/// assert!(!d.days.is_empty());
/// ```
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
                "start hour mismatch, expected {}, got {:?}",
                start_h,
                duration
            );
        }
        if duration.start.minute != start_m {
            bail!(
                "start minute mismatch, expected {}, got {:?}",
                start_m,
                duration
            );
        }
        if duration.end.hour != end_h {
            bail!("end hour mismatch, expected {}, got {:?}", end_h, duration);
        }
        if duration.end.minute != end_m {
            bail!(
                "end minute mismatch, expected {}, got {:?}",
                end_m,
                duration
            );
        }

        if duration.days != expected_days {
            bail!(
                "weekday mismatch, expected {:?}, got {:?}",
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
