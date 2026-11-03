use serde::{Deserialize, Serialize};

use proxmox_match_expression::MatchExpression;

use crate::{Error, Notification};

use super::{calendar::CalendarMatcher, field::FieldMatcher, severity::SeverityMatcher};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum NotificationMatcher {
    /// Match a notification's metadata field.
    Field(FieldMatcher),
    /// Match a notification's timestamp.
    Calendar(CalendarMatcher),
    /// Match the severity of a notification.
    Severity(SeverityMatcher),
}

impl MatchExpression for NotificationMatcher {
    type Data = Notification;
    type Error = Error;

    fn evaluate(&self, data: &Notification) -> Result<bool, Error> {
        use super::MatchDirective;

        match self {
            NotificationMatcher::Field(field_matcher) => field_matcher.matches(data),
            NotificationMatcher::Calendar(calendar_matcher) => calendar_matcher.matches(data),
            NotificationMatcher::Severity(severity_matcher) => severity_matcher.matches(data),
        }
    }
}

#[cfg(test)]
mod test {
    use proxmox_match_expression::Expression;

    use super::*;

    #[test]
    fn test_matcher_deser() {
        let e = r#"
            {
              "any-of": [
                {
                  "match": {
                    "type": "severity",
                    "severities": [
                      "info",
                      "notice"
                    ]
                  }
                },
                {
                  "match": {
                    "type": "field",
                    "field": "something",
                    "regex": "^abc$"
                    }
                },
                {
                  "match": {
                    "type": "field",
                    "field": "something",
                    "values": [
                      "a",
                      "b",
                      "c"
                    ]
                  }
                },
                {
                  "match": {
                    "type": "calendar",
                    "schedule": "sat,sun 10-14"
                  }
                }
              ]
            }"#;

        let _: Expression<NotificationMatcher> = serde_json::from_str(e).unwrap();
    }
}
