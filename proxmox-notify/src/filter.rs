use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{Error, Notification, Severity};

pub const FILTER_TYPENAME: &str = "filter";

#[api]
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum FilterModeOperator {
    /// All filter properties have to match (AND)
    #[default]
    And,
    /// At least one filter property has to match (OR)
    Or,
}

impl FilterModeOperator {
    /// Apply the mode operator to two bools, lhs and rhs
    fn apply(&self, lhs: bool, rhs: bool) -> bool {
        match self {
            FilterModeOperator::And => lhs && rhs,
            FilterModeOperator::Or => lhs || rhs,
        }
    }

    fn neutral_element(&self) -> bool {
        match self {
            FilterModeOperator::And => true,
            FilterModeOperator::Or => false,
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
    })]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for Sendmail notification endpoints
pub struct FilterConfig {
    /// Name of the filter
    #[updater(skip)]
    pub name: String,

    /// Minimum severity to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_severity: Option<Severity>,

    /// Choose between 'and' and 'or' for when multiple properties are specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<FilterModeOperator>,

    /// Invert match of the whole filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert_match: Option<bool>,

    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableFilterProperty {
    MinSeverity,
    Mode,
    InvertMatch,
    Comment,
}

/// A caching, lazily-evaluating notification filter. Parameterized with the notification itself,
/// since there are usually multiple filters to check for a single notification that is to be sent.
pub(crate) struct FilterMatcher<'a> {
    filters: HashMap<&'a str, &'a FilterConfig>,
    cached_results: HashMap<&'a str, bool>,
    notification: &'a Notification,
}

impl<'a> FilterMatcher<'a> {
    pub(crate) fn new(filters: &'a [FilterConfig], notification: &'a Notification) -> Self {
        let filters = filters.iter().map(|f| (f.name.as_str(), f)).collect();

        Self {
            filters,
            cached_results: Default::default(),
            notification,
        }
    }

    /// Check if the notification that was used to instantiate Self matches a given filter
    pub(crate) fn check_filter_match(&mut self, filter_name: &str) -> Result<bool, Error> {
        let mut visited = HashSet::new();

        self.do_check_filter(filter_name, &mut visited)
    }

    fn do_check_filter(
        &mut self,
        filter_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<bool, Error> {
        if visited.contains(filter_name) {
            return Err(Error::FilterFailed(format!(
                "recursive filter definition: {filter_name}"
            )));
        }

        if let Some(is_match) = self.cached_results.get(filter_name) {
            return Ok(*is_match);
        }

        visited.insert(filter_name.into());

        let filter_config =
            self.filters.get(filter_name).copied().ok_or_else(|| {
                Error::FilterFailed(format!("filter '{filter_name}' does not exist"))
            })?;

        let invert_match = filter_config.invert_match.unwrap_or_default();

        let mode_operator = filter_config.mode.unwrap_or_default();

        let mut notification_matches = mode_operator.neutral_element();

        notification_matches = mode_operator.apply(
            notification_matches,
            self.check_severity_match(filter_config, mode_operator),
        );

        Ok(notification_matches != invert_match)
    }

    fn check_severity_match(
        &self,
        filter_config: &FilterConfig,
        mode_operator: FilterModeOperator,
    ) -> bool {
        if let Some(min_severity) = filter_config.min_severity {
            self.notification.severity >= min_severity
        } else {
            mode_operator.neutral_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config, Content};

    fn parse_filters(config: &str) -> Result<Vec<FilterConfig>, Error> {
        let (config, _) = config::config(config)?;
        Ok(config.convert_to_typed_array("filter").unwrap())
    }

    fn empty_notification_with_severity(severity: Severity) -> Notification {
        Notification {
            content: Content::Template {
                title_template: String::new(),
                body_template: String::new(),
                data: Default::default(),
            },
            severity,
        }
    }

    #[test]
    fn test_trivial_severity_filters() -> Result<(), Error> {
        let config = "
filter: test
    min-severity warning
";

        let filters = parse_filters(config)?;

        let is_match = |severity| {
            let notification = empty_notification_with_severity(severity);
            let mut results = FilterMatcher::new(&filters, &notification);
            results.check_filter_match("test")
        };

        assert!(is_match(Severity::Warning)?);
        assert!(!is_match(Severity::Notice)?);
        assert!(is_match(Severity::Error)?);

        Ok(())
    }
}
