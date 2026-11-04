use std::collections::HashSet;
use std::fmt::Debug;

use proxmox_match_expression::Expression;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use proxmox_api_macro::api;
use proxmox_schema::Updater;
use proxmox_schema::api_types::COMMENT_SCHEMA;

use crate::matcher::expression::NotificationMatcher;
use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{Error, Notification, Origin};

pub mod calendar;
pub mod expression;
pub mod field;
pub mod severity;

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
#[derive(Clone, Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for notification matchers.
pub struct MatcherConfig {
    /// Name of the matcher.
    #[updater(skip)]
    pub name: String,

    /// List of matched metadata fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "legacy-matchers")]
    pub match_field: Vec<field::InlineFieldMatcher>,

    /// List of matched severity levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "legacy-matchers")]
    pub match_severity: Vec<severity::InlineSeverityMatcher>,

    /// List of matched severity levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    #[cfg(feature = "legacy-matchers")]
    pub match_calendar: Vec<calendar::InlineCalendarMatcher>,
    /// Decide if 'all' or 'any' match statements must match.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "legacy-matchers")]
    pub mode: Option<MatchModeOperator>,

    /// Invert match of the whole filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "legacy-matchers")]
    pub invert_match: Option<bool>,

    /// Match expression as inline JSON. This option is mutually exclusive with
    /// the following options.
    ///
    ///   - match-field
    ///   - match-calendar
    ///   - match-severity
    ///   - invert-match
    ///   - mode
    ///
    /// All of the above can be represented as (sub)-expressions of this
    /// expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,

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
    /// Validate any additional constraints that cannot be expressed in the type system.
    fn validate(&self) -> Result<(), Error>;
}

impl MatcherConfig {
    pub fn matches(&self, notification: &Notification) -> Result<Option<&[String]>, Error> {
        #[cfg(feature = "legacy-matchers")]
        self.warn_about_invalid_configuration();

        let expression: Expression<NotificationMatcher> = {
            if let Some(expression_str) = &self.expression {
                serde_json::from_str(expression_str).map_err(|err| {
                    Error::FilterFailed(format!("could not deserialize filter expression: {err:#}"))
                })?
            } else {
                #[cfg(feature = "legacy-matchers")]
                {
                    self.generate_expression_from_legacy_config()
                }
                #[cfg(not(feature = "legacy-matchers"))]
                {
                    Expression::Constant(true)
                }
            }
        };

        // Later, once we have a notification history, we can also save the evaluated expression.
        // This gives the user a trace of which rules matched and which did not.
        let evaluated_expression = expression.evaluate(notification)?;

        Ok(evaluated_expression
            .is_match()
            .then_some(self.target.as_slice()))
    }

    #[cfg(feature = "legacy-matchers")]
    pub(crate) fn generate_expression_from_legacy_config(&self) -> Expression<NotificationMatcher> {
        if self.match_severity.is_empty()
            && self.match_field.is_empty()
            && self.match_calendar.is_empty()
        {
            // No match clauses means that the matcher always matches.
            // Also, 'invert-match' does not have any effect then.
            return Expression::Constant(true);
        }
        let mut expressions = Vec::new();

        for m in self.match_calendar.clone() {
            expressions.push(Expression::Match(NotificationMatcher::Calendar(
                m.into_inner(),
            )));
        }
        for m in self.match_field.clone() {
            expressions.push(Expression::Match(NotificationMatcher::Field(
                m.into_inner(),
            )));
        }
        for m in self.match_severity.clone() {
            expressions.push(Expression::Match(NotificationMatcher::Severity(
                m.into_inner(),
            )));
        }

        let root = match self.mode.unwrap_or_default() {
            MatchModeOperator::All => Expression::AllOf(expressions),
            MatchModeOperator::Any => Expression::AnyOf(expressions),
        };

        if self.invert_match.unwrap_or_default() {
            Expression::Not(Box::new(root))
        } else {
            root
        }
    }

    /// Ensure the validity of this matcher.
    pub(crate) fn ensure_valid(&mut self) -> Result<(), Error> {
        #[cfg(feature = "legacy-matchers")]
        self.ensure_no_ignored_properties()?;
        self.ensure_valid_expression()?;

        Ok(())
    }

    #[cfg(feature = "legacy-matchers")]
    fn ensure_no_ignored_properties(&self) -> Result<(), Error> {
        if self.expression.is_some() {
            let conflicting_properties = self.properties_conflicting_with_expression();

            if !conflicting_properties.is_empty() {
                let joined = conflicting_properties.join(", ");
                return Err(Error::Generic(format!(
                    "'expression' property is mutually exclusive with: {joined}"
                )));
            }
        }

        Ok(())
    }

