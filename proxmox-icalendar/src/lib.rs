//! A minimal library for generating ICS (iCalendar) files following RFC5545.
//!
//! This crate provides a simple, dependency-light way to generate `.ics` calendar files
//! that can be served over HTTP or imported into calendar applications.
//!
//! # Features
//!
//! - `jiff` - Enable integration with the [jiff](https://docs.rs/jiff) datetime library
//!   for timezone-aware datetime handling
//! - `serde` - Enable serialization/deserialization support
//!
//! # Quick Start
//!
//! ```
//! use simple_ics::{Calendar, Event, DateTime};
//!
//! let event = Event::new(&quot;meeting-123&quot;, &quot;Coffee with Anna&quot;)
//!     .start(DateTime::new(2026, 1, 15, 10, 0, 0))
//!     .end(DateTime::new(2026, 1, 15, 11, 0, 0))
//!     .location(&quot;Café Central, Vienna&quot;)
//!     .description(&quot;Discussing the new project proposal&quot;);
//!
//! let calendar = Calendar::new(&quot;My Schedule&quot;)
//!     .timezone(&quot;Europe/Vienna&quot;)
//!     .event(event);
//!
//! let ics_content = calendar.to_ics();
//! // Serve ics_content over HTTP with content-type: text/calendar
//! ```
//!
//! # With Jiff Feature
//!
//! When the `jiff` feature is enabled, you can use timezone-aware datetimes:
//!
//! ```ignore
//! use simple_ics::{Calendar, Event};
//! use jiff::Zoned;
//!
//! let start: Zoned = &quot;2026-01-15T10:00[Europe/Vienna]&quot;.parse().unwrap();
//! let end: Zoned = &quot;2026-01-15T11:00[Europe/Vienna]&quot;.parse().unwrap();
//!
//! let event = Event::new(&quot;meeting-123&quot;, &quot;Coffee with Anna&quot;)
//!     .start_zoned(start)
//!     .end_zoned(end)
//!     .location(&quot;Café Central, Vienna&quot;);
//!
//! let calendar = Calendar::new(&quot;My Schedule&quot;).event(event);
//! ```

use std::fmt::{self, Display, Write};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use thiserror::Error;

/// Errors that can occur when working with ICS data.
#[derive(Error, Debug)]
pub enum IcsError {
    /// The provided date or time values are invalid.
    #[error("Invalid date/time: {0}")]
    InvalidDateTime(String),

    /// A required field is missing from an event or calendar.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// A field contains an invalid value.
    #[error("Invalid value for {field}: {reason}")]
    InvalidValue {
        /// The name of the field with the invalid value.
        field: String,
        /// The reason the value is invalid.
        reason: String,
    },

    /// An error occurred while formatting the ICS output.
    #[error("Formatting error: {0}")]
    FormatError(#[from] fmt::Error),

    /// An error from the jiff datetime library (only with `jiff` feature).
    #[cfg(feature = "jiff")]
    #[error("Jiff error: {0}")]
    JiffError(#[from] jiff::Error),
}

/// A specialized Result type for ICS operations.
pub type Result<T> = std::result::Result<T, IcsError>;

/// A simple date and time representation for calendar events.
///
/// This is a lightweight datetime type for basic use cases. For full timezone
/// support, enable the `jiff` feature and use [`jiff::Zoned`] with the
/// `start_zoned()` and `end_zoned()` methods on [`Event`].
///
/// # Examples
///
/// Creating a local datetime:
///
/// ```
/// use simple_ics::DateTime;
///
/// let dt = DateTime::new(2026, 3, 15, 14, 30, 0);
/// assert_eq!(dt.to_ics_string(), &quot;20260315T143000&quot;);
/// ```
///
/// Creating a UTC datetime:
///
/// ```
/// use simple_ics::DateTime;
///
/// let dt = DateTime::new_utc(2026, 3, 15, 14, 30, 0);
/// assert_eq!(dt.to_ics_string(), &quot;20260315T143000Z&quot;);
/// ```
///
/// Creating a date-only value for all-day events:
///
/// ```
/// use simple_ics::DateTime;
///
/// let dt = DateTime::date(2026, 12, 25);
/// assert_eq!(dt.to_ics_date_string(), &quot;20261225&quot;);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DateTime {
    /// Year (e.g., 2026).
    pub year: i16,
    /// Month (1-12).
    pub month: u8,
    /// Day of month (1-31).
    pub day: u8,
    /// Hour (0-23).
    pub hour: u8,
    /// Minute (0-59).
    pub minute: u8,
    /// Second (0-59).
    pub second: u8,
    /// Whether this datetime is in UTC (adds 'Z' suffix in ICS output).
    pub utc: bool,
}

impl DateTime {
    /// Creates a new local datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::DateTime;
    ///
    /// // 15th March 2026 at 09:30:00 local time
    /// let dt = DateTime::new(2026, 3, 15, 9, 30, 0);
    /// ```
    pub fn new(year: i16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            utc: false,
        }
    }

