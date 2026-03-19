//! Human readable time spans compatible with the systemd time span format.
//!
//! This module provides two related types:
//!
//! - [`TimeSpan`] — the primary type, storing a duration as total seconds and sub-second
//!   nanoseconds. Values are always normalized, so `parse("120s") == parse("2m")`. Use this type
//!   for storing, comparing, and passing around durations.
//! - [`TimeSpanParts`] — a decomposed 10-field view (years through nanoseconds), obtained via
//!   [`TimeSpan::parts()`] or constructible manually. Use this type when you need to inspect or
//!   build a duration field-by-field, e.g. for display formatting or constructing a span from
//!   individual components programmatically.
//!
//! `TimeSpan` converts infallibly into `TimeSpanParts` (via [`TimeSpan::parts()`] or
//! [`From<TimeSpan>`](From)). The reverse direction uses [`TryFrom<TimeSpanParts>`](TryFrom)
//! because arbitrary field values can overflow when summed into total seconds.
//!
//! Parts of this documentation have been adapted from the
//! [systemd.time](https://www.freedesktop.org/software/systemd/man/systemd.time.html) manual page.
//!
//! # Supported Time Units
//!
//! The following time units are understood when parsing:
//!
//! | Unit         | Aliases                         | Notes                                 |
//! |--------------|---------------------------------|---------------------------------------|
//! | Years        | `years`, `year`, `y`            | Defined as 365.25 days (see below)    |
//! | Months       | `months`, `month`, `M`          | Defined as 30.44 days (see below)     |
//! | Weeks        | `weeks`, `week`, `w`            |                                       |
//! | Days         | `days`, `day`, `d`              |                                       |
//! | Hours        | `hours`, `hour`, `hr`, `h`      |                                       |
//! | Minutes      | `minutes`, `minute`, `min`, `m` |                                       |
//! | Seconds      | `seconds`, `second`, `sec`, `s` |                                       |
//! | Milliseconds | `msec`, `ms`                    |                                       |
//! | Microseconds | `usec`, `us`, `µs`              |                                       |
//! | Nanoseconds  | `nsec`, `ns`                    | Not always accepted by `systemd.time` |
//!
//! # Warning: Approximate Units
//!
//! **Years and months use fixed approximations**, not calendar-aware definitions:
//!
//! - 1 year = **365.25 days** = 31,557,600 seconds
//! - 1 month = **30.44 days** = 2,630,016 seconds
//!
//! These are the same definitions used by systemd. They are useful for coarse human-readable
//! durations (e.g. retention policies, expiry times) but are **not suitable for calendar-accurate
//! date arithmetic**. In particular:
//!
//! - 12 months ≠ 1 year (12 × 30.44 days = 365.28 days, not 365.25), so `"12M"` displays as
//!   `"1y 43m 12s"`.
//! - These values do not account for leap years, varying month lengths, or time zones.
//!
//! If you need precise calendar offsets, use a calendar-aware library or
//! [`CalendarEvent`](crate::CalendarEvent) instead.
//!
//! # Display Format
//!
//! When displayed, time spans are formatted as a space-separated series of values, each suffixed by
//! its shortest unit identifier (e.g. `2h 30m`, `1w 2d 3s`). Zero-valued components are omitted.
//!
//! Sub-second components (milliseconds, microseconds, nanoseconds) are folded into the seconds
//! value and displayed as a decimal with up to one decimal place. Spans shorter than 0.1 seconds
//! are displayed as `<0.1s`. A completely zero span displays as `0s`.
//!
//! ## Precision-controlled sub-second display
//!
//! An explicit **fmt precision** (`{ts:.N}`) selects the finest sub-second unit to include.
//! Every 3 levels of precision introduces the next sub-second unit as a discrete integer field;
//! intermediate levels add decimal places to the finest unit shown:
//!
//! ```
//! # use proxmox_time::TimeSpan;
//! let ts = TimeSpan::from(std::time::Duration::new(1, 500_200_003));
//! assert_eq!(format!("{ts:.0}"), "1s");           // whole seconds
//! assert_eq!(format!("{ts:.1}"), "1.5s");         // 1 decimal place
//! assert_eq!(format!("{ts:.3}"), "1s 500ms");     // discrete milliseconds
//! assert_eq!(format!("{ts:.6}"), "1s 500ms 200µs"); // + discrete microseconds
//! assert_eq!(format!("{ts:.9}"), "1s 500ms 200µs 3ns"); // all sub-second units
//! ```
//!
//! ## Truncating at coarser units
//!
//! [`TimeSpan::display_down_to`] returns a wrapper that shows discrete integer units down to a
//! chosen [`TimeUnit`], omitting everything finer:
//!
//! ```
//! # use proxmox_time::{TimeSpan, TimeUnit};
//! let ts: TimeSpan = "1h 30m 45s".parse().unwrap();
//! assert_eq!(ts.display_down_to(TimeUnit::Minutes).to_string(), "1h 30m");
//! assert_eq!(ts.display_down_to(TimeUnit::Seconds).to_string(), "1h 30m 45s");
//! ```
//!
//! # Parsing
//!
//! When parsing a time span, all units listed above are accepted. Spaces between numeric values and
//! their units are optional, and the order of components does not matter.
//! A bare number without a unit suffix is interpreted as seconds.
//! Duplicate units are allowed and their values are summed.
//!
//! Parsed values are immediately normalized into total seconds and nanoseconds, so two
//! [`TimeSpan`] values compare equal whenever they represent the same total duration — for
//! example `parse("120s") == parse("2m")`.
//!
//! The following examples all represent the same duration of 1 day, 2 hours, and 3 minutes:
//!
//! - `1d 2h 3m`
//! - `1d2h3m`
//! - `26h 180s`
//! - `1 day 2 hours 3 minutes`
//! - `0y 0M 0w 1d 2h 3m`
//! - `2h 1d 3m`
//!
//! # Converting to and from [`std::time::Duration`]
//!
//! - [`From<std::time::Duration> for TimeSpan`] is a trivial conversion (both types store seconds
//!   and nanoseconds).
//! - [`From<TimeSpan> for std::time::Duration`] is likewise trivial and infallible.

