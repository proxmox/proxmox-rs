use std::fmt;
use std::str::FromStr;

use proxmox_time::DailyDuration;

use crate::{Error, Notification};

use super::MatchDirective;

/// Match timestamp of the notification.
#[derive(Clone, Debug)]
pub struct CalendarMatcher {
    schedule: DailyDuration,
    original: String,
}

impl MatchDirective for CalendarMatcher {
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        self.schedule
            .time_match(notification.metadata.timestamp, false)
            .map_err(|err| Error::Generic(format!("could not match timestamp: {err}")))
    }
}

impl fmt::Display for CalendarMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl FromStr for CalendarMatcher {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let schedule = proxmox_time::parse_daily_duration(s)
            .map_err(|e| Error::Generic(format!("could not parse schedule: {e}")))?;

        Ok(Self {
            schedule,
            original: s.to_string(),
        })
    }
}

proxmox_serde::forward_deserialize_to_from_str!(CalendarMatcher);
proxmox_serde::forward_serialize_to_display!(CalendarMatcher);
