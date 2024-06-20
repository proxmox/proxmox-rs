use anyhow::Error;
use nom::{
    bytes::complete::tag,
    character::complete::space0,
    combinator::opt,
    error::context,
    multi::separated_list1,
    sequence::{preceded, terminated, tuple},
};

use crate::date_time_value::DateTimeValue;
use crate::parse_helpers::{parse_complete_line, parse_error, parse_time_comp, IResult};
use crate::{parse_weekdays_range, WeekDays};

/// Calendar events may be used to refer to one or more points in time in a
/// single expression. They are designed after the systemd.time Calendar Events
/// specification, but are not guaranteed to be 100% compatible.
#[derive(Default, Clone, Debug)]
pub struct CalendarEvent {
    /// if true, the event is calculated in utc and the local timezone otherwise
    utc: bool,
    /// the days in a week this event should trigger
    pub(crate) days: WeekDays,
    /// the second(s) this event should trigger
    pub(crate) second: Vec<DateTimeValue>, // todo: support float values
    /// the minute(s) this event should trigger
    pub(crate) minute: Vec<DateTimeValue>,
    /// the hour(s) this event should trigger
    pub(crate) hour: Vec<DateTimeValue>,
    /// the day(s) in a month this event should trigger
    pub(crate) day: Vec<DateTimeValue>,
    /// the month(s) in a year this event should trigger
    pub(crate) month: Vec<DateTimeValue>,
    /// the years(s) this event should trigger
    pub(crate) year: Vec<DateTimeValue>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CalendarEvent {
    /// Computes the next timestamp after `last`. If `utc` is false, the local
    /// timezone will be used for the calculation.
    pub fn compute_next_event(&self, last: i64) -> Result<Option<i64>, Error> {
        let last = last + 1; // at least one second later

        let all_days = self.days.is_empty() || self.days.is_all();

        let mut t = crate::TmEditor::with_epoch(last, self.utc)?;

        let mut count = 0;

        loop {
            // cancel after 1000 loops
            if count > 1000 {
                return Ok(None);
            } else {
                count += 1;
            }

            if !self.year.is_empty() {
                let year: u32 = t.year().try_into()?;
                if !DateTimeValue::list_contains(&self.year, year) {
                    if let Some(n) = DateTimeValue::find_next(&self.year, year) {
                        t.add_years((n - year).try_into()?)?;
                        continue;
                    } else {
                        // if we have no valid year, we cannot find a correct timestamp
                        return Ok(None);
                    }
                }
            }

            if !self.month.is_empty() {
                let month: u32 = t.month().try_into()?;
                if !DateTimeValue::list_contains(&self.month, month) {
                    if let Some(n) = DateTimeValue::find_next(&self.month, month) {
                        t.add_months((n - month).try_into()?)?;
                    } else {
                        // if we could not find valid month, retry next year
                        t.add_years(1)?;
                    }
                    continue;
                }
            }

            if !self.day.is_empty() {
                let day: u32 = t.day().try_into()?;
                if !DateTimeValue::list_contains(&self.day, day) {
                    if let Some(n) = DateTimeValue::find_next(&self.day, day) {
                        t.add_days((n - day).try_into()?)?;
                    } else {
                        // if we could not find valid mday, retry next month
                        t.add_months(1)?;
                    }
                    continue;
                }
            }

            if !all_days {
                // match day first
                let day_num: u32 = t.day_num().try_into()?;
                let day = WeekDays::from_bits(1 << day_num).unwrap();
                if !self.days.contains(day) {
                    if let Some(n) = ((day_num + 1)..7)
                        .find(|d| self.days.contains(WeekDays::from_bits(1 << d).unwrap()))
                    {
                        // try next day
                        t.add_days((n - day_num).try_into()?)?;
                    } else {
                        // try next week
                        t.add_days((7 - day_num).try_into()?)?;
                    }
                    continue;
                }
            }

            // this day
            if !self.hour.is_empty() {
                let hour = t.hour().try_into()?;
                if !DateTimeValue::list_contains(&self.hour, hour) {
                    if let Some(n) = DateTimeValue::find_next(&self.hour, hour) {
                        // test next hour
                        t.set_time(n.try_into()?, 0, 0)?;
                    } else {
                        // test next day
                        t.add_days(1)?;
                    }
                    continue;
                }
            }

            // this hour
            if !self.minute.is_empty() {
                let minute = t.min().try_into()?;
                if !DateTimeValue::list_contains(&self.minute, minute) {
                    if let Some(n) = DateTimeValue::find_next(&self.minute, minute) {
                        // test next minute
                        t.set_min_sec(n.try_into()?, 0)?;
                    } else {
                        // test next hour
                        t.set_time(t.hour() + 1, 0, 0)?;
                    }
                    continue;
                }
            }

            // this minute
            if !self.second.is_empty() {
                let second = t.sec().try_into()?;
                if !DateTimeValue::list_contains(&self.second, second) {
                    if let Some(n) = DateTimeValue::find_next(&self.second, second) {
                        // test next second
                        t.set_sec(n.try_into()?)?;
                    } else {
                        // test next min
                        t.set_min_sec(t.min() + 1, 0)?;
                    }
                    continue;
                }
            }

            let next = t.into_epoch()?;
            return Ok(Some(next));
        }
    }
}

impl std::str::FromStr for CalendarEvent {
    type Err = Error;