    /// Creates a new UTC datetime.
    ///
    /// The resulting ICS output will include the 'Z' suffix indicating UTC time.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::DateTime;
    ///
    /// let dt = DateTime::new_utc(2026, 3, 15, 8, 30, 0);
    /// assert!(dt.to_ics_string().ends_with(&#x27;Z&#x27;));
    /// ```
    pub fn new_utc(year: i16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            utc: true,
        }
    }

    /// Creates a date-only value for all-day events.
    ///
    /// The time components are set to zero and ignored when formatting.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::DateTime;
    ///
    /// // Christmas 2026
    /// let holiday = DateTime::date(2026, 12, 25);
    /// ```
    pub fn date(year: i16, month: u8, day: u8) -> Self {
        Self::new(year, month, day, 0, 0, 0)
    }

    /// Validates the datetime values.
    ///
    /// Returns an error if month, day, hour, minute, or second are out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::DateTime;
    ///
    /// let valid = DateTime::new(2026, 12, 31, 23, 59, 59);
    /// assert!(valid.validate().is_ok());
    ///
    /// let invalid = DateTime::new(2026, 13, 1, 0, 0, 0); // month 13 is invalid
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.month < 1 || self.month > 12 {
            return Err(IcsError::InvalidDateTime(format!(
                "month {} out of range 1-12",
                self.month
            )));
        }
        if self.day < 1 || self.day > 31 {
            return Err(IcsError::InvalidDateTime(format!(
                "day {} out of range 1-31",
                self.day
            )));
        }
        if self.hour > 23 {
            return Err(IcsError::InvalidDateTime(format!(
                "hour {} out of range 0-23",
                self.hour
            )));
        }
        if self.minute > 59 || self.second > 59 {
            return Err(IcsError::InvalidDateTime(
                "minute/second out of range 0-59".to_string(),
            ));
        }
        Ok(())
    }

    /// Formats as ICS datetime string (`YYYYMMDDTHHMMSS` or `YYYYMMDDTHHMMSSZ` for UTC).
    pub fn to_ics_string(&self) -> String {
        let suffix = if self.utc { "Z" } else { "" };
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}{}",
            self.year, self.month, self.day, self.hour, self.minute, self.second, suffix
        )
    }

    /// Formats as ICS date-only string (`YYYYMMDD`).
    pub fn to_ics_date_string(&self) -> String {
        format!("{:04}{:02}{:02}", self.year, self.month, self.day)
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ics_string())
    }
}

#[cfg(feature = "jiff")]
mod jiff_support {
    use super::*;

    impl From<jiff::civil::DateTime> for DateTime {
        fn from(dt: jiff::civil::DateTime) -> Self {
            Self {
                year: dt.year(),
                month: dt.month() as u8,
                day: dt.day() as u8,
                hour: dt.hour() as u8,
                minute: dt.minute() as u8,
                second: dt.second() as u8,
                utc: false,
            }
        }
    }

    impl From<jiff::civil::Date> for DateTime {
        fn from(d: jiff::civil::Date) -> Self {
            Self {
                year: d.year(),
                month: d.month() as u8,
                day: d.day() as u8,
                hour: 0,
                minute: 0,
                second: 0,
                utc: false,
            }
        }
    }

    impl TryFrom<DateTime> for jiff::civil::DateTime {
        type Error = jiff::Error;

        fn try_from(dt: DateTime) -> std::result::Result<Self, Self::Error> {
            jiff::civil::DateTime::new(
                dt.year,
                dt.month as i8,
                dt.day as i8,
                dt.hour as i8,
                dt.minute as i8,
                dt.second as i8,
                0,
            )
        }
    }

    /// A wrapper for timezone-aware datetimes from jiff.
    ///
    /// This type wraps a [`jiff::Zoned`] value and provides methods for
    /// formatting it in ICS format with proper timezone identifiers.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use simple_ics::ZonedDateTime;
    /// use jiff::Zoned;
    ///
    /// let zoned: Zoned = &quot;2026-01-15T10:00[Europe/Vienna]&quot;.parse().unwrap();
    /// let zdt = ZonedDateTime::new(zoned);
    ///
    /// let (tzid, dt_str) = zdt.to_ics_with_tzid();
    /// assert_eq!(tzid, &quot;Europe/Vienna&quot;);
    /// assert_eq!(dt_str, &quot;20260115T100000&quot;);
    /// ```
    #[derive(Debug, Clone)]
    pub struct ZonedDateTime {
        inner: jiff::Zoned,
    }

