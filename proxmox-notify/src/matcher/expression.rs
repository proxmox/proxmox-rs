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

impl NotificationMatcher {
    /// Validate any additional constraints that cannot be expressed in the type system.
    pub(crate) fn validate(&self) -> Result<(), Error> {
        use crate::matcher::MatchDirective;

        match self {
            NotificationMatcher::Field(field_matcher) => field_matcher.validate(),
            NotificationMatcher::Calendar(calendar_matcher) => calendar_matcher.validate(),
            NotificationMatcher::Severity(severity_matcher) => severity_matcher.validate(),
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

    #[test]
    fn test_field_matcher_ambigious_variant() {
        let e = r#"
            {
              "match": {
                "type": "field",
                "field": "something",
                "regex": "^abc$",
                "values": ["a", "b"]
                }
            }"#;

        assert!(serde_json::from_str::<Expression<NotificationMatcher>>(e).is_err());
    }
}
