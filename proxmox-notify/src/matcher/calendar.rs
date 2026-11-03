use std::fmt;
use std::str::FromStr;

use proxmox_match_expression::Expression;
use serde::{Deserialize, Serialize};

use proxmox_time::DailyDuration;

use crate::{Error, Notification, matcher::expression::NotificationMatcher};

use super::MatchDirective;

/// Convenience wrapper around [`DailyDuration`] that implements [`Serialize`] and
/// [`Deserialize`].
#[derive(Clone, Debug)]
struct DailyDurationWrapper {
    schedule: String,
    daily_duration: DailyDuration,
}

impl fmt::Display for DailyDurationWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DailyDurationWrapper {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let daily_duration = proxmox_time::parse_daily_duration(s)
            .map_err(|e| Error::Generic(format!("could not parse schedule: {e}")))?;

        Ok(Self {
            daily_duration,
            schedule: s.to_string(),
        })
    }
}

impl DailyDurationWrapper {
    fn as_str(&self) -> &str {
        &self.schedule
    }
}

proxmox_serde::forward_deserialize_to_from_str!(DailyDurationWrapper);
proxmox_serde::forward_serialize_to_display!(DailyDurationWrapper);

/// Match timestamp of the notification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CalendarMatcher {
    schedule: DailyDurationWrapper,
}

impl From<CalendarMatcher> for Expression<NotificationMatcher> {
    fn from(value: CalendarMatcher) -> Self {
        Expression::Match(NotificationMatcher::Calendar(value))
    }
}

impl MatchDirective for CalendarMatcher {
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        self.schedule
            .daily_duration
            .time_match(notification.metadata.timestamp, false)
            .map_err(|err| Error::Generic(format!("could not match timestamp: {err}")))
    }
}

#[cfg(feature = "legacy-matchers")]
pub mod inline {
    use super::*;

    /// Match the timestamp of a notification.
    ///
    /// This is a wrapper that serializes into an inline format that can be used
    /// in a section config key-value pair.
    #[derive(Clone, Debug)]
    pub struct InlineCalendarMatcher(CalendarMatcher);

    impl MatchDirective for InlineCalendarMatcher {
        fn matches(&self, notification: &Notification) -> Result<bool, Error> {
            self.0.matches(notification)
        }
    }

    impl fmt::Display for InlineCalendarMatcher {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str(self.0.schedule.as_str())
        }
    }

    impl FromStr for InlineCalendarMatcher {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Error> {
            Ok(Self(CalendarMatcher {
                schedule: s.parse()?,
            }))
        }
    }

    impl InlineCalendarMatcher {
        pub fn into_inner(self) -> CalendarMatcher {
            self.0
        }
    }

    proxmox_serde::forward_deserialize_to_from_str!(InlineCalendarMatcher);
    proxmox_serde::forward_serialize_to_display!(InlineCalendarMatcher);
}

#[cfg(feature = "legacy-matchers")]
pub use inline::*;

#[cfg(test)]
mod test {
    use serde_json::Value;

    use crate::{Notification, Severity, matcher::MatchDirective as _};

    use super::*;

    #[test]
    fn test_calendar_matcher() {
        let mut notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, Default::default());

        // Fr 22 Mai 2026 12:45:00 CEST
        notification.metadata.timestamp = 1779446700;

        // Match on a wide rage to avoid issues when running this test case
        // in a different time zone.
        let matcher = CalendarMatcher {
            schedule: "thu..sat 0-23".parse().unwrap(),
        };
        assert!(matcher.matches(&notification).unwrap());
    }

    #[test]
    fn test_calendar_matcher_de_ser_roundtrip() {
        let calendar_matcher = "{ \"schedule\": \"thu..sat 0-23\" }";

        let calendar_matcher: CalendarMatcher = serde_json::from_str(calendar_matcher).unwrap();

        let _s = serde_json::to_string(&calendar_matcher).unwrap();
    }
}