    impl ZonedDateTime {
        /// Creates a new ZonedDateTime from a jiff Zoned value.
        pub fn new(zoned: jiff::Zoned) -> Self {
            Self { inner: zoned }
        }

        /// Returns the IANA timezone identifier (e.g., "Europe/Vienna").
        pub fn timezone_id(&self) -> &str {
            self.inner.time_zone().iana_name().unwrap_or("UTC")
        }

        /// Formats for ICS output, returning the timezone ID and datetime string.
        ///
        /// Returns a tuple of (timezone_id, datetime_string) for use with TZID parameter.
        pub fn to_ics_with_tzid(&self) -> (String, String) {
            let dt = self.inner.datetime();
            let ics_dt = format!(
                "{:04}{:02}{:02}T{:02}{:02}{:02}",
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            );
            (self.timezone_id().to_string(), ics_dt)
        }

        /// Converts to UTC and formats as ICS datetime with Z suffix.
        pub fn to_ics_utc(&self) -> String {
            let ts = self.inner.timestamp();
            let dt = ts.to_zoned(jiff::tz::TimeZone::UTC).datetime();
            format!(
                "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            )
        }
    }

    impl From<jiff::Zoned> for ZonedDateTime {
        fn from(zoned: jiff::Zoned) -> Self {
            Self::new(zoned)
        }
    }
}

#[cfg(feature = "jiff")]
pub use jiff_support::ZonedDateTime;

/// Unified datetime type for ICS output.
///
/// This enum can hold either a simple [`DateTime`], a date-only value for all-day
/// events, or (with the `jiff` feature) a timezone-aware [`ZonedDateTime`].
///
/// You typically don't need to construct this directly—use the builder methods
/// on [`Event`] instead.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IcsDateTime {
    /// A simple local or UTC datetime.
    Simple(DateTime),

    /// A date-only value for all-day events.
    DateOnly(DateTime),

    #[cfg(feature = "jiff")]
    #[cfg_attr(feature = "serde", serde(skip))]
    /// A timezone-aware datetime from jiff.
    Zoned(ZonedDateTime),
}

impl IcsDateTime {
    /// Writes the datetime in ICS format to the given writer.
    pub fn write_ics(&self, w: &mut impl Write, property: &str) -> fmt::Result {
        match self {
            IcsDateTime::Simple(dt) => {
                writeln!(w, "{}:{}", property, dt.to_ics_string())
            }
            IcsDateTime::DateOnly(dt) => {
                writeln!(w, "{};VALUE=DATE:{}", property, dt.to_ics_date_string())
            }
            #[cfg(feature = "jiff")]
            IcsDateTime::Zoned(zdt) => {
                let (tzid, dt_str) = zdt.to_ics_with_tzid();
                writeln!(w, "{};TZID={}:{}", property, tzid, dt_str)
            }
        }
    }

    /// Validates the datetime.
    pub fn validate(&self) -> Result<()> {
        match self {
            IcsDateTime::Simple(dt) | IcsDateTime::DateOnly(dt) => dt.validate(),
            #[cfg(feature = "jiff")]
            IcsDateTime::Zoned(_) => Ok(()),
        }
    }
}

impl From<DateTime> for IcsDateTime {
    fn from(dt: DateTime) -> Self {
        IcsDateTime::Simple(dt)
    }
}

#[cfg(feature = "jiff")]
impl From<jiff::civil::DateTime> for IcsDateTime {
    fn from(dt: jiff::civil::DateTime) -> Self {
        IcsDateTime::Simple(DateTime::from(dt))
    }
}

#[cfg(feature = "jiff")]
impl From<jiff::civil::Date> for IcsDateTime {
    fn from(d: jiff::civil::Date) -> Self {
        IcsDateTime::DateOnly(DateTime::from(d))
    }
}

#[cfg(feature = "jiff")]
impl From<jiff::Zoned> for IcsDateTime {
    fn from(zoned: jiff::Zoned) -> Self {
        IcsDateTime::Zoned(ZonedDateTime::from(zoned))
    }
}

/// The status of a calendar event.
///
/// # Example
///
/// ```
/// use simple_ics::{Event, EventStatus};
///
/// let event = Event::new(&quot;evt-001&quot;, &quot;Tentative Meeting&quot;)
///     .status(EventStatus::Tentative);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EventStatus {
    /// The event is confirmed to occur.
    #[default]
    Confirmed,
    /// The event is tentatively scheduled.
    Tentative,
    /// The event has been cancelled.
    Cancelled,
}