use anyhow::{Context, Error};

use crate::parse_helpers::{parse_complete_line, parse_error, IResult};

// Seconds-per-unit constants. Month and year are the systemd definitions:
// 1 month = 30.44 days = 2,630,016 s (exact), 1 year = 365.25 days = 31,557,600 s (exact).
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 60 * SECS_PER_MINUTE;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_WEEK: u64 = 7 * SECS_PER_DAY;
const SECS_PER_MONTH: u64 = SECS_PER_DAY * 3044 / 100; // 30.44 days
const SECS_PER_YEAR: u64 = SECS_PER_DAY * 36525 / 100; // 365.25 days

/// Decomposes a `(total_seconds, sub_second_nanos)` pair into a [`TimeSpanParts`] with each field
/// in its natural range.
fn decompose(total_secs: u64, sub_nanos: u32) -> TimeSpanParts {
    debug_assert!(sub_nanos < 1_000_000_000);
    let mut rem = total_secs;
    let years = rem / SECS_PER_YEAR;
    rem %= SECS_PER_YEAR;
    let months = rem / SECS_PER_MONTH;
    rem %= SECS_PER_MONTH;
    let weeks = rem / SECS_PER_WEEK;
    rem %= SECS_PER_WEEK;
    let days = rem / SECS_PER_DAY;
    rem %= SECS_PER_DAY;
    let hours = rem / SECS_PER_HOUR;
    rem %= SECS_PER_HOUR;
    let minutes = rem / SECS_PER_MINUTE;
    let seconds = rem % SECS_PER_MINUTE;

    let mut ns = sub_nanos;
    let msec = (ns / 1_000_000) as u64;
    ns %= 1_000_000;
    let usec = (ns / 1_000) as u64;
    let nsec = (ns % 1_000) as u64;

    TimeSpanParts {
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

    /// Returns the shortest display suffix for this unit.
    pub fn suffix(self) -> &'static str {
        match self {
            TimeUnit::Years => "y",
            TimeUnit::Months => "M",
            TimeUnit::Weeks => "w",
            TimeUnit::Days => "d",
            TimeUnit::Hours => "h",
            TimeUnit::Minutes => "m",
            TimeUnit::Seconds => "s",
            TimeUnit::Milliseconds => "ms",
            TimeUnit::Microseconds => "µs",
            TimeUnit::Nanoseconds => "ns",
        }
    }

    /// Ordinal from coarsest (0 = Years) to finest (9 = Nanoseconds).
    fn ordinal(self) -> u8 {
        match self {
            TimeUnit::Years => 0,
            TimeUnit::Months => 1,
            TimeUnit::Weeks => 2,
            TimeUnit::Days => 3,
            TimeUnit::Hours => 4,
            TimeUnit::Minutes => 5,
            TimeUnit::Seconds => 6,
            TimeUnit::Milliseconds => 7,
            TimeUnit::Microseconds => 8,
            TimeUnit::Nanoseconds => 9,
        }
    }
}

/// A decomposed view of a time span with separate fields for each unit.
///
/// Obtained via [`TimeSpan::parts()`] (the canonical decomposition) or constructed manually.
/// Convert back to a [`TimeSpan`] via [`TryFrom<TimeSpanParts>`](TryFrom).
///
/// When obtained from [`TimeSpan::parts()`], each field is in its natural range (e.g.
/// `seconds` is 0-59, `minutes` is 0-59, etc.). When constructed manually, fields may hold
/// arbitrary values (e.g. `minutes: 120`).
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSpanParts {
    pub years: u64,
    pub months: u64,
    pub weeks: u64,
    pub days: u64,
    pub hours: u64,
    pub minutes: u64,
    pub seconds: u64,
    pub msec: u64,
    pub usec: u64,
    pub nsec: u64,
}

impl TimeSpanParts {
    /// Return all (value, unit) pairs from largest to smallest.
    fn unit_pairs(&self) -> [(u64, TimeUnit); 10] {
        [
            (self.years, TimeUnit::Years),
            (self.months, TimeUnit::Months),
            (self.weeks, TimeUnit::Weeks),
            (self.days, TimeUnit::Days),
            (self.hours, TimeUnit::Hours),
            (self.minutes, TimeUnit::Minutes),
            (self.seconds, TimeUnit::Seconds),
            (self.msec, TimeUnit::Milliseconds),
            (self.usec, TimeUnit::Microseconds),
            (self.nsec, TimeUnit::Nanoseconds),
        ]
    }
}

