use proxmox_http_error::HttpError;

use crate::api::http_err;
use crate::matcher::{
    DeleteableMatcherProperty, MatcherConfig, MatcherConfigUpdater, MATCHER_TYPENAME,
};
use crate::Config;

/// Get a list of all matchers
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all matchers or a `HttpError` if the config is
/// (`500 Internal server error`).
pub fn get_matchers(config: &Config) -> Result<Vec<MatcherConfig>, HttpError> {
    config
        .config
        .convert_to_typed_array(MATCHER_TYPENAME)
        .map_err(|e| http_err!(INTERNAL_SERVER_ERROR, "Could not fetch matchers: {e}"))
}

/// Get matcher with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or a `HttpError` if the matcher was not found (`404 Not found`).
pub fn get_matcher(config: &Config, name: &str) -> Result<MatcherConfig, HttpError> {
    config
        .config
        .lookup(MATCHER_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "matcher '{name}' not found"))
}

/// Add new notification matcher.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn add_matcher(config: &mut Config, matcher_config: &MatcherConfig) -> Result<(), HttpError> {
    super::ensure_unique(config, &matcher_config.name)?;

    if let Some(targets) = matcher_config.target.as_deref() {
        super::ensure_endpoints_exist(config, targets)?;
    }

    config
        .config
        .set_data(&matcher_config.name, MATCHER_TYPENAME, matcher_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save matcher '{}': {e}",
                matcher_config.name
            )
        })?;

    Ok(())
}

/// Update existing notification matcher
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the configuration could not be saved (`500 Internal server error`)
///   - an invalid digest was passed (`400 Bad request`)
pub fn update_matcher(
    config: &mut Config,
    name: &str,
    matcher_updater: &MatcherConfigUpdater,
    delete: Option<&[DeleteableMatcherProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut matcher = get_matcher(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableMatcherProperty::MatchSeverity => matcher.match_severity = None,
                DeleteableMatcherProperty::MatchField => matcher.match_field = None,
                DeleteableMatcherProperty::MatchCalendar => matcher.match_calendar = None,
                DeleteableMatcherProperty::Target => matcher.target = None,
                DeleteableMatcherProperty::Mode => matcher.mode = None,
                DeleteableMatcherProperty::InvertMatch => matcher.invert_match = None,
                DeleteableMatcherProperty::Comment => matcher.comment = None,
                DeleteableMatcherProperty::Disable => matcher.disable = None,
            }
        }
    }

    if let Some(match_severity) = &matcher_updater.match_severity {
        matcher.match_severity = Some(match_severity.clone());
    }

    if let Some(match_field) = &matcher_updater.match_field {
        matcher.match_field = Some(match_field.clone());
    }

    if let Some(match_calendar) = &matcher_updater.match_calendar {
        matcher.match_calendar = Some(match_calendar.clone());
    }

    if let Some(mode) = matcher_updater.mode {
        matcher.mode = Some(mode);
    }

    if let Some(invert_match) = matcher_updater.invert_match {
        matcher.invert_match = Some(invert_match);
    }

    if let Some(comment) = &matcher_updater.comment {
        matcher.comment = Some(comment.into());
    }

    if let Some(disable) = &matcher_updater.disable {
        matcher.disable = Some(*disable);
    }

    if let Some(target) = &matcher_updater.target {
        super::ensure_endpoints_exist(config, target.as_slice())?;
        matcher.target = Some(target.clone());
    }

    config
        .config
        .set_data(name, MATCHER_TYPENAME, &matcher)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save matcher '{name}': {e}"
            )
        })?;

    Ok(())
}

/// Delete existing matcher
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the entity does not exist (`404 Not found`)
pub fn delete_matcher(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the matcher exists
    let _ = get_matcher(config, name)?;

    config.config.sections.remove(name);

    Ok(())
}

#[cfg(all(test, feature = "sendmail", feature = "pve-context"))]
mod tests {
    use super::*;
    use crate::matcher::MatchModeOperator;

    fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }

    fn config_with_two_matchers() -> Config {
        Config::new(
            "
sendmail: foo
    mailto test@example.com

matcher: matcher1

matcher: matcher2
",
            "",
        )
        .unwrap()
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();
        assert!(update_matcher(&mut config, "test", &Default::default(), None, None).is_err());
        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), HttpError> {
        let mut config = config_with_two_matchers();
        assert!(update_matcher(
            &mut config,
            "matcher1",
            &Default::default(),
            None,
            Some(&[0u8; 32])
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_matcher_update() -> Result<(), HttpError> {
        let mut config = config_with_two_matchers();

        let digest = config.digest;

        update_matcher(
            &mut config,
            "matcher1",
            &MatcherConfigUpdater {
                mode: Some(MatchModeOperator::Any),
                match_field: None,
                match_severity: None,
                match_calendar: None,
                invert_match: Some(true),
                target: Some(vec!["foo".into()]),
                comment: Some("new comment".into()),
                ..Default::default()
            },
            None,
            Some(&digest),
        )?;

        let matcher = get_matcher(&config, "matcher1")?;

        assert!(matches!(matcher.mode, Some(MatchModeOperator::Any)));
        assert_eq!(matcher.invert_match, Some(true));
        assert_eq!(matcher.comment, Some("new comment".into()));

        // Test property deletion
        update_matcher(
            &mut config,
            "matcher1",
            &Default::default(),
            Some(&[
                DeleteableMatcherProperty::InvertMatch,
                DeleteableMatcherProperty::Mode,
                DeleteableMatcherProperty::MatchField,
                DeleteableMatcherProperty::Target,
                DeleteableMatcherProperty::Comment,
            ]),
            Some(&digest),
        )?;

        let matcher = get_matcher(&config, "matcher1")?;

        assert_eq!(matcher.invert_match, None);
        assert!(matcher.match_severity.is_none());
        assert!(matches!(matcher.match_field, None));
        assert_eq!(matcher.target, None);
        assert!(matcher.mode.is_none());
        assert_eq!(matcher.comment, None);

        Ok(())
    }

    #[test]
    fn test_matcher_delete() -> Result<(), HttpError> {
        let mut config = config_with_two_matchers();

        delete_matcher(&mut config, "matcher1")?;
        assert!(delete_matcher(&mut config, "matcher1").is_err());

        Ok(())
    }
}