impl Display for EventStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventStatus::Confirmed => write!(f, "CONFIRMED"),
            EventStatus::Tentative => write!(f, "TENTATIVE"),
            EventStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// The classification (visibility) of a calendar event.
///
/// # Example
///
/// ```
/// use simple_ics::{Event, Classification};
///
/// let event = Event::new(&quot;private-001&quot;, &quot;Doctor Appointment&quot;)
///     .classification(Classification::Private);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Classification {
    /// The event is public and can be shared freely.
    #[default]
    Public,
    /// The event is private and should not be shared.
    Private,
    /// The event is confidential.
    Confidential,
}

impl Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Classification::Public => write!(f, "PUBLIC"),
            Classification::Private => write!(f, "PRIVATE"),
            Classification::Confidential => write!(f, "CONFIDENTIAL"),
        }
    }
}

/// A calendar event (VEVENT component).
///
/// Use the builder pattern to construct events with the desired properties.
///
/// # Required Fields
///
/// - `uid` - A globally unique identifier for the event
/// - `summary` - The title/name of the event
///
/// # Example
///
/// ```
/// use simple_ics::{Event, DateTime, EventStatus};
///
/// let event = Event::new(&quot;concert-2026&quot;, &quot;Vienna Philharmonic Concert&quot;)
///     .start(DateTime::new(2026, 6, 20, 19, 30, 0))
///     .end(DateTime::new(2026, 6, 20, 22, 0, 0))
///     .location(&quot;Musikverein, Vienna&quot;)
///     .description(&quot;Summer Night Concert in the Golden Hall&quot;)
///     .url(&quot;https://www.musikverein.at&quot;)
///     .status(EventStatus::Confirmed);
/// ```
///
/// # All-Day Events
///
/// For all-day events, use `start_date()` and `end_date()`:
///
/// ```
/// use simple_ics::{Event, DateTime};
///
/// let holiday = Event::new(&quot;austria-national-day&quot;, &quot;Austrian National Day&quot;)
///     .start_date(DateTime::date(2026, 10, 26))
///     .end_date(DateTime::date(2026, 10, 27)); // End date is exclusive
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Event {
    /// Unique identifier for this event.
    pub uid: String,
    /// Title/summary of the event.
    pub summary: String,
    /// Detailed description of the event.
    pub description: Option<String>,
    /// Location where the event takes place.
    pub location: Option<String>,
    /// Start date/time of the event.
    pub start: Option<IcsDateTime>,
    /// End date/time of the event.
    pub end: Option<IcsDateTime>,
    /// Status of the event (confirmed, tentative, cancelled).
    pub status: EventStatus,
    /// Classification/visibility of the event.
    pub classification: Classification,
    /// URL associated with the event.
    pub url: Option<String>,
    /// Email address of the event organizer.
    pub organizer: Option<String>,
    /// Timestamp when the event was created.
    pub created: Option<IcsDateTime>,
    /// Timestamp when the event was last modified.
    pub last_modified: Option<IcsDateTime>,
}

