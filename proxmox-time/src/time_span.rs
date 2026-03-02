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

use anyhow::Error;
use nom::{bytes::complete::take_while1, character::complete::space0, combinator::opt};

use crate::parse_helpers::{parse_complete_line, parse_error, parse_u64, IResult};

/// Enumerates the recognized time unit categories.
///
/// This is the single source of truth for unit classification; both the parser and the unit
/// validation flow through [`TimeUnit::classify`] which maps string aliases to these variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

impl TimeUnit {
    /// Maps a unit string (e.g. `"hours"`, `"m"`, `"µs"`) to its [`TimeUnit`].
    ///
    /// Returns [`None`] for unrecognized input.
    pub fn classify(s: &str) -> Option<Self> {
        match s {
            "seconds" | "second" | "sec" | "s" => Some(TimeUnit::Seconds),
            "msec" | "ms" => Some(TimeUnit::Milliseconds),
            "usec" | "us" | "µs" => Some(TimeUnit::Microseconds),
            "nsec" | "ns" => Some(TimeUnit::Nanoseconds),
            "minutes" | "minute" | "min" | "m" => Some(TimeUnit::Minutes),
            "hours" | "hour" | "hr" | "h" => Some(TimeUnit::Hours),
            "days" | "day" | "d" => Some(TimeUnit::Days),
            "weeks" | "week" | "w" => Some(TimeUnit::Weeks),
            "months" | "month" | "M" => Some(TimeUnit::Months),
            "years" | "year" | "y" => Some(TimeUnit::Years),
            _ => None,
        }
    }
}

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
        let seconds = self.seconds as f64
            + (self.msec as f64 / 1_000.0)
            + (self.usec as f64 / 1_000_000.0)
            + (self.nsec as f64 / 1_000_000_000.0);
        if seconds >= 0.1 {
            if !first {
                write!(f, " ")?;
            }
            let rounded = (seconds * 10.0).round() / 10.0;
            if rounded.fract().abs() < f64::EPSILON {
                write!(f, "{rounded:.0}s")?;
            } else {
                write!(f, "{rounded:.1}s")?;
            }
        } else if first {
            if seconds > 0.0 {
                write!(f, "<0.1s")?;
            } else {
                write!(f, "0s")?;
            }
        }
        Ok(())
    }
}

fn parse_time_unit(i: &str) -> IResult<&str, TimeUnit> {
    let (n, text) = take_while1(|c: char| char::is_ascii_alphabetic(&c) || c == 'µ')(i)?;
    match TimeUnit::classify(text) {
        Some(unit) => Ok((n, unit)),
        None => Err(parse_error(text, "time unit")),
    }
}

impl std::str::FromStr for TimeSpan {
    type Err = Error;

    fn from_str(i: &str) -> Result<Self, Self::Err> {
        parse_complete_line("time span", i, parse_time_span_incomplete)
    }
}

