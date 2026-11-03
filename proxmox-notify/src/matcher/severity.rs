use std::fmt;
use std::str::FromStr;

use crate::{Error, Notification, Severity};

use super::MatchDirective;

/// Match severity of the notification.
#[derive(Clone, Debug)]
pub struct SeverityMatcher {
    severities: Vec<Severity>,
}

/// Common trait implemented by all matching directives
impl MatchDirective for SeverityMatcher {
    /// Check if this directive matches a given notification
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        Ok(self.severities.contains(&notification.metadata.severity))
    }
}

impl fmt::Display for SeverityMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let severities: Vec<String> = self.severities.iter().map(|s| format!("{s}")).collect();
        f.write_str(&severities.join(","))
    }
}

impl FromStr for SeverityMatcher {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let mut severities = Vec::new();

        for element in s.split(',') {
            let element = element.trim();
            let severity: Severity = element.parse()?;

            severities.push(severity)
        }

        Ok(Self { severities })
    }
}

proxmox_serde::forward_deserialize_to_from_str!(SeverityMatcher);
proxmox_serde::forward_serialize_to_display!(SeverityMatcher);

#[cfg(test)]
mod test {
    use serde_json::Value;

    use crate::{
        Notification, Severity,
        matcher::{MatchDirective as _, severity::SeverityMatcher},
    };

    #[test]
    fn test_severities() {
        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, Default::default());

        let matcher: SeverityMatcher = "info,notice,warning,error".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());
    }
}
