use std::collections::HashMap;

use anyhow::Error;
use lazy_static::lazy_static;
use nom::{bytes::complete::take_while1, character::complete::space0, combinator::opt};

use crate::parse_helpers::{parse_complete_line, parse_error, parse_u64, IResult};

lazy_static! {
    static ref TIME_SPAN_UNITS: HashMap<&'static str, f64> = {
        let mut map = HashMap::new();

        let second = 1.0;

        map.insert("seconds", second);
        map.insert("second", second);
        map.insert("sec", second);
        map.insert("s", second);

        let msec = second / 1000.0;

        map.insert("msec", msec);
        map.insert("ms", msec);

        let usec = msec / 1000.0;

        map.insert("usec", usec);
        map.insert("us", usec);
        map.insert("µs", usec);

        let nsec = usec / 1000.0;

        map.insert("nsec", nsec);
        map.insert("ns", nsec);

        let minute = second * 60.0;

        map.insert("minutes", minute);
        map.insert("minute", minute);
        map.insert("min", minute);
        map.insert("m", minute);

        let hour = minute * 60.0;

        map.insert("hours", hour);
        map.insert("hour", hour);
        map.insert("hr", hour);
        map.insert("h", hour);

        let day = hour * 24.0;

        map.insert("days", day);
        map.insert("day", day);
        map.insert("d", day);

        let week = day * 7.0;

        map.insert("weeks", week);
        map.insert("week", week);
        map.insert("w", week);

        let month = 30.44 * day;

        map.insert("months", month);
        map.insert("month", month);
        map.insert("M", month);

        let year = 365.25 * day;

        map.insert("years", year);
        map.insert("year", year);
        map.insert("y", year);

        map
    };
}

/// A time spans defines a time duration
#[derive(Default, Clone, Debug)]
pub struct TimeSpan {
    pub nsec: u64,
    pub usec: u64,
    pub msec: u64,
    pub seconds: u64,
    pub minutes: u64,
    pub hours: u64,
    pub days: u64,
    pub weeks: u64,
    pub months: u64,
    pub years: u64,
}

impl From<TimeSpan> for f64 {
    fn from(ts: TimeSpan) -> Self {
        (ts.seconds as f64)
            + ((ts.nsec as f64) / 1_000_000_000.0)
            + ((ts.usec as f64) / 1_000_000.0)
            + ((ts.msec as f64) / 1_000.0)
            + ((ts.minutes as f64) * 60.0)
            + ((ts.hours as f64) * 3600.0)
            + ((ts.days as f64) * 3600.0 * 24.0)
            + ((ts.weeks as f64) * 3600.0 * 24.0 * 7.0)
            + ((ts.months as f64) * 3600.0 * 24.0 * 30.44)
            + ((ts.years as f64) * 3600.0 * 24.0 * 365.25)
    }
}

impl From<std::time::Duration> for TimeSpan {
    fn from(duration: std::time::Duration) -> Self {
        let mut duration = duration.as_nanos();
        let nsec = (duration % 1000) as u64;
        duration /= 1000;
        let usec = (duration % 1000) as u64;
        duration /= 1000;
        let msec = (duration % 1000) as u64;
        duration /= 1000;
        let seconds = (duration % 60) as u64;
        duration /= 60;
        let minutes = (duration % 60) as u64;
        duration /= 60;
        let hours = (duration % 24) as u64;
        duration /= 24;
        let years = (duration as f64 / 365.25) as u64;
        let ydays = (duration as f64 % 365.25) as u64;
        let months = (ydays as f64 / 30.44) as u64;
        let mdays = (ydays as f64 % 30.44) as u64;
        let weeks = mdays / 7;
        let days = mdays % 7;
        Self {
            nsec,
            usec,
            msec,
            seconds,
            minutes,
            hours,
            days,
            weeks,
            months,
            years,
        }
    }
}

impl std::fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut first = true;
        {
            // block scope for mutable borrows
            let mut do_write = |v: u64, unit: &str| -> Result<(), std::fmt::Error> {
                if !first {
                    write!(f, " ")?;
                }
                first = false;
                write!(f, "{}{}", v, unit)
            };
            if self.years > 0 {
                do_write(self.years, "y")?;
            }
            if self.months > 0 {
                do_write(self.months, "m")?;
            }
            if self.weeks > 0 {
                do_write(self.weeks, "w")?;
            }
            if self.days > 0 {
                do_write(self.days, "d")?;
            }
            if self.hours > 0 {
                do_write(self.hours, "h")?;
            }
            if self.minutes > 0 {
                do_write(self.minutes, "min")?;
            }
        }
        if !first {
            write!(f, " ")?;
        }
        let seconds = self.seconds as f64 + (self.msec as f64 / 1000.0);
        if seconds >= 0.1 {
            if seconds >= 1.0 || !first {
                write!(f, "{:.0}s", seconds)?;
            } else {
                write!(f, "{:.1}s", seconds)?;
            }
        } else if first {
            write!(f, "<0.1s")?;
        }
        Ok(())
    }
}

fn parse_time_unit(i: &str) -> IResult<&str, &str> {
    let (n, text) = take_while1(|c: char| char::is_ascii_alphabetic(&c) || c == 'µ')(i)?;
    if TIME_SPAN_UNITS.contains_key(&text) {
        Ok((n, text))
    } else {
        Err(parse_error(text, "time unit"))
    }
}

impl std::str::FromStr for TimeSpan {
    type Err = Error;

    fn from_str(i: &str) -> Result<Self, Self::Err> {
        parse_complete_line("calendar event", i, parse_time_span_incomplete)
    }
}

/// Parse a [TimeSpan]
#[deprecated="Use std::str::FromStr trait instead."]
pub fn parse_time_span(i: &str) -> Result<TimeSpan, Error> {
    i.parse()
}

fn parse_time_span_incomplete(mut i: &str) -> IResult<&str, TimeSpan> {
    let mut ts = TimeSpan::default();

    loop {
        i = space0(i)?.0;
        if i.is_empty() {
            break;
        }
        let (n, num) = parse_u64(i)?;
        i = space0(n)?.0;

        if let (n, Some(unit)) = opt(parse_time_unit)(i)? {
            i = n;
            match unit {
                "seconds" | "second" | "sec" | "s" => {
                    ts.seconds += num;
                }
                "msec" | "ms" => {
                    ts.msec += num;
                }
                "usec" | "us" | "µs" => {
                    ts.usec += num;
                }
                "nsec" | "ns" => {
                    ts.nsec += num;
                }
                "minutes" | "minute" | "min" | "m" => {
                    ts.minutes += num;
                }
                "hours" | "hour" | "hr" | "h" => {
                    ts.hours += num;
                }
                "days" | "day" | "d" => {
                    ts.days += num;
                }
                "weeks" | "week" | "w" => {
                    ts.weeks += num;
                }
                "months" | "month" | "M" => {
                    ts.months += num;
                }
                "years" | "year" | "y" => {
                    ts.years += num;
                }
                _ => return Err(parse_error(unit, "internal error")),
            }
        } else {
            ts.seconds += num;
        }
    }

    Ok((i, ts))
}

/// Verify the format of the [TimeSpan]
pub fn verify_time_span(i: &str) -> Result<(), Error> {
    let _: TimeSpan = i.parse()?;
    Ok(())
}