/// A time span representing a duration of time.
///
/// Internally stores total seconds and sub-second nanoseconds (always `< 1_000_000_000`).
/// Values are always normalized, so two `TimeSpan` values compare equal whenever they represent
/// the same total duration — `parse("120s") == parse("2m")`.
///
/// Use [`parts()`](TimeSpan::parts) to obtain the decomposed [`TimeSpanParts`] view.
///
/// See the [module documentation](self) for supported units, display formatting options, and
/// parsing details.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
/// # use proxmox_time::TimeSpan;
///
/// let ts = TimeSpan::from_str("1h 30m").unwrap();
/// assert_eq!(ts.hours(), 1);
/// assert_eq!(ts.minutes(), 30);
/// assert_eq!(ts.to_string(), "1h 30m");
/// ```
///
/// ```
/// # use proxmox_time::TimeSpan;
/// let dur = std::time::Duration::from_secs(90);
/// let ts = TimeSpan::from(dur);
/// assert_eq!(ts.minutes(), 1);
/// assert_eq!(ts.seconds(), 30);
/// ```
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSpan {
    secs: u64,
    nanos: u32, // always < 1_000_000_000
}

impl TimeSpan {
    /// Returns the total number of whole seconds in this time span.
    pub fn as_secs(&self) -> u64 {
        self.secs
    }

    /// Returns the sub-second nanosecond component (always `< 1_000_000_000`).
    pub fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// Decomposes this time span into its constituent [`TimeSpanParts`].
    ///
    /// The returned parts are in their natural ranges (e.g. `seconds` is 0-59).
    pub fn parts(&self) -> TimeSpanParts {
        decompose(self.secs, self.nanos)
    }

    /// Returns the years component of the decomposed time span.
    pub fn years(&self) -> u64 {
        self.parts().years
    }

    /// Returns the months component of the decomposed time span.
    pub fn months(&self) -> u64 {
        self.parts().months
    }

    /// Returns the weeks component of the decomposed time span.
    pub fn weeks(&self) -> u64 {
        self.parts().weeks
    }

    /// Returns the days component of the decomposed time span.
    pub fn days(&self) -> u64 {
        self.parts().days
    }

    /// Returns the hours component of the decomposed time span.
    pub fn hours(&self) -> u64 {
        self.parts().hours
    }

    /// Returns the minutes component of the decomposed time span.
    pub fn minutes(&self) -> u64 {
        self.parts().minutes
    }

    /// Returns the seconds component of the decomposed time span (0-59).
    pub fn seconds(&self) -> u64 {
        self.parts().seconds
    }

    /// Returns the milliseconds component of the decomposed time span (0-999).
    pub fn msec(&self) -> u64 {
        self.parts().msec
    }

    /// Returns the microseconds component of the decomposed time span (0-999).
    pub fn usec(&self) -> u64 {
        self.parts().usec
    }

    /// Returns the nanoseconds component of the decomposed time span (0-999).
    pub fn nsec(&self) -> u64 {
        self.parts().nsec
    }

    /// Returns `Ok(self)` — `TimeSpan` is always normalized.
    ///
    /// This method is retained for API compatibility. Since `TimeSpan` now stores a normalized
    /// `(secs, nanos)` pair, normalization is implicit and this is a no-op.
    pub fn normalize(self) -> Result<Self, Error> {
        Ok(self)
    }

    /// Returns a [`Display`](std::fmt::Display) wrapper that formats with discrete integer units
    /// down to `smallest`, omitting zero-valued components.
    ///
    /// Unlike the default `Display` (which folds sub-second parts into a decimal seconds value),
    /// this shows each unit separately with its own suffix.
    ///
    /// # Examples
    ///
    /// ```
    /// # use proxmox_time::{TimeSpan, TimeUnit};
    /// let ts: TimeSpan = "1h 30m 45s".parse().unwrap();
    /// assert_eq!(ts.display_down_to(TimeUnit::Minutes).to_string(), "1h 30m");
    ///
    /// let ts = TimeSpan::from(std::time::Duration::new(5, 500_200_003));
    /// assert_eq!(ts.display_down_to(TimeUnit::Milliseconds).to_string(), "5s 500ms");
    /// assert_eq!(ts.display_down_to(TimeUnit::Nanoseconds).to_string(), "5s 500ms 200µs 3ns");
    /// ```
    pub fn display_down_to(self, smallest: TimeUnit) -> DisplayTimeSpan {
        DisplayTimeSpan { ts: self, smallest }
    }

    /// Return a display wrapper that shows `count` contiguous units starting from the largest
    /// non-zero one, filling gaps with zeros for readability.
    ///
    /// Useful for human-friendly summaries like "2d 4h" or "1y 3M".
    ///
    /// # Examples
    ///
    /// ```
    /// # use proxmox_time::TimeSpan;
    /// let ts: TimeSpan = "1y 3months 2d 4h 30min".parse().unwrap();
    /// assert_eq!(ts.display_top(2).to_string(), "1y 3M");
    /// assert_eq!(ts.display_top(3).to_string(), "1y 3M 0w");
    ///
    /// let ts: TimeSpan = "1d 2s".parse().unwrap();
    /// assert_eq!(ts.display_top(2).to_string(), "1d 0h");
    /// assert_eq!(ts.display_top(3).to_string(), "1d 0h 0m");
    /// ```
    pub fn display_top(self, count: usize) -> DisplayTopUnits {
        DisplayTopUnits { ts: self, count }
    }
}

/// Wrapper for displaying a [`TimeSpan`] showing the N largest contiguous units starting from the
/// first non-zero one. Obtained via [`TimeSpan::display_top`].
pub struct DisplayTopUnits {
    ts: TimeSpan,
    count: usize,
}

