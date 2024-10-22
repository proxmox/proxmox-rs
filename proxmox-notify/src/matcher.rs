use std::collections::HashSet;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;

use const_format::concatcp;
use regex::Regex;
use serde::{Deserialize, Serialize};

use proxmox_schema::api_types::{COMMENT_SCHEMA, SAFE_ID_REGEX_STR};
use proxmox_schema::{api, const_regex, ApiStringFormat, Schema, StringSchema, Updater};
use proxmox_time::{parse_daily_duration, DailyDuration};

use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{Error, Notification, Origin, Severity};

pub const MATCHER_TYPENAME: &str = "matcher";

#[api]
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
/// The mode in which the results of matches are combined.
pub enum MatchModeOperator {
    /// All match statements have to match (AND)
    #[default]
    All,
    /// At least one filter property has to match (OR)
    Any,
}

impl MatchModeOperator {
    /// Apply the mode operator to two bools, lhs and rhs
    fn apply(&self, lhs: bool, rhs: bool) -> bool {
        match self {
            MatchModeOperator::All => lhs && rhs,
            MatchModeOperator::Any => lhs || rhs,
        }
    }

    // https://en.wikipedia.org/wiki/Identity_element
    fn neutral_element(&self) -> bool {
        match self {
            MatchModeOperator::All => true,
            MatchModeOperator::Any => false,
        }
    }
}

const_regex! {
    pub MATCH_FIELD_ENTRY_REGEX = concatcp!(r"^(?:(exact|regex):)?(", SAFE_ID_REGEX_STR, r")=(.*)$");
}

pub const MATCH_FIELD_ENTRY_FORMAT: ApiStringFormat =
    ApiStringFormat::VerifyFn(verify_field_matcher);

fn verify_field_matcher(s: &str) -> Result<(), anyhow::Error> {
    let _: FieldMatcher = s.parse()?;
    Ok(())
}

pub const MATCH_FIELD_ENTRY_SCHEMA: Schema = StringSchema::new("Match metadata field.")
    .format(&MATCH_FIELD_ENTRY_FORMAT)
    .min_length(1)
    .max_length(1024)
    .schema();

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        "match-field": {
            type: Array,
            items: {
                description: "Fields to match",
                type: String
            },
            optional: true,
        },
        "match-severity": {
            type: Array,
            items: {
                description: "Severity level to match.",
                type: String
            },
            optional: true,
        },
        "match-calendar": {
            type: Array,
            items: {
                description: "Time stamps to match",
                type: String
            },
            optional: true,
        },
        "target": {
            type: Array,
            items: {
                schema: ENTITY_NAME_SCHEMA,
            },
            optional: true,
        },
    })]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for Sendmail notification endpoints
pub struct MatcherConfig {
    /// Name of the matcher.
    #[updater(skip)]
    pub name: String,

    /// List of matched metadata fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub match_field: Vec<FieldMatcher>,

    /// List of matched severity levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub match_severity: Vec<SeverityMatcher>,

    /// List of matched severity levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub match_calendar: Vec<CalendarMatcher>,
    /// Decide if 'all' or 'any' match statements must match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<MatchModeOperator>,

    /// Invert match of the whole filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert_match: Option<bool>,

    /// Targets to notify.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub target: Vec<String>,

    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// Disable this matcher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,

    /// Origin of this config entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[updater(skip)]
    pub origin: Option<Origin>,
}

trait MatchDirective {
    fn matches(&self, notification: &Notification) -> Result<bool, Error>;
}

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

proxmox_serde::forward_deserialize_to_from_str!(FieldMatcher);
proxmox_serde::forward_serialize_to_display!(FieldMatcher);

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

impl fmt::Display for FieldMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Attention, Display is used to implement Serialize, do not
        // change the format.

        match self {
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

impl FromStr for FieldMatcher {
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

                    Ok(Self::Regex {
                        field: field.into(),
                        matched_regex: regex,
                    })
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
                    Ok(Self::Exact {
                        field: field.into(),
                        matched_values: values,
                    })
                }
            }
        } else {
            Err(Error::FilterFailed(format!(
                "invalid match-field statement: {s}"
            )))
        }
    }
}