    fn ensure_valid_expression(&mut self) -> Result<(), Error> {
        if let Some(expression) = &self.expression {
            match serde_json::from_str::<Expression<NotificationMatcher>>(expression) {
                Ok(expr) => {
                    Self::verify_match_expression_constraints(&expr)?;

                    // Storing the *serialized* expression makes sure that
                    // we have a minified, single-line JSON string
                    self.expression = Some(serde_json::to_string(&expr).map_err(|err| {
                        Error::Generic(format!("could not serialize expression: {err:#}"))
                    })?);
                }
                Err(err) => {
                    return Err(Error::Generic(format!(
                        "'expression' is not valid: {err:#}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Recursively walk expression nodes and verify any additional constraints on them:
    ///
    ///   - all-of/any-of/one-of *must* have child nodes:
    ///     The generic proxmox-match-expression crates handles empty combinators just fine,
    ///     however, from a UX perspective, it makes sense to require them to have child nodes.
    fn verify_match_expression_constraints(
        expr: &Expression<NotificationMatcher>,
    ) -> Result<(), Error> {
        match expr {
            Expression::AllOf(expressions) => {
                if expressions.is_empty() {
                    return Err(Error::Generic(
                        "'expression' may not contain an empty 'all-of' rule".to_string(),
                    ));
                }
            }
            Expression::AnyOf(expressions) => {
                if expressions.is_empty() {
                    return Err(Error::Generic(
                        "'expression' may not contain an empty 'any-of' rule".to_string(),
                    ));
                }
            }
            Expression::OneOf(expressions) => {
                if expressions.is_empty() {
                    return Err(Error::Generic(
                        "'expression' may not contain an empty 'one-of' rule".to_string(),
                    ));
                }
            }
            Expression::Match(matcher) => {
                matcher.validate()?;
            }
            _ => {}
        }

        for child_expression in expr.children() {
            Self::verify_match_expression_constraints(child_expression)?;
        }

        Ok(())
    }

    #[cfg(feature = "legacy-matchers")]
    fn warn_about_invalid_configuration(&self) {
        if self.expression.is_some() {
            let conflicting_properties = self.properties_conflicting_with_expression();

            if !conflicting_properties.is_empty() {
                let joined = conflicting_properties.join(", ");
                tracing::warn!(
                    "matcher '{}' has 'expression' set, the following properties are ignored: {joined}",
                    self.name
                );
            }
        }
    }

    #[cfg(feature = "legacy-matchers")]
    fn properties_conflicting_with_expression(&self) -> Vec<&'static str> {
        let mut conflicting_properties = Vec::new();

        if self.invert_match.is_some() {
            conflicting_properties.push("invert-match");
        }
        if self.mode.is_some() {
            conflicting_properties.push("mode");
        }
        if !self.match_field.is_empty() {
            conflicting_properties.push("match-field");
        }
        if !self.match_severity.is_empty() {
            conflicting_properties.push("match-severity");
        }
        if !self.match_calendar.is_empty() {
            conflicting_properties.push("match-calendar");
        }

        conflicting_properties
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
    #[cfg(feature = "legacy-matchers")]
    InvertMatch,
    /// Delete `match-calendar`
    #[cfg(feature = "legacy-matchers")]
    MatchCalendar,
    /// Delete `match-field`
    #[cfg(feature = "legacy-matchers")]
    MatchField,
    /// Delete `match-severity`
    #[cfg(feature = "legacy-matchers")]
    MatchSeverity,
    /// Delete `mode`
    #[cfg(feature = "legacy-matchers")]
    Mode,
    /// Delete `expression`
    Expression,
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
    use super::*;

    #[test]
    #[cfg(feature = "legacy-matchers")]
    fn test_empty_matcher_matches_always() {
        use super::*;
        use crate::Severity;
        use serde_json::Value;

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

    #[test]
    fn test_expression_constraints() {
        let empty_any_of = r#"
            {
              "any-of": []
            }"#;
        let empty_all_of = r#"
            {
              "all-of": []
            }"#;
        let empty_one_of = r#"
            {
              "one-of": []
            }"#;

        let empty_field_matcher = r#"
            {
              "match": {
                "type": "field",
                "field": "something",
                "values": []
              }
            }"#;

        let empty_severity_matcher = r#"
            {
              "match": {
                "type": "severity",
                "severities": []
              }
            }"#;

        for expr in [
            empty_one_of,
            empty_all_of,
            empty_any_of,
            empty_field_matcher,
            empty_severity_matcher,
        ] {
            let mut config = MatcherConfig {
                name: "matcher".to_string(),
                expression: Some(expr.into()),
                ..Default::default()
            };

            assert!(config.ensure_valid().is_err());
        }
    }

    #[test]
    fn test_invalid_fields() {
        let exact_field_matcher_template = r#"
            {
              "match": {
                "type": "field",
                "field": "%FIELD%",
                "values": ["abc"]
              }
            }"#;

        let regex_field_matcher_template = r#"
            {
              "match": {
                "type": "field",
                "field": "%FIELD%",
                "regex": "abc"
              }
            }"#;

        for (field, valid) in [
            ("some-field", true),
            ("some_field", true),
            ("some-", true),
            ("some.field", true),
            ("-some", false),
            ("some%field", false),
            ("", false),
        ] {
            for tpl in [exact_field_matcher_template, regex_field_matcher_template] {
                let mut config = MatcherConfig {
                    name: "matcher".to_string(),
                    expression: Some(tpl.replace("%FIELD%", field)),
                    ..Default::default()
                };

                assert_eq!(
                    config.ensure_valid().is_ok(),
                    valid,
                    "{field} should be {valid}"
                );
            }
        }
    }
}