    fn from_str(i: &str) -> Result<Self, Self::Err> {
        parse_complete_line("calendar event", i, parse_calendar_event_incomplete)
    }
}

/// Verify the format of the [CalendarEvent]
pub fn verify_calendar_event(i: &str) -> Result<(), Error> {
    let _: CalendarEvent = i.parse()?;
    Ok(())
}

fn parse_calendar_event_incomplete(mut i: &str) -> IResult<&str, CalendarEvent> {
    let mut has_dayspec = false;
    let mut has_timespec = false;
    let mut has_datespec = false;

    let mut event = CalendarEvent::default();
    if let Some(n) = i.strip_suffix("UTC") {
        event.utc = true;
        i = n.trim_end_matches(' ');
    }

    if i.starts_with(|c: char| char::is_ascii_alphabetic(&c)) {
        match i {
            "minutely" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        second: vec![DateTimeValue::Single(0)],
                        ..Default::default()
                    },
                ));
            }
            "hourly" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        ..Default::default()
                    },
                ));
            }
            "daily" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        ..Default::default()
                    },
                ));
            }
            "weekly" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        days: WeekDays::MONDAY,
                        ..Default::default()
                    },
                ));
            }
            "monthly" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        day: vec![DateTimeValue::Single(1)],
                        ..Default::default()
                    },
                ));
            }
            "yearly" | "annually" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        day: vec![DateTimeValue::Single(1)],
                        month: vec![DateTimeValue::Single(1)],
                        ..Default::default()
                    },
                ));
            }
            "quarterly" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        day: vec![DateTimeValue::Single(1)],
                        month: vec![
                            DateTimeValue::Single(1),
                            DateTimeValue::Single(4),
                            DateTimeValue::Single(7),
                            DateTimeValue::Single(10),
                        ],
                        ..Default::default()
                    },
                ));
            }
            "semiannually" | "semi-annually" => {
                return Ok((
                    "",
                    CalendarEvent {
                        utc: event.utc,
                        hour: vec![DateTimeValue::Single(0)],
                        minute: vec![DateTimeValue::Single(0)],
                        second: vec![DateTimeValue::Single(0)],
                        day: vec![DateTimeValue::Single(1)],
                        month: vec![DateTimeValue::Single(1), DateTimeValue::Single(7)],
                        ..Default::default()
                    },
                ));
            }
            _ => { /* continue */ }
        }

        let (n, range_list) = context(
            "weekday range list",
            separated_list1(tag(","), parse_weekdays_range),
        )(i)?;

        has_dayspec = true;

        i = space0(n)?.0;

        for range in range_list {
            event.days.insert(range);
        }
    }

    if let (n, Some(date)) = opt(parse_date_spec)(i)? {
        event.year = date.year;
        event.month = date.month;
        event.day = date.day;
        has_datespec = true;
        i = space0(n)?.0;
    }

    if let (n, Some(time)) = opt(parse_time_spec)(i)? {
        event.hour = time.hour;
        event.minute = time.minute;
        event.second = time.second;
        has_timespec = true;
        i = n;
    } else {
        event.hour = vec![DateTimeValue::Single(0)];
        event.minute = vec![DateTimeValue::Single(0)];
        event.second = vec![DateTimeValue::Single(0)];
    }

    if !(has_dayspec || has_timespec || has_datespec) {
        return Err(parse_error(i, "date or time specification"));
    }

    Ok((i, event))
}

