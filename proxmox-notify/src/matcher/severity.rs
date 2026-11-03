use proxmox_match_expression::Expression;
use serde::{Deserialize, Serialize};

use crate::{Error, Notification, Severity, matcher::expression::NotificationMatcher};

use super::MatchDirective;

/// Match severity of the notification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeverityMatcher {
    pub(crate) severities: Vec<Severity>,
}

impl From<SeverityMatcher> for Expression<NotificationMatcher> {
    fn from(value: SeverityMatcher) -> Self {
        Expression::Match(NotificationMatcher::Severity(value))
    }
}

impl MatchDirective for SeverityMatcher {
    /// Check if this directive matches a given notification
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        Ok(self.severities.contains(&notification.metadata.severity))
    }
}

#[cfg(feature = "legacy-matchers")]
pub mod inline {
    use std::{fmt, str::FromStr};

    use super::*;

    /// Match severity of the notification.
    ///
    /// This is a wrapper that serializes into an inline format that can be used
    /// in a section config key-value pair.
    #[derive(Clone, Debug)]
    pub struct InlineSeverityMatcher(SeverityMatcher);

    /// Common trait implemented by all matching directives
    impl MatchDirective for InlineSeverityMatcher {
        /// Check if this directive matches a given notification
        fn matches(&self, notification: &Notification) -> Result<bool, Error> {
            self.0.matches(notification)
        }
    }

    impl fmt::Display for InlineSeverityMatcher {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let severities: Vec<String> =
                self.0.severities.iter().map(|s| format!("{s}")).collect();
            f.write_str(&severities.join(","))
        }
    }

    impl FromStr for InlineSeverityMatcher {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Error> {
            let mut severities = Vec::new();

            for element in s.split(',') {
                let element = element.trim();
                let severity: Severity = element.parse()?;

                severities.push(severity)
            }

            Ok(Self(SeverityMatcher { severities }))
        }
    }

    impl InlineSeverityMatcher {
        pub fn into_inner(self) -> SeverityMatcher {
            self.0
        }
    }

    proxmox_serde::forward_deserialize_to_from_str!(InlineSeverityMatcher);
    proxmox_serde::forward_serialize_to_display!(InlineSeverityMatcher);

    #[cfg(test)]
    mod test {
        use serde_json::Value;

        use super::*;

        #[test]
        fn test_severities() {
            let notification = Notification::from_template(
                Severity::Notice,
                "test",
                Value::Null,
                Default::default(),
            );

            let matcher: InlineSeverityMatcher = "info,notice,warning,error".parse().unwrap();
            assert!(matcher.matches(&notification).unwrap());
        }
    }
}

#[cfg(feature = "legacy-matchers")]
pub use inline::*;
