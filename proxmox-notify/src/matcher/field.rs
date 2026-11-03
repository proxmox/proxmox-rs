use std::{fmt, str::FromStr};

use const_format::concatcp;
use regex::Regex;

use proxmox_schema::{
    ApiStringFormat, Schema, StringSchema, api_types::SAFE_ID_REGEX_STR, const_regex,
};

use crate::{Error, Notification};

use super::MatchDirective;

const_regex! {
    pub MATCH_FIELD_ENTRY_REGEX = concatcp!(r"^(?:(exact|regex):)?(", SAFE_ID_REGEX_STR, r")=(.*)$");
}

pub const MATCH_FIELD_ENTRY_FORMAT: ApiStringFormat =
    ApiStringFormat::VerifyFn(verify_inline_field_matcher);

fn verify_inline_field_matcher(s: &str) -> Result<(), anyhow::Error> {
    let _: InlineFieldMatcher = s.parse()?;
    Ok(())
}

pub const MATCH_FIELD_ENTRY_SCHEMA: Schema = StringSchema::new("Match metadata field.")
    .format(&MATCH_FIELD_ENTRY_FORMAT)
    .min_length(1)
    .max_length(1024)
    .schema();

/// Check if the notification metadata fields match
#[derive(Clone, Debug)]
pub enum FieldMatcher {
    Exact {
        field: String,
        matched_values: Vec<String>,
    },
    Regex {
        field: String,
        matched_regex: Regex,
    },
}

impl MatchDirective for FieldMatcher {
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        Ok(match self {
            FieldMatcher::Exact {
                field,
                matched_values,
            } => {
                let value = notification.metadata.additional_fields.get(field);

                if let Some(value) = value {
                    matched_values.contains(value)
                } else {
                    // Metadata field does not exist, so we do not match
                    false
                }
            }
            FieldMatcher::Regex {
                field,
                matched_regex,
            } => {
                let value = notification.metadata.additional_fields.get(field);

                if let Some(value) = value {
                    matched_regex.is_match(value)
                } else {
                    // Metadata field does not exist, so we do not match
                    false
                }
            }
        })
    }
}

/// Check if the notification metadata fields match
///
/// This is a wrapper that serializes into an inline format that can be used
/// in a section config key-value pair.
#[derive(Clone, Debug)]
pub struct InlineFieldMatcher(FieldMatcher);

impl MatchDirective for InlineFieldMatcher {
    fn matches(&self, notification: &Notification) -> Result<bool, Error> {
        self.0.matches(notification)
    }
}

impl fmt::Display for InlineFieldMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Attention, Display is used to implement Serialize, do not
        // change the format.

        match &self.0 {
            FieldMatcher::Exact {
                field,
                matched_values,
            } => {
                let values = matched_values.join(",");
                write!(f, "exact:{field}={values}")
            }
            FieldMatcher::Regex {
                field,
                matched_regex,
            } => {
                let re = matched_regex.as_str();
                write!(f, "regex:{field}={re}")
            }
        }
    }
}

impl FromStr for InlineFieldMatcher {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        if !MATCH_FIELD_ENTRY_REGEX.is_match(s) {
            return Err(Error::FilterFailed(format!(
                "invalid match-field statement: {s}"
            )));
        }

        if let Some(remaining) = s.strip_prefix("regex:") {
            match remaining.split_once('=') {
                None => Err(Error::FilterFailed(format!(
                    "invalid match-field statement: {s}"
                ))),
                Some((field, expected_value_regex)) => {
                    let regex = Regex::new(expected_value_regex)
                        .map_err(|err| Error::FilterFailed(format!("invalid regex: {err}")))?;

                    Ok(Self(FieldMatcher::Regex {
                        field: field.into(),
                        matched_regex: regex,
                    }))
                }
            }
        } else if let Some(remaining) = s.strip_prefix("exact:") {
            match remaining.split_once('=') {
                None => Err(Error::FilterFailed(format!(
                    "invalid match-field statement: {s}"
                ))),
                Some((field, expected_values)) => {
                    let values: Vec<String> = expected_values
                        .split(',')
                        .map(str::trim)
                        .map(String::from)
                        .collect();
                    Ok(Self(FieldMatcher::Exact {
                        field: field.into(),
                        matched_values: values,
                    }))
                }
            }
        } else {
            Err(Error::FilterFailed(format!(
                "invalid match-field statement: {s}"
            )))
        }
    }
}

impl InlineFieldMatcher {
    pub fn into_inner(self) -> FieldMatcher {
        self.0
    }
}

proxmox_serde::forward_deserialize_to_from_str!(InlineFieldMatcher);
proxmox_serde::forward_serialize_to_display!(InlineFieldMatcher);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value;

    use crate::Severity;

    use super::*;

    #[test]
    fn test_matching() {
        let mut fields = HashMap::new();
        fields.insert("foo".into(), "bar".into());

        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, fields);

        let matcher: InlineFieldMatcher = "exact:foo=bar".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());

        let matcher: InlineFieldMatcher = "regex:foo=b.*".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());

        let matcher: InlineFieldMatcher = "regex:notthere=b.*".parse().unwrap();
        assert!(!matcher.matches(&notification).unwrap());

        let matcher: InlineFieldMatcher = "exact:foo=bar,test".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());

        let mut fields = HashMap::new();
        fields.insert("foo".into(), "test".into());

        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, fields);
        assert!(matcher.matches(&notification).unwrap());

        let mut fields = HashMap::new();
        fields.insert("foo".into(), "notthere".into());

        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, fields);
        assert!(!matcher.matches(&notification).unwrap());

        assert!("regex:'3=b.*".parse::<InlineFieldMatcher>().is_err());
        assert!("invalid:'bar=b.*".parse::<InlineFieldMatcher>().is_err());
    }
}