impl Event {
    /// Creates a new event with the given unique ID and summary.
    ///
    /// # Arguments
    ///
    /// * `uid` - A globally unique identifier. Consider using a UUID or
    ///   a combination of timestamp and domain (e.g., `event-123@example.com`).
    /// * `summary` - The title of the event shown in calendar applications.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::Event;
    ///
    /// let event = Event::new(&quot;meeting-2026-01-15@mycompany.com&quot;, &quot;Team Meeting&quot;);
    /// ```
    pub fn new(uid: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            uid: uid.into(),
            summary: summary.into(),
            description: None,
            location: None,
            start: None,
            end: None,
            status: EventStatus::default(),
            classification: Classification::default(),
            url: None,
            organizer: None,
            created: None,
            last_modified: None,
        }
    }

    /// Sets the start date/time.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::{Event, DateTime};
    ///
    /// let event = Event::new(&quot;evt&quot;, &quot;Meeting&quot;)
    ///     .start(DateTime::new(2026, 1, 15, 10, 0, 0));
    /// ```
    pub fn start(mut self, dt: DateTime) -> Self {
        self.start = Some(IcsDateTime::Simple(dt));
        self
    }

    /// Sets the end date/time.
    pub fn end(mut self, dt: DateTime) -> Self {
        self.end = Some(IcsDateTime::Simple(dt));
        self
    }

    /// Sets start as date-only for all-day events.
    pub fn start_date(mut self, dt: DateTime) -> Self {
        self.start = Some(IcsDateTime::DateOnly(dt));
        self
    }

    /// Sets end as date-only for all-day events.
    ///
    /// Note: The end date is exclusive in iCalendar format. For a single-day
    /// event on January 15th, set end to January 16th.
    pub fn end_date(mut self, dt: DateTime) -> Self {
        self.end = Some(IcsDateTime::DateOnly(dt));
        self
    }

    /// Sets the start using a jiff civil datetime.
    #[cfg(feature = "jiff")]
    pub fn start_civil(mut self, dt: jiff::civil::DateTime) -> Self {
        self.start = Some(IcsDateTime::from(dt));
        self
    }

    /// Sets the end using a jiff civil datetime.
    #[cfg(feature = "jiff")]
    pub fn end_civil(mut self, dt: jiff::civil::DateTime) -> Self {
        self.end = Some(IcsDateTime::from(dt));
        self
    }

    /// Sets the start using a jiff date (for all-day events).
    #[cfg(feature = "jiff")]
    pub fn start_date_jiff(mut self, d: jiff::civil::Date) -> Self {
        self.start = Some(IcsDateTime::from(d));
        self
    }

    /// Sets the end using a jiff date (for all-day events).
    #[cfg(feature = "jiff")]
    pub fn end_date_jiff(mut self, d: jiff::civil::Date) -> Self {
        self.end = Some(IcsDateTime::from(d));
        self
    }

    /// Sets the start using a timezone-aware jiff Zoned datetime.
    ///
    /// The timezone identifier will be included in the ICS output using
    /// the TZID parameter.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use simple_ics::Event;
    /// use jiff::Zoned;
    ///
    /// let start: Zoned = &quot;2026-01-15T10:00[Europe/Vienna]&quot;.parse().unwrap();
    /// let event = Event::new(&quot;evt&quot;, &quot;Vienna Meeting&quot;).start_zoned(start);
    /// ```
    #[cfg(feature = "jiff")]
    pub fn start_zoned(mut self, zoned: jiff::Zoned) -> Self {
        self.start = Some(IcsDateTime::from(zoned));
        self
    }

    /// Sets the end using a timezone-aware jiff Zoned datetime.
    #[cfg(feature = "jiff")]
    pub fn end_zoned(mut self, zoned: jiff::Zoned) -> Self {
        self.end = Some(IcsDateTime::from(zoned));
        self
    }

    /// Sets the event description.
    ///
    /// The description can contain longer text and will be properly escaped.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Sets the event location.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::Event;
    ///
    /// let event = Event::new(&quot;evt&quot;, &quot;Dinner&quot;)
    ///     .location(&quot;Steirereck, Stadtpark, 1030 Vienna&quot;);
    /// ```
    pub fn location(mut self, loc: impl Into<String>) -> Self {
        self.location = Some(loc.into());
        self
    }

    /// Sets the event status.
    pub fn status(mut self, status: EventStatus) -> Self {
        self.status = status;
        self
    }

    /// Sets the event classification.
    pub fn classification(mut self, class: Classification) -> Self {
        self.classification = class;
        self
    }

    /// Sets a URL associated with the event.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the organizer's email address.
    pub fn organizer(mut self, email: impl Into<String>) -> Self {
        self.organizer = Some(email.into());
        self
    }

    /// Sets the created timestamp.
    pub fn created(mut self, dt: impl Into<IcsDateTime>) -> Self {
        self.created = Some(dt.into());
        self
    }

    /// Sets the last modified timestamp.
    pub fn last_modified(mut self, dt: impl Into<IcsDateTime>) -> Self {
        self.last_modified = Some(dt.into());
        self
    }

    /// Validates the event, checking for required fields and valid values.
    pub fn validate(&self) -> Result<()> {
        if self.uid.is_empty() {
            return Err(IcsError::MissingField("uid".to_string()));
        }
        if self.summary.is_empty() {
            return Err(IcsError::MissingField("summary".to_string()));
        }
        if let Some(ref start) = self.start {
            start.validate()?;
        }
        if let Some(ref end) = self.end {
            end.validate()?;
        }
        Ok(())
    }

    /// Writes the event in ICS format.
    pub fn write_ics(&self, w: &mut impl Write) -> fmt::Result {
        writeln!(w, "BEGIN:VEVENT")?;
        writeln!(w, "UID:{}", escape_text(&self.uid))?;
        writeln!(w, "SUMMARY:{}", escape_text(&self.summary))?;

        if let Some(ref start) = self.start {
            start.write_ics(w, "DTSTART")?;
        }

        if let Some(ref end) = self.end {
            end.write_ics(w, "DTEND")?;
        }

        if let Some(ref desc) = self.description {
            writeln!(w, "DESCRIPTION:{}", escape_text(desc))?;
        }

        if let Some(ref loc) = self.location {
            writeln!(w, "LOCATION:{}", escape_text(loc))?;
        }

        if let Some(ref url) = self.url {
            writeln!(w, "URL:{}", url)?;
        }

        if let Some(ref org) = self.organizer {
            writeln!(w, "ORGANIZER:mailto:{}", org)?;
        }

        if let Some(ref created) = self.created {
            created.write_ics(w, "CREATED")?;
        }

        if let Some(ref modified) = self.last_modified {
            modified.write_ics(w, "LAST-MODIFIED")?;
        }

        writeln!(w, "STATUS:{}", self.status)?;
        writeln!(w, "CLASS:{}", self.classification)?;
        writeln!(w, "END:VEVENT")?;
        Ok(())
    }
}