impl std::fmt::Display for DisplayTopUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut remaining = self.count;
        let mut started = false;

        for (val, unit) in self.ts.parts().unit_pairs() {
            if remaining == 0 {
                break;
            }
            if started {
                write!(f, " {val}{}", unit.suffix())?;
                remaining -= 1;
            } else if val > 0 {
                write!(f, "{val}{}", unit.suffix())?;
                remaining -= 1;
                started = true;
            }
        }

        if !started {
            write!(f, "0s")?;
        }

        Ok(())
    }
}

/// Wrapper for displaying a [`TimeSpan`] with discrete integer units down to a specified
/// smallest unit. Obtained via [`TimeSpan::display_down_to`].
pub struct DisplayTimeSpan {
    ts: TimeSpan,
    smallest: TimeUnit,
}

impl std::fmt::Display for DisplayTimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let limit = self.smallest.ordinal();
        let mut first = true;

        for (val, unit) in self.ts.parts().unit_pairs() {
            if unit.ordinal() > limit {
                break;
            }
            if val > 0 {
                if !first {
                    write!(f, " ")?;
                }
                first = false;
                write!(f, "{val}{}", unit.suffix())?;
            }
        }

        if first {
            write!(f, "0{}", self.smallest.suffix())?;
        }

        Ok(())
    }
}

/// Converts a [`TimeSpanParts`] into a [`TimeSpan`] by summing all fields into total seconds and
/// nanoseconds using checked arithmetic.
///
/// # Errors
///
/// Returns an error if the total seconds would overflow [`u64::MAX`].
///
/// # Examples
///
/// ```
/// # use proxmox_time::{TimeSpan, TimeSpanParts};
/// let parts = TimeSpanParts { hours: 1, minutes: 30, ..Default::default() };
/// let ts = TimeSpan::try_from(parts).unwrap();
/// assert_eq!(ts.as_secs(), 5400);
/// ```
impl TryFrom<TimeSpanParts> for TimeSpan {
    type Error = Error;

    fn try_from(parts: TimeSpanParts) -> Result<Self, Error> {
        // Sub-second fields can hold arbitrary u64 values; extract whole-second
        // carry and sub-second remainder from each.
        let (carry_ms, rem_ms) = (parts.msec / 1_000, (parts.msec % 1_000) as u32);
        let (carry_us, rem_us) = (parts.usec / 1_000_000, (parts.usec % 1_000_000) as u32);
        let (carry_ns, rem_ns) = (
            parts.nsec / 1_000_000_000,
            (parts.nsec % 1_000_000_000) as u32,
        );

        // Combine sub-second remainders. Maximum possible value:
        //   999 * 1_000_000 + 999_999 * 1_000 + 999_999_999 = 2_999_998_999
        // This fits in u32 (max 4_294_967_295) and yields at most 2 extra seconds.
        let combined_sub: u32 = rem_ms * 1_000_000 + rem_us * 1_000 + rem_ns;
        let extra_carry = (combined_sub / 1_000_000_000) as u64;
        let nanos = combined_sub % 1_000_000_000;

        let secs = [
            parts.years.checked_mul(SECS_PER_YEAR),
            parts.months.checked_mul(SECS_PER_MONTH),
            parts.weeks.checked_mul(SECS_PER_WEEK),
            parts.days.checked_mul(SECS_PER_DAY),
            parts.hours.checked_mul(SECS_PER_HOUR),
            parts.minutes.checked_mul(SECS_PER_MINUTE),
            Some(parts.seconds),
            Some(carry_ms),
            Some(carry_us),
            Some(carry_ns),
            Some(extra_carry),
        ]
        .into_iter()
        .try_fold(0u64, |acc, v| acc.checked_add(v?))
        .context("time span too large")?;

        Ok(TimeSpan { secs, nanos })
    }
}

/// Decomposes a [`TimeSpan`] into its constituent [`TimeSpanParts`].
///
/// This is equivalent to calling [`TimeSpan::parts()`].
impl From<TimeSpan> for TimeSpanParts {
    fn from(ts: TimeSpan) -> Self {
        ts.parts()
    }
}

/// Converts a [`TimeSpan`] reference to its total duration in **seconds** as an [`f64`].
impl From<&TimeSpan> for f64 {
    fn from(ts: &TimeSpan) -> Self {
        ts.secs as f64 + ts.nanos as f64 / 1_000_000_000.0
    }
}

/// Converts a [`TimeSpan`] to its total duration in **seconds** as an [`f64`].
impl From<TimeSpan> for f64 {
    fn from(ts: TimeSpan) -> Self {
        Self::from(&ts)
    }
}

/// Converts a [`TimeSpan`] reference to a [`std::time::Duration`].
impl From<&TimeSpan> for std::time::Duration {
    fn from(ts: &TimeSpan) -> Self {
        std::time::Duration::new(ts.secs, ts.nanos)
    }
}

/// Converts a [`TimeSpan`] to a [`std::time::Duration`].
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
/// # use proxmox_time::TimeSpan;
///
/// let ts = TimeSpan::from_str("1h 30m").unwrap();
/// let dur: std::time::Duration = ts.into();
/// assert_eq!(dur.as_secs(), 5400);
/// ```
impl From<TimeSpan> for std::time::Duration {
    fn from(ts: TimeSpan) -> Self {
        Self::from(&ts)
    }
}