fn parse_time_span_incomplete(mut i: &str) -> IResult<&str, TimeSpan> {
    let mut ts = TimeSpan::default();
    let mut parsed_any = false;

    loop {
        i = space0(i)?.0;
        if i.is_empty() {
            break;
        }
        let (n, num) = parse_u64(i)?;
        i = space0(n)?.0;
        parsed_any = true;

        if let (n, Some(unit)) = opt(parse_time_unit)(i)? {
            i = n;
            match unit {
                TimeUnit::Seconds => ts.seconds += num,
                TimeUnit::Milliseconds => ts.msec += num,
                TimeUnit::Microseconds => ts.usec += num,
                TimeUnit::Nanoseconds => ts.nsec += num,
                TimeUnit::Minutes => ts.minutes += num,
                TimeUnit::Hours => ts.hours += num,
                TimeUnit::Days => ts.days += num,
                TimeUnit::Weeks => ts.weeks += num,
                TimeUnit::Months => ts.months += num,
                TimeUnit::Years => ts.years += num,
            }
        } else {
            ts.seconds += num;
        }
    }

    if !parsed_any {
        return Err(parse_error(i, "time span"));
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
    fn parse_equivalences() {
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
    }

    #[test]
    fn parse_all_unit_aliases() {
        let aliases = [
            "1seconds", "1second", "1sec", "1s", "1msec", "1ms", "1usec", "1us", "1µs", "1nsec",
            "1ns", "1minutes", "1minute", "1min", "1m", "1hours", "1hour", "1hr", "1h", "1days",
            "1day", "1d", "1weeks", "1week", "1w", "1months", "1month", "1M", "1years", "1year",
            "1y",
        ];
        for alias in aliases {
            assert!(
                TimeSpan::from_str(alias).is_ok(),
                "failed to parse alias: {alias}",
            );
        }
    }

    #[test]
    fn case_sensitivity_m_vs_big_m() {
        let months = TimeSpan::from_str("1M").unwrap();
        let minutes = TimeSpan::from_str("1m").unwrap();
        assert_eq!(months.months, 1);
        assert_eq!(months.minutes, 0);
        assert_eq!(minutes.minutes, 1);
        assert_eq!(minutes.months, 0);
        assert_ne!(months, minutes);
    }

    #[test]
    fn parse_rejects_empty_and_invalid() {
        assert!(TimeSpan::from_str("").is_err());
        assert!(TimeSpan::from_str("abc").is_err());
        assert!(TimeSpan::from_str("1x").is_err());
    }

    #[test]
    fn error_message_mentions_time_span() {
        let err = TimeSpan::from_str("not valid").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("time span"),
            "error should mention 'time span', got: {msg}",
        );
    }

    #[test]
    fn display_basic() {
        let ts = TimeSpan::from_str("1h 1m 3s").unwrap();
        assert_eq!(ts.to_string(), "1h 1m 3s");

        let ts_long = TimeSpan::from_str("1 hour 1 minute 3 second").unwrap();
        assert_eq!(ts.to_string(), ts_long.to_string());
    }

    #[test]
    fn display_zero() {
        assert_eq!(TimeSpan::default().to_string(), "0s");
    }

    #[test]
    fn display_very_small() {
        let ts = TimeSpan {
            nsec: 1,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "<0.1s");

        let ts = TimeSpan {
            usec: 50,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "<0.1s");

        let ts = TimeSpan {
            msec: 50,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "<0.1s");
    }

    #[test]
    fn display_subsecond_includes_usec_and_nsec() {
        let ts = TimeSpan {
            usec: 500_000,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "0.5s");

        let ts = TimeSpan {
            nsec: 100_000_000,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "0.1s");
    }

    #[test]
    fn display_subsecond_precision() {
        let ts = TimeSpan {
            msec: 500,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "0.5s");

        let ts = TimeSpan {
            seconds: 1,
            msec: 800,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "1.8s");
    }

    #[test]
    fn display_subsecond_rounds_to_whole_number_cleanly() {
        let ts = TimeSpan {
            msec: 999,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "1s");

        let ts = TimeSpan {
            seconds: 1,
            msec: 999,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "2s");
    }

    #[test]
    fn display_integer_seconds_no_decimal() {
        let ts = TimeSpan {
            seconds: 5,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "5s");

        let ts = TimeSpan {
            hours: 2,
            seconds: 10,
            ..Default::default()
        };
        assert_eq!(ts.to_string(), "2h 10s");
    }

    #[test]
    fn from_duration_round_trip() {
        let duration = std::time::Duration::new(32 * 24 * 60 * 60 + 3, 0);
        let ts = TimeSpan::from(duration);
        let ts_parsed = TimeSpan::from_str("1M 1d 3s").unwrap();
        assert_eq!(ts, ts_parsed);

        // minutes vs months
        assert_ne!(ts, TimeSpan::from_str("1m 1d 3s").unwrap());
    }
}