/// A calendar (VCALENDAR object) containing events.
///
/// Use the builder pattern to construct a calendar and add events.
///
/// # Example
///
/// ```
/// use simple_ics::{Calendar, Event, DateTime};
///
/// let events = vec![
///     Event::new(&quot;1&quot;, &quot;Morning Coffee&quot;)
///         .start(DateTime::new(2026, 1, 15, 9, 0, 0))
///         .location(&quot;Café Sperl, Vienna&quot;),
///     Event::new(&quot;2&quot;, &quot;Museum Visit&quot;)
///         .start(DateTime::new(2026, 1, 15, 14, 0, 0))
///         .location(&quot;Kunsthistorisches Museum, Vienna&quot;),
/// ];
///
/// let calendar = Calendar::new(&quot;Vienna Trip&quot;)
///     .product_id(&quot;-//MyApp//Vienna Planner//EN&quot;)
///     .timezone(&quot;Europe/Vienna&quot;)
///     .events(events);
///
/// let ics = calendar.to_ics();
/// ```
///
/// # Saving to File
///
/// ```no_run
/// use simple_ics::Calendar;
///
/// let calendar = Calendar::new(&quot;My Calendar&quot;);
/// calendar.save_to_file(&quot;calendar.ics&quot;).expect(&quot;Failed to save&quot;);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Calendar {
    /// Optional display name for the calendar.
    pub name: Option<String>,
    /// Product identifier (should follow `-//Company//Product//Language` format).
    pub product_id: String,
    /// iCalendar version (always "2.0").
    pub version: String,
    /// Events in this calendar.
    pub events: Vec<Event>,
    /// Default timezone for the calendar.
    pub timezone: Option<String>,
}

impl Calendar {
    /// Creates a new calendar with the given display name.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::Calendar;
    ///
    /// let calendar = Calendar::new(&quot;Work Schedule&quot;);
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            product_id: "-//simple-ics//EN".to_string(),
            version: "2.0".to_string(),
            events: Vec::new(),
            timezone: None,
        }
    }

    /// Creates an empty calendar without a name.
    pub fn empty() -> Self {
        Self {
            name: None,
            product_id: "-//simple-ics//EN".to_string(),
            version: "2.0".to_string(),
            events: Vec::new(),
            timezone: None,
        }
    }

    /// Sets the product identifier.
    ///
    /// The product ID should follow the format `-//Company//Product//Language`.
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::Calendar;
    ///
    /// let calendar = Calendar::new(&quot;Schedule&quot;)
    ///     .product_id(&quot;-//Acme Corp//Event Manager 1.0//EN&quot;);
    /// ```
    pub fn product_id(mut self, id: impl Into<String>) -> Self {
        self.product_id = id.into();
        self
    }

    /// Sets the default timezone for the calendar.
    ///
    /// Use an IANA timezone identifier like "Europe/Vienna".
    ///
    /// # Example
    ///
    /// ```
    /// use simple_ics::Calendar;
    ///
    /// let calendar = Calendar::new(&quot;Vienna Events&quot;)
    ///     .timezone(&quot;Europe/Vienna&quot;);
    /// ```
    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.timezone = Some(tz.into());
        self
    }

    /// Sets timezone from a jiff TimeZone.
    #[cfg(feature = "jiff")]
    pub fn timezone_jiff(mut self, tz: &jiff::tz::TimeZone) -> Self {
        if let Some(name) = tz.iana_name() {
            self.timezone = Some(name.to_string());
        }
        self
    }

    /// Adds an event to the calendar.
    pub fn event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    /// Adds multiple events to the calendar.
    pub fn events(mut self, events: impl IntoIterator<Item = Event>) -> Self {
        self.events.extend(events);
        self
    }

    /// Validates all events in the calendar.
    pub fn validate(&self) -> Result<()> {
        for event in &self.events {
            event.validate()?;
        }
        Ok(())
    }

    /// Generates the ICS file content as a String.
    ///
    /// The output follows RFC5545 format with CRLF line endings.
    pub fn to_ics(&self) -> String {
        let mut output = String::new();
        self.write_ics(&mut output).expect("String write failed");
        normalize_line_endings(&output)
    }

    /// Writes the calendar in ICS format to a writer.
    pub fn write_ics(&self, w: &mut impl Write) -> fmt::Result {
        writeln!(w, "BEGIN:VCALENDAR")?;
        writeln!(w, "VERSION:{}", self.version)?;
        writeln!(w, "PRODID:{}", self.product_id)?;
        writeln!(w, "CALSCALE:GREGORIAN")?;
        writeln!(w, "METHOD:PUBLISH")?;

        if let Some(ref name) = self.name {
            writeln!(w, "X-WR-CALNAME:{}", escape_text(name))?;
        }

        if let Some(ref tz) = self.timezone {
            writeln!(w, "X-WR-TIMEZONE:{}", tz)?;
        }

        for event in &self.events {
            event.write_ics(w)?;
        }

        writeln!(w, "END:VCALENDAR")?;
        Ok(())
    }

    /// Saves the calendar to a file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use simple_ics::Calendar;
    ///
    /// let calendar = Calendar::new(&quot;My Calendar&quot;);
    /// calendar.save_to_file(&quot;my_calendar.ics&quot;).expect(&quot;Failed to save&quot;);
    /// ```
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.to_ics())
    }
}