impl MatcherConfig {
    pub fn matches(&self, notification: &Notification) -> Result<Option<&[String]>, Error> {
        let mode = self.mode.unwrap_or_default();

        let mut is_match = mode.neutral_element();
        // If there are no matching directives, the matcher will always match
        let mut no_matchers = true;

        if !self.match_severity.is_empty() {
            no_matchers = false;
            is_match = mode.apply(
                is_match,
                self.check_matches(notification, &self.match_severity)?,
            );
        }
        if !self.match_field.is_empty() {
            no_matchers = false;
            is_match = mode.apply(
                is_match,
                self.check_matches(notification, &self.match_field)?,
            );
        }
        if !self.match_calendar.is_empty() {
            no_matchers = false;
            is_match = mode.apply(
                is_match,
                self.check_matches(notification, &self.match_calendar)?,
            );
        }

        let invert_match = self.invert_match.unwrap_or_default();

        Ok(if is_match != invert_match || no_matchers {
            Some(&self.target)
        } else {
            None
        })
    }

    /// Check if given `MatchDirectives` match a notification.
    fn check_matches(
        &self,
        notification: &Notification,
        matchers: &[impl MatchDirective],
    ) -> Result<bool, Error> {
        let mode = self.mode.unwrap_or_default();
        let mut is_match = mode.neutral_element();

        for field_matcher in matchers {
            is_match = mode.apply(is_match, field_matcher.matches(notification)?);
        }

        Ok(is_match)
    }
}

/// Match severity of the notification.
#[derive(Clone, Debug)]
pub struct SeverityMatcher {
    severities: Vec<Severity>,
}

proxmox_serde::forward_deserialize_to_from_str!(SeverityMatcher);
proxmox_serde::forward_serialize_to_display!(SeverityMatcher);

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

/// Match timestamp of the notification.
#[derive(Clone, Debug)]
pub struct CalendarMatcher {
    schedule: DailyDuration,
    original: String,
}

proxmox_serde::forward_deserialize_to_from_str!(CalendarMatcher);
proxmox_serde::forward_serialize_to_display!(CalendarMatcher);

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
        let schedule = parse_daily_duration(s)
            .map_err(|e| Error::Generic(format!("could not parse schedule: {e}")))?;

        Ok(Self {
            schedule,
            original: s.to_string(),
        })
    }
}

#[api]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// The set of matcher properties that can be deleted.
pub enum DeleteableMatcherProperty {
    /// Delete `comment`
    Comment,
    /// Delete `disable`
    Disable,
    /// Delete `invert-match`
    InvertMatch,
    /// Delete `match-calendar`
    MatchCalendar,
    /// Delete `match-field`
    MatchField,
    /// Delete `match-severity`
    MatchSeverity,
    /// Delete `mode`
    Mode,
    /// Delete `target`
    Target,
}

pub fn check_matches<'a>(
    matchers: &'a [MatcherConfig],
    notification: &Notification,
) -> HashSet<&'a str> {
    let mut targets = HashSet::new();

    for matcher in matchers {
        if matcher.disable.unwrap_or_default() {
            // Skip this matcher if it is disabled
            log::info!("skipping disabled matcher '{name}'", name = matcher.name);
            continue;
        }

        match matcher.matches(notification) {
            Ok(t) => {
                let t = t.unwrap_or_default();
                targets.extend(t.iter().map(|s| s.as_str()));
            }
            Err(err) => log::error!("matcher '{matcher}' failed: {err}", matcher = matcher.name),
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn test_matching() {
        let mut fields = HashMap::new();
        fields.insert("foo".into(), "bar".into());

        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, fields);

        let matcher: FieldMatcher = "exact:foo=bar".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());

        let matcher: FieldMatcher = "regex:foo=b.*".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());

        let matcher: FieldMatcher = "regex:notthere=b.*".parse().unwrap();
        assert!(!matcher.matches(&notification).unwrap());

        let matcher: FieldMatcher = "exact:foo=bar,test".parse().unwrap();
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

        assert!("regex:'3=b.*".parse::<FieldMatcher>().is_err());
        assert!("invalid:'bar=b.*".parse::<FieldMatcher>().is_err());
    }
    #[test]
    fn test_severities() {
        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, Default::default());

        let matcher: SeverityMatcher = "info,notice,warning,error".parse().unwrap();
        assert!(matcher.matches(&notification).unwrap());
    }

    #[test]
    fn test_empty_matcher_matches_always() {
        let notification =
            Notification::from_template(Severity::Notice, "test", Value::Null, Default::default());

        for mode in [MatchModeOperator::All, MatchModeOperator::Any] {
            let config = MatcherConfig {
                name: "matcher".to_string(),
                mode: Some(mode),
                ..Default::default()
            };

            assert!(config.matches(&notification).unwrap().is_some())
        }
    }
}