/// Creates a [`TimeSpan`] from a [`std::time::Duration`].
///
/// # Examples
///
/// ```
/// # use proxmox_time::TimeSpan;
/// let dur = std::time::Duration::from_secs(2_630_016 + 86_400 + 3);
/// let ts = TimeSpan::from(dur);
/// assert_eq!(ts.months(), 1);
/// assert_eq!(ts.days(), 1);
/// assert_eq!(ts.seconds(), 3);
/// assert_eq!(ts.to_string(), "1M 1d 3s");
/// ```
impl From<std::time::Duration> for TimeSpan {
    fn from(duration: std::time::Duration) -> Self {
        TimeSpan {
            secs: duration.as_secs(),
            nanos: duration.subsec_nanos(),
        }
    }
}

/// Formats the seconds-and-below components of a [`TimeSpanParts`] with explicit precision.
///
/// Precision follows a tiered scheme where every 3 levels introduces the next sub-second unit,
/// and intermediate levels add decimal places to the finest unit.
fn fmt_precise_subseconds(
    f: &mut std::fmt::Formatter<'_>,
    p: &TimeSpanParts,
    nanos: u32,
    precision: usize,
    first: &mut bool,
) -> std::fmt::Result {
    let tier = (precision / 3).min(3);
    let dp = if tier < 3 { precision % 3 } else { 0 };

    macro_rules! sep {
        () => {
            if !*first {
                write!(f, " ")?;
            }
            *first = false;
        };
    }

    // Tier 0: seconds only.
    if tier == 0 {
        if dp == 0 {
            if p.seconds > 0 {
                sep!();
                write!(f, "{}s", p.seconds)?;
            }
        } else {
            let secs_f = p.seconds as f64 + nanos as f64 / 1_000_000_000.0;
            if secs_f > 0.0 {
                sep!();
                write!(f, "{val:.prec$}s", val = secs_f, prec = dp)?;
            }
        }
        return Ok(());
    }

    // Tier >= 1: seconds as integer, then sub-second units.
    if p.seconds > 0 {
        sep!();
        write!(f, "{}s", p.seconds)?;
    }

    let ms = (nanos / 1_000_000) as u64;
    let ms_rem = nanos % 1_000_000;

    if tier == 1 {
        if dp == 0 {
            if ms > 0 {
                sep!();
                write!(f, "{ms}ms")?;
            }
        } else {
            let ms_f = ms as f64 + ms_rem as f64 / 1_000_000.0;
            if ms_f > 0.0 {
                sep!();
                write!(f, "{val:.prec$}ms", val = ms_f, prec = dp)?;
            }
        }
        return Ok(());
    }

    // Tier >= 2: ms as integer.
    if ms > 0 {
        sep!();
        write!(f, "{ms}ms")?;
    }

    let us = (ms_rem / 1_000) as u64;
    let us_rem = ms_rem % 1_000;

    if tier == 2 {
        if dp == 0 {
            if us > 0 {
                sep!();
                write!(f, "{us}µs")?;
            }
        } else {
            let us_f = us as f64 + us_rem as f64 / 1_000.0;
            if us_f > 0.0 {
                sep!();
                write!(f, "{val:.prec$}µs", val = us_f, prec = dp)?;
            }
        }
        return Ok(());
    }

    // Tier 3: all discrete integers.
    if us > 0 {
        sep!();
        write!(f, "{us}µs")?;
    }

    let ns = us_rem as u64;
    if ns > 0 {
        sep!();
        write!(f, "{ns}ns")?;
    }

    Ok(())
}

