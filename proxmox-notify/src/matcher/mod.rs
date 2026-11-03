use std::collections::HashSet;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use tracing::{error, info};

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{Updater, api};

use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{Error, Notification, Origin};

pub mod calendar;
pub mod field;
pub mod severity;

use calendar::CalendarMatcher;
use field::InlineFieldMatcher;
use severity::InlineSeverityMatcher;

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
/// Config for notification matchers.
pub struct MatcherConfig {
    /// Name of the matcher.
    #[updater(skip)]
    pub name: String,

    /// List of matched metadata fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub match_field: Vec<InlineFieldMatcher>,

    /// List of matched severity levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub match_severity: Vec<InlineSeverityMatcher>,

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
    let mut matching_matchers = Vec::new();

    for matcher in matchers {
        if matcher.disable.unwrap_or_default() {
            // Skip this matcher if it is disabled
            info!("skipping disabled matcher '{name}'", name = matcher.name);
            continue;
        }

        match matcher.matches(notification) {
            Ok(Some(t)) => {
                matching_matchers.push(matcher.name.as_str());
                targets.extend(t.iter().map(|s| s.as_str()));
            }
            Ok(None) => {}
            Err(err) => error!("matcher '{matcher}' failed: {err}", matcher = matcher.name),
        }
    }

    if !matching_matchers.is_empty() {
        let target_list = Vec::from_iter(targets.iter().copied());

        info!(
            "notification matched the following matchers: {}. The following targets will be notified: {}",
            matching_matchers.join(", "),
            target_list.join(", ")
        );
    } else {
        info!("notification was not matched by any matcher");
    }

    targets
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::Severity;

    use super::*;

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