impl Default for Calendar {
    fn default() -> Self {
        Self::empty()
    }
}

/// Helper module for working with jiff datetimes.
///
/// Available only when the `jiff` feature is enabled.
#[cfg(feature = "jiff")]
pub mod jiff_helpers {
    use super::*;

    /// Creates an event with automatic created/last_modified timestamps.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use simple_ics::jiff_helpers;
    ///
    /// let event = jiff_helpers::event_now(&quot;evt-001&quot;, &quot;New Event&quot;)?;
    /// // event.created and event.last_modified are set to now
    /// ```
    pub fn event_now(uid: impl Into<String>, summary: impl Into<String>) -> Result<Event> {
        let now = jiff::Zoned::now();
        Ok(Event::new(uid, summary)
            .created(IcsDateTime::Zoned(ZonedDateTime::from(now.clone())))
            .last_modified(IcsDateTime::Zoned(ZonedDateTime::from(now))))
    }

    /// Returns the current UTC time as a DateTime.
    pub fn utc_now() -> Result<DateTime> {
        let now = jiff::Timestamp::now();
        let dt = now.to_zoned(jiff::tz::TimeZone::UTC).datetime();
        Ok(DateTime {
            year: dt.year(),
            month: dt.month() as u8,
            day: dt.day() as u8,
            hour: dt.hour() as u8,
            minute: dt.minute() as u8,
            second: dt.second() as u8,
            utc: true,
        })
    }
}