struct TimeSpec {
    hour: Vec<DateTimeValue>,
    minute: Vec<DateTimeValue>,
    second: Vec<DateTimeValue>,
}

struct DateSpec {
    year: Vec<DateTimeValue>,
    month: Vec<DateTimeValue>,
    day: Vec<DateTimeValue>,
}

fn parse_date_time_comp(max: usize) -> impl Fn(&str) -> IResult<&str, DateTimeValue> {
    move |i: &str| {
        let (i, value) = parse_time_comp(max)(i)?;

        if let (i, Some(end)) = opt(preceded(tag(".."), parse_time_comp(max)))(i)? {
            if value > end {
                return Err(parse_error(i, "range start is bigger than end"));
            }
            if let Some(time) = i.strip_prefix('/') {
                let (time, repeat) = parse_time_comp(max)(time)?;
                return Ok((time, DateTimeValue::Repeated(value, repeat, Some(end))));
            }
            return Ok((i, DateTimeValue::Range(value, end)));
        }

        if let Some(time) = i.strip_prefix('/') {
            let (time, repeat) = parse_time_comp(max)(time)?;
            Ok((time, DateTimeValue::Repeated(value, repeat, None)))
        } else {
            Ok((i, DateTimeValue::Single(value)))
        }
    }
}

fn parse_date_time_comp_list(
    start: u32,
    max: usize,
) -> impl Fn(&str) -> IResult<&str, Vec<DateTimeValue>> {
    move |i: &str| {
        if let Some(rest) = i.strip_prefix('*') {
            if let Some(time) = rest.strip_prefix('/') {
                let (n, repeat) = parse_time_comp(max)(time)?;
                if repeat > 0 {
                    return Ok((n, vec![DateTimeValue::Repeated(start, repeat, None)]));
                }
            }
            return Ok((rest, Vec::new()));
        }

        separated_list1(tag(","), parse_date_time_comp(max))(i)
    }
}

fn parse_time_spec(i: &str) -> IResult<&str, TimeSpec> {
    let (i, (opt_hour, minute, opt_second)) = tuple((
        opt(terminated(parse_date_time_comp_list(0, 24), tag(":"))),
        parse_date_time_comp_list(0, 60),
        opt(preceded(tag(":"), parse_date_time_comp_list(0, 60))),
    ))(i)?;

    let hour = opt_hour.unwrap_or_default();
    let second = opt_second.unwrap_or_else(|| vec![DateTimeValue::Single(0)]);

    Ok((
        i,
        TimeSpec {
            hour,
            minute,
            second,
        },
    ))
}

fn parse_date_spec(i: &str) -> IResult<&str, DateSpec> {
    // TODO: implement ~ for days (man systemd.time)
    if let Ok((i, (year, month, day))) = tuple((
        parse_date_time_comp_list(0, 2200), // the upper limit for systemd, stay compatible
        preceded(tag("-"), parse_date_time_comp_list(1, 13)),
        preceded(tag("-"), parse_date_time_comp_list(1, 32)),
    ))(i)
    {
        Ok((i, DateSpec { year, month, day }))
    } else if let Ok((i, (month, day))) = tuple((
        parse_date_time_comp_list(1, 13),
        preceded(tag("-"), parse_date_time_comp_list(1, 32)),
    ))(i)
    {
        Ok((
            i,
            DateSpec {
                year: Vec::new(),
                month,
                day,
            },
        ))
    } else {
        Err(parse_error(i, "invalid date spec"))
    }
}