impl std::fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let p = self.parts();
        let mut first = true;
        {
            let mut do_write = |v: u64, unit: &str| -> Result<(), std::fmt::Error> {
                if !first {
                    write!(f, " ")?;
                }
                first = false;
                write!(f, "{v}{unit}")
            };
            if p.years > 0 {
                do_write(p.years, "y")?;
            }
            if p.months > 0 {
                do_write(p.months, "M")?;
            }
            if p.weeks > 0 {
                do_write(p.weeks, "w")?;
            }
            if p.days > 0 {
                do_write(p.days, "d")?;
            }
            if p.hours > 0 {
                do_write(p.hours, "h")?;
            }
            if p.minutes > 0 {
                do_write(p.minutes, "m")?;
            }
        }
        match f.precision() {
            None => {
                // Default: fold sub-second into decimal seconds with 1dp.
                let seconds = p.seconds as f64
                    + (p.msec as f64 / 1_000.0)
                    + (p.usec as f64 / 1_000_000.0)
                    + (p.nsec as f64 / 1_000_000_000.0);
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
            }
            Some(precision) => {
                fmt_precise_subseconds(f, &p, self.nanos, precision, &mut first)?;
                if first {
                    // Nothing written at all — show a zero appropriate for the precision.
                    if (1..=2).contains(&precision) {
                        write!(f, "{val:.prec$}s", val = 0.0, prec = precision)?;
                    } else {
                        write!(f, "0s")?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn parse_time_unit(i: &str) -> IResult<&str, TimeUnit> {
    let end = i
        .find(|c: char| !c.is_ascii_alphabetic() && c != 'µ')
        .unwrap_or(i.len());
    if end == 0 {
        return Err(parse_error(i, "time unit"));
    }
    let (text, rest) = i.split_at(end);
    match TimeUnit::classify(text) {
        Some(unit) => Ok((rest, unit)),
        None => Err(parse_error(text, "time unit")),
    }
}

impl std::str::FromStr for TimeSpan {
    type Err = Error;

    fn from_str(i: &str) -> Result<Self, Self::Err> {
        let parts: TimeSpanParts = parse_complete_line("time span", i, parse_time_span_incomplete)?;
        Self::try_from(parts)
    }
}

fn parse_time_span_incomplete(mut i: &str) -> IResult<&str, TimeSpanParts> {
    let mut parts = TimeSpanParts::default();
    let mut parsed_any = false;

    loop {
        i = i.trim_start();
        if i.is_empty() {
            break;
        }
        let (n, num) = nom::character::complete::u64(i)?;
        i = n.trim_start();
        parsed_any = true;

        match parse_time_unit(i) {
            Ok((n, unit)) => {
                i = n;
                match unit {
                    TimeUnit::Seconds => parts.seconds += num,
                    TimeUnit::Milliseconds => parts.msec += num,
                    TimeUnit::Microseconds => parts.usec += num,
                    TimeUnit::Nanoseconds => parts.nsec += num,
                    TimeUnit::Minutes => parts.minutes += num,
                    TimeUnit::Hours => parts.hours += num,
                    TimeUnit::Days => parts.days += num,
                    TimeUnit::Weeks => parts.weeks += num,
                    TimeUnit::Months => parts.months += num,
                    TimeUnit::Years => parts.years += num,
                }
            }
            Err(_) => parts.seconds += num,
        }
    }

    if !parsed_any {
        return Err(parse_error(i, "time span"));
    }

    Ok((i, parts))
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
    use std::time::Duration;

    /// Helper to build a TimeSpan from parts, panicking on overflow.
    fn ts_from_parts(parts: TimeSpanParts) -> TimeSpan {
        TimeSpan::try_from(parts).unwrap()
    }

    #[test]
    fn parse_equivalences() {
        let ts1 = TimeSpan::from_str("1h 1m 3s").unwrap();
        let ts2 = TimeSpan::from_str("1 hour 1 minute 3 second").unwrap();
        let ts3 = TimeSpan::from_str("0y 0M 0w 1h 1m 3s").unwrap();
        let ts4 = TimeSpan::from_str("1h1m3s").unwrap();
        let ts5 = TimeSpan::from_str("3s 1m 1h").unwrap();
        assert_eq!(ts1, ts2);
        assert_eq!(ts1, ts3);
        assert_eq!(ts1, ts4);
        assert_eq!(ts1, ts5);
    }

    #[test]
    fn equality_by_total_duration() {
        assert_eq!(
            TimeSpan::from_str("120s").unwrap(),
            TimeSpan::from_str("2m").unwrap(),
        );
        assert_eq!(
            TimeSpan::from_str("3663s").unwrap(),
            TimeSpan::from_str("1h 1m 3s").unwrap(),
        );
        assert_eq!(
            TimeSpan::from_str("90m").unwrap(),
            TimeSpan::from_str("1h 30m").unwrap(),
        );
    }

    #[test]
    fn bare_number_parsed_as_seconds() {
        let ts = TimeSpan::from_str("300").unwrap();
        assert_eq!(ts.as_secs(), 300);
        assert_eq!(ts.minutes(), 5);
        assert_eq!(ts.seconds(), 0);
    }

    #[test]
    fn duplicate_units_are_summed() {
        let ts = TimeSpan::from_str("1h 1h").unwrap();
        assert_eq!(ts, TimeSpan::from_str("2h").unwrap());

        let ts = TimeSpan::from_str("30s 30s").unwrap();
        assert_eq!(ts, TimeSpan::from_str("1m").unwrap());
    }

    #[test]
    fn case_sensitivity_m_vs_big_m() {
        let months = TimeSpan::from_str("1M").unwrap();
        let minutes = TimeSpan::from_str("1m").unwrap();
        assert_eq!(months.months(), 1);
        assert_eq!(months.minutes(), 0);
        assert_eq!(minutes.minutes(), 1);
        assert_eq!(minutes.months(), 0);
        assert_ne!(months, minutes);
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
        let ts = ts_from_parts(TimeSpanParts {
            nsec: 1,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "<0.1s");

        let ts = ts_from_parts(TimeSpanParts {
            usec: 50,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "<0.1s");

        let ts = ts_from_parts(TimeSpanParts {
            msec: 50,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "<0.1s");
    }

    #[test]
    fn display_subsecond_includes_usec_and_nsec() {
        let ts = ts_from_parts(TimeSpanParts {
            usec: 500_000,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "0.5s");

        let ts = ts_from_parts(TimeSpanParts {
            nsec: 100_000_000,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "0.1s");
    }

    #[test]
    fn display_subsecond_precision() {
        let ts = ts_from_parts(TimeSpanParts {
            msec: 500,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "0.5s");

        let ts = ts_from_parts(TimeSpanParts {
            seconds: 1,
            msec: 800,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "1.8s");

        let ts = ts_from_parts(TimeSpanParts {
            seconds: 1,
            msec: 200,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "1.2s");
    }

    #[test]
    fn display_subsecond_rounds_to_whole_number_cleanly() {
        let ts = ts_from_parts(TimeSpanParts {
            msec: 999,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "1s");

        let ts = ts_from_parts(TimeSpanParts {
            seconds: 1,
            msec: 999,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "2s");
    }

    #[test]
    fn display_integer_seconds_no_decimal() {
        let ts = ts_from_parts(TimeSpanParts {
            seconds: 5,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "5s");

        let ts = ts_from_parts(TimeSpanParts {
            hours: 2,
            seconds: 10,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "2h 10s");
    }

    #[test]
    fn display_large_units() {
        let ts = ts_from_parts(TimeSpanParts {
            years: 1,
            months: 2,
            weeks: 3,
            days: 4,
            ..Default::default()
        });
        assert_eq!(ts.to_string(), "1y 2M 3w 4d");
    }

    #[test]
    fn try_from_parts_basic() {
        let parts = TimeSpanParts {
            minutes: 1,
            seconds: 30,
            ..Default::default()
        };
        let ts = TimeSpan::try_from(parts).unwrap();
        assert_eq!(ts.as_secs(), 90);
        assert_eq!(ts.subsec_nanos(), 0);
    }

    #[test]
    fn try_from_parts_subsecond() {
        let parts = TimeSpanParts {
            msec: 1,
            usec: 2,
            nsec: 3,
            ..Default::default()
        };
        let ts = TimeSpan::try_from(parts).unwrap();
        assert_eq!(ts.as_secs(), 0);
        assert_eq!(ts.subsec_nanos(), 1_002_003);
    }

    #[test]
    fn try_from_parts_subsecond_carry() {
        // 1500ms = 1 second + 500ms
        let parts = TimeSpanParts {
            msec: 1500,
            ..Default::default()
        };
        let ts = TimeSpan::try_from(parts).unwrap();
        assert_eq!(ts.as_secs(), 1);
        assert_eq!(ts.subsec_nanos(), 500_000_000);
    }

    #[test]
    fn try_from_parts_overflow() {
        let parts = TimeSpanParts {
            years: u64::MAX,
            ..Default::default()
        };
        assert!(TimeSpan::try_from(parts).is_err());
    }

    #[test]
    fn parts_round_trip() {
        let ts = TimeSpan::from_str("1y 2M 3w 4d 5h 6m 7s").unwrap();
        let parts = ts.parts();
        assert_eq!(parts.years, 1);
        assert_eq!(parts.months, 2);
        assert_eq!(parts.weeks, 3);
        assert_eq!(parts.days, 4);
        assert_eq!(parts.hours, 5);
        assert_eq!(parts.minutes, 6);
        assert_eq!(parts.seconds, 7);

        // Round-trip: parts -> TimeSpan -> parts should be identity.
        let ts2 = TimeSpan::try_from(parts).unwrap();
        assert_eq!(ts, ts2);
    }

    #[test]
    fn parts_from_trait() {
        let ts = TimeSpan::from_str("2h 30m").unwrap();
        let parts: TimeSpanParts = ts.into();
        assert_eq!(parts.hours, 2);
        assert_eq!(parts.minutes, 30);
    }

    #[test]
    fn from_duration_exact_month_boundary() {
        let dur = Duration::from_secs(SECS_PER_MONTH + SECS_PER_DAY + 3);
        let ts = TimeSpan::from(dur);
        assert_eq!(ts.months(), 1);
        assert_eq!(ts.days(), 1);
        assert_eq!(ts.seconds(), 3);
        assert_eq!(ts.to_string(), "1M 1d 3s");
    }

    #[test]
    fn from_duration_preserves_sub_day_remainder() {
        let total_secs = 32 * SECS_PER_DAY;
        let dur = Duration::from_secs(total_secs);
        let ts = TimeSpan::from(dur);
        assert_eq!(ts.months(), 1);

        // Verify round-trip is exact.
        let dur_back: Duration = ts.into();
        assert_eq!(dur, dur_back);
    }

    #[test]
    fn from_duration_subsecond() {
        let dur = Duration::new(3, 4_005_006);
        let ts = TimeSpan::from(dur);
        assert_eq!(ts.seconds(), 3);
        assert_eq!(ts.msec(), 4);
        assert_eq!(ts.usec(), 5);
        assert_eq!(ts.nsec(), 6);
    }

    #[test]
    fn from_duration_zero() {
        let ts = TimeSpan::from(Duration::ZERO);
        assert_eq!(ts, TimeSpan::default());
    }

    #[test]
    fn from_duration_max() {
        let ts = TimeSpan::from(Duration::MAX);
        assert!(ts.years() > 0);
    }

    #[test]
    fn into_duration_round_trip() {
        let ts1 = TimeSpan::from_str("1h 30m 15s").unwrap();
        let dur: Duration = ts1.into();
        assert_eq!(dur.as_secs(), 5415);
        let ts2 = TimeSpan::from(dur);
        assert_eq!(ts1, ts2);
    }

    #[test]
    fn into_duration_from_ref() {
        let ts = TimeSpan::from_str("2m 30s").unwrap();
        let dur = Duration::from(&ts);
        assert_eq!(dur.as_secs(), 150);
    }

    #[test]
    fn into_duration_subsecond() {
        let ts = ts_from_parts(TimeSpanParts {
            seconds: 1,
            msec: 500,
            ..Default::default()
        });
        let dur: Duration = ts.into();
        assert_eq!(dur.as_millis(), 1500);
    }

    #[test]
    fn to_f64_total_seconds() {
        let ts = TimeSpan::from_str("1h 30m").unwrap();
        let total: f64 = (&ts).into();
        assert!((total - 5400.0).abs() < f64::EPSILON);

        let ts = TimeSpan::from_str("500ms").unwrap();
        let total: f64 = (&ts).into();
        assert!((total - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn to_f64_owned() {
        let ts = TimeSpan::from_str("1m").unwrap();
        let total: f64 = ts.into();
        assert!((total - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normalize_is_noop() {
        let ts = TimeSpan::from_str("90m").unwrap();
        let n = ts.normalize().unwrap();
        assert_eq!(ts, n);
        assert_eq!(n.hours(), 1);
        assert_eq!(n.minutes(), 30);

        let ts = TimeSpan::from_str("3600s").unwrap();
        let n = ts.normalize().unwrap();
        assert_eq!(ts, n);
        assert_eq!(n.hours(), 1);

        assert_eq!(
            TimeSpan::default().normalize().unwrap(),
            TimeSpan::default()
        );
    }

    #[test]
    fn normalize_idempotent() {
        let ts = TimeSpan::from_str("90m 3700ms").unwrap();
        let n1 = ts.normalize().unwrap();
        let n2 = n1.normalize().unwrap();
        assert_eq!(n1, n2);
    }

    #[test]
    fn twelve_months_normalization_note() {
        // 12 months != 1 year because 12 * 30.44 days != 365.25 days.
        let ts = TimeSpan::from_str("12M").unwrap();
        assert_eq!(ts.to_string(), "1y 43m 12s");
    }

    #[test]
    fn verify_valid() {
        assert!(verify_time_span("1h 30m").is_ok());
        assert!(verify_time_span("0s").is_ok());
        assert!(verify_time_span("1").is_ok());
    }

    #[test]
    fn verify_invalid() {
        assert!(verify_time_span("invalid").is_err());
        assert!(verify_time_span("1x").is_err());
    }

    #[test]
    fn display_precision() {
        let ts = TimeSpan::from(Duration::new(1, 500_200_003));

        // Tier 0: whole seconds / decimal seconds
        assert_eq!(format!("{ts:.0}"), "1s");
        assert_eq!(format!("{ts:.1}"), "1.5s");
        assert_eq!(format!("{ts:.2}"), "1.50s");

        // Tier 1: seconds + ms (integer or decimal)
        assert_eq!(format!("{ts:.3}"), "1s 500ms");
        assert_eq!(format!("{ts:.4}"), "1s 500.2ms");
        assert_eq!(format!("{ts:.5}"), "1s 500.20ms");

        // Tier 2: seconds + ms + µs (integer or decimal)
        assert_eq!(format!("{ts:.6}"), "1s 500ms 200µs");
        assert_eq!(format!("{ts:.7}"), "1s 500ms 200.0µs");
        assert_eq!(format!("{ts:.8}"), "1s 500ms 200.00µs");

        // Tier 3: all discrete sub-second units
        assert_eq!(format!("{ts:.9}"), "1s 500ms 200µs 3ns");

        // Zero span
        assert_eq!(format!("{:.0}", TimeSpan::default()), "0s");
        assert_eq!(format!("{:.1}", TimeSpan::default()), "0.0s");
        assert_eq!(format!("{:.2}", TimeSpan::default()), "0.00s");

        // Sub-second only
        let ts = TimeSpan::from_str("500ms").unwrap();
        assert_eq!(format!("{ts:.0}"), "0s");
        assert_eq!(format!("{ts:.1}"), "0.5s");
        assert_eq!(format!("{ts:.3}"), "500ms");

        // Big units unaffected by precision
        let ts = TimeSpan::from_str("1y 2M 3w 4d 5h 6m 7s").unwrap();
        assert_eq!(format!("{ts:.0}"), "1y 2M 3w 4d 5h 6m 7s");
        assert_eq!(format!("{ts:.3}"), "1y 2M 3w 4d 5h 6m 7s");
    }

    #[test]
    fn display_down_to() {
        let ts = TimeSpan::from_str("1h 30m 45s").unwrap();
        assert_eq!(ts.display_down_to(TimeUnit::Hours).to_string(), "1h");
        assert_eq!(ts.display_down_to(TimeUnit::Minutes).to_string(), "1h 30m");
        assert_eq!(
            ts.display_down_to(TimeUnit::Seconds).to_string(),
            "1h 30m 45s"
        );

        // Sub-second units
        let ts = TimeSpan::from(Duration::new(5, 500_200_003));
        assert_eq!(
            ts.display_down_to(TimeUnit::Milliseconds).to_string(),
            "5s 500ms"
        );
        assert_eq!(
            ts.display_down_to(TimeUnit::Microseconds).to_string(),
            "5s 500ms 200µs"
        );
        assert_eq!(
            ts.display_down_to(TimeUnit::Nanoseconds).to_string(),
            "5s 500ms 200µs 3ns"
        );

        // Zero span shows zero in the chosen unit
        let ts = TimeSpan::default();
        assert_eq!(ts.display_down_to(TimeUnit::Seconds).to_string(), "0s");
        assert_eq!(
            ts.display_down_to(TimeUnit::Milliseconds).to_string(),
            "0ms"
        );
        assert_eq!(ts.display_down_to(TimeUnit::Nanoseconds).to_string(), "0ns");

        // Zero-valued intermediate fields are omitted
        let ts = TimeSpan::from_str("1h 3s").unwrap();
        assert_eq!(ts.display_down_to(TimeUnit::Seconds).to_string(), "1h 3s");
    }
}
