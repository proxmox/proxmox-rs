//! Timespans that try to be compatible with the systemd time span format.
//!
//! Time spans refer to time durations, like [std::time::Duration] but in the format that is
//! targeting human interfaces and that systemd understands. Parts of this documentation have been
//! adapted from the systemd.time manual page.
//!
//! The following time units are understood:
//! - `nsec`, `ns` (not always accepted by systemd.time)
//! - `usec`, `us`, `µs`
//! - `msec`, `ms`
//! - `seconds`, `second`, `sec`, `s`
//! - `minutes`, `minute`, `min`, `m`
//! - `hours`, `hour`, `hr`, `h`
//! - `days`, `day`, `d`
//! - `weeks`, `week`, `w`
//! - `months`, `month`, `M` (defined as 30.44 days)
//! - `years`, `year`, `y` (defined as 365.25 days)
//!
//! On display, time spans are formatted as space-separated series of time values each
//! suffixed by the single-character time unit, for example
//!
//! - `2h 30m`
//! - `1w 2m 3s`
//!
//! For parsing a time stamp the same syntax applies, all time units from above are supported, and
//! spaces between units and/or values can be added or omitted. The order of the time values does
//! not matter.
//!
//! The following examples are all representing the exact same time span of 1 day 2 hours and 3
//! minutes:
//!
//! - `1d 2h 3m`
//! - `1d2h3m`
//! - `26h 180s`
//! - `1 day 2 hours 3 minutes`
//! - `0y 0M 0w 1d 2h 3m`
//! - `2h 1d 3m`
//!
//! This module also supports transforming a [std::time::Duration] to a [TimeSpan].

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::Error;
use nom::{bytes::complete::take_while1, character::complete::space0, combinator::opt};

use crate::parse_helpers::{parse_complete_line, parse_error, parse_u64, IResult};

static TIME_SPAN_UNITS: LazyLock<HashMap<&'static str, f64>> = LazyLock::new(|| {
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
});

/// A time spans defines a time duration
#[derive(Default, Clone, Debug, PartialEq)]
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

impl From<&TimeSpan> for f64 {
    fn from(ts: &TimeSpan) -> Self {
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

impl From<TimeSpan> for f64 {
    fn from(ts: TimeSpan) -> Self {
        Self::from(&ts)
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
                write!(f, "{v}{unit}")
            };
            if self.years > 0 {
                do_write(self.years, "y")?;
            }
            if self.months > 0 {
                do_write(self.months, "M")?;
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
                do_write(self.minutes, "m")?;
            }
        }
        let seconds = self.seconds as f64 + (self.msec as f64 / 1000.0);
        if seconds >= 0.1 {
            if !first {
                write!(f, " ")?;
            }
            if seconds >= 1.0 || !first {
                write!(f, "{seconds:.0}s")?;
            } else {
                write!(f, "{seconds:.1}s")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn conversions() {
        let ts1 = TimeSpan::from_str("1h 1m 3s").unwrap();
        let ts2 = TimeSpan::from_str("1 hour 1 minute 3 second").unwrap();
        let ts3 = TimeSpan::from_str("0y 0M 0w 1h 1m 3s").unwrap();
        let ts4 = TimeSpan::from_str("1h1m3s").unwrap();
        let ts5 = TimeSpan::from_str("3s 1m 1h").unwrap();
        // TODO: below fails to compare equal to above as we do not normalize on parse, while that
        // might be OK it would be at least nice to provide a normalize method for TimeSpan (e.g.
        // by converting the time stamp to a Duration and then back again using from<Duration>
        //let ts6 = TimeSpan::from_str("3663s").unwrap();
        assert_eq!(ts1, ts2);
        assert_eq!(ts1, ts3);
        assert_eq!(ts1, ts4);
        assert_eq!(ts1, ts5);

        let display_ts1 = format!("{ts1}");
        let display_ts2 = format!("{ts2}");
        assert_eq!(display_ts1, display_ts2);
        assert_eq!(String::from("1h 1m 3s"), display_ts2);

        let duration1 = std::time::Duration::new(32 * 24 * 60 * 60 + 3, 0);
        let ts1 = TimeSpan::from(duration1);
        let ts2 = TimeSpan::from_str("1M 1d 3s").unwrap();
        let ts3 = TimeSpan::from_str("1m 1d 3s").unwrap();
        assert_eq!(ts1, ts2);
        assert_ne!(ts1, ts3);
    }
}