fn escape_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_formatting() {
        let dt = DateTime::new(2026, 1, 15, 14, 30, 0);
        assert_eq!(dt.to_ics_string(), "20260115T143000");

        let dt_utc = DateTime::new_utc(2026, 1, 15, 14, 30, 0);
        assert_eq!(dt_utc.to_ics_string(), "20260115T143000Z");
    }

    #[test]
    fn test_datetime_validation() {
        let valid = DateTime::new(2026, 12, 31, 23, 59, 59);
        assert!(valid.validate().is_ok());

        let invalid_month = DateTime::new(2026, 13, 1, 0, 0, 0);
        assert!(invalid_month.validate().is_err());
    }

    #[test]
    fn test_event_builder() {
        let event = Event::new("test-123", "Coffee at Café Central")
            .description("Discussing project ideas")
            .location("Café Central, Herrengasse 14, 1010 Vienna")
            .start(DateTime::new(2026, 1, 15, 10, 0, 0))
            .end(DateTime::new(2026, 1, 15, 11, 30, 0));

        assert_eq!(event.uid, "test-123");
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_all_day_event() {
        let event = Event::new("holiday-001", "Austrian National Day")
            .start_date(DateTime::date(2026, 10, 26))
            .end_date(DateTime::date(2026, 10, 27));

        let calendar = Calendar::empty().event(event);
        let ics = calendar.to_ics();

        assert!(ics.contains("DTSTART;VALUE=DATE:20261026"));
        assert!(ics.contains("DTEND;VALUE=DATE:20261027"));
    }

    #[test]
    fn test_calendar_generation() {
        let event = Event::new("evt-001", "Schönbrunn Palace Tour")
            .start(DateTime::new_utc(2026, 1, 15, 9, 0, 0))
            .end(DateTime::new_utc(2026, 1, 15, 12, 0, 0))
            .location("Schönbrunn Palace, Vienna");

        let calendar = Calendar::new("Vienna Sightseeing")
            .timezone("Europe/Vienna")
            .event(event);
        let ics = calendar.to_ics();

        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("X-WR-CALNAME:Vienna Sightseeing"));
        assert!(ics.contains("X-WR-TIMEZONE:Europe/Vienna"));
        assert!(ics.contains("DTSTART:20260115T090000Z"));
    }

    #[test]
    fn test_text_escaping() {
        let event = Event::new("esc", "Meeting; Important")
            .description("Line1\nLine2")
            .location("Room, Floor 2");

        let calendar = Calendar::empty().event(event);
        let ics = calendar.to_ics();

        assert!(ics.contains("SUMMARY:Meeting\\; Important"));
        assert!(ics.contains("DESCRIPTION:Line1\\nLine2"));
        assert!(ics.contains("LOCATION:Room\\, Floor 2"));
    }

    #[test]
    fn test_crlf_line_endings() {
        let calendar = Calendar::new("Test").event(Event::new("1", "Event"));
        let ics = calendar.to_ics();

        assert!(ics.contains("\r\n"));
    }

    #[test]
    fn test_multiple_events() {
        let events = vec![
            Event::new("1", "Stephansdom Visit"),
            Event::new("2", "Naschmarkt Walk"),
            Event::new("3", "Opera Performance"),
        ];

        let calendar = Calendar::new("Vienna Day Trip").events(events);
        let ics = calendar.to_ics();

        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 3);
        assert_eq!(ics.matches("END:VEVENT").count(), 3);
    }

    #[test]
    fn test_event_with_url_and_organizer() {
        let event = Event::new("opera-001", "Vienna State Opera")
            .url("https://www.wiener-staatsoper.at")
            .organizer("tickets@wiener-staatsoper.at");

        let calendar = Calendar::empty().event(event);
        let ics = calendar.to_ics();

        assert!(ics.contains("URL:https://www.wiener-staatsoper.at"));
        assert!(ics.contains("ORGANIZER:mailto:tickets@wiener-staatsoper.at"));
    }

    #[test]
    fn test_event_validation_errors() {
        let empty_uid = Event::new("", "Summary");
        assert!(matches!(
            empty_uid.validate(),
            Err(IcsError::MissingField(_))
        ));

        let empty_summary = Event::new("uid", "");
        assert!(matches!(
            empty_summary.validate(),
            Err(IcsError::MissingField(_))
        ));
    }

    #[cfg(feature = "jiff")]
    mod jiff_tests {
        use super::*;
        use jiff::civil::{date, datetime};

        #[test]
        fn test_jiff_civil_datetime() {
            let event = Event::new("jiff-001", "Belvedere Museum")
                .start_civil(datetime(2026, 1, 15, 10, 0, 0, 0))
                .end_civil(datetime(2026, 1, 15, 13, 0, 0, 0))
                .location("Belvedere Palace, Vienna");

            let calendar = Calendar::empty().event(event);
            let ics = calendar.to_ics();

            assert!(ics.contains("DTSTART:20260115T100000"));
            assert!(ics.contains("DTEND:20260115T130000"));
        }

        #[test]
        fn test_jiff_civil_date() {
            let event = Event::new("jiff-002", "Vienna City Marathon")
                .start_date_jiff(date(2026, 4, 19))
                .end_date_jiff(date(2026, 4, 20));

            let calendar = Calendar::empty().event(event);
            let ics = calendar.to_ics();

            assert!(ics.contains("DTSTART;VALUE=DATE:20260419"));
            assert!(ics.contains("DTEND;VALUE=DATE:20260420"));
        }

        #[test]
        fn test_jiff_zoned_datetime() {
            let zoned = "2026-01-15T19:00:00[Europe/Vienna]"
                .parse::<jiff::Zoned>()
                .unwrap();

            let event = Event::new("zoned-001", "Vienna Philharmonic")
                .start_zoned(zoned)
                .location("Musikverein, Vienna");

            let calendar = Calendar::empty().event(event);
            let ics = calendar.to_ics();

            assert!(ics.contains("DTSTART;TZID=Europe/Vienna:20260115T190000"));
        }

        #[test]
        fn test_datetime_conversion_roundtrip() {
            let jiff_dt = datetime(2026, 3, 20, 15, 45, 30, 0);
            let our_dt: DateTime = jiff_dt.into();
            let back: jiff::civil::DateTime = our_dt.try_into().unwrap();

            assert_eq!(jiff_dt, back);
        }
    }
}
