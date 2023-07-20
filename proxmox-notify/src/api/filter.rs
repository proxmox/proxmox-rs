use crate::api::ApiError;
use crate::filter::{DeleteableFilterProperty, FilterConfig, FilterConfigUpdater, FILTER_TYPENAME};
use crate::Config;

/// Get a list of all filters
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all filters or an `ApiError` if the config is erroneous.
pub fn get_filters(config: &Config) -> Result<Vec<FilterConfig>, ApiError> {
    config
        .config
        .convert_to_typed_array(FILTER_TYPENAME)
        .map_err(|e| ApiError::internal_server_error("Could not fetch filters", Some(e.into())))
}

/// Get filter with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or an `ApiError` if the filter was not found.
pub fn get_filter(config: &Config, name: &str) -> Result<FilterConfig, ApiError> {
    config
        .config
        .lookup(FILTER_TYPENAME, name)
        .map_err(|_| ApiError::not_found(format!("filter '{name}' not found"), None))
}

/// Add new notification filter.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if a filter with the same name already exists or
/// if the filter could not be saved.
pub fn add_filter(config: &mut Config, filter_config: &FilterConfig) -> Result<(), ApiError> {
    if get_filter(config, &filter_config.name).is_ok() {
        return Err(ApiError::bad_request(
            format!("filter '{}' already exists", filter_config.name),
            None,
        ));
    }

    config
        .config
        .set_data(&filter_config.name, FILTER_TYPENAME, filter_config)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save filter '{}'", filter_config.name),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Update existing filter
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the config could not be saved.
pub fn update_filter(
    config: &mut Config,
    name: &str,
    filter_updater: &FilterConfigUpdater,
    delete: Option<&[DeleteableFilterProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), ApiError> {
    super::verify_digest(config, digest)?;

    let mut filter = get_filter(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableFilterProperty::MinSeverity => filter.min_severity = None,
                DeleteableFilterProperty::Mode => filter.mode = None,
                DeleteableFilterProperty::InvertMatch => filter.invert_match = None,
                DeleteableFilterProperty::Comment => filter.comment = None,
            }
        }
    }

    if let Some(min_severity) = filter_updater.min_severity {
        filter.min_severity = Some(min_severity);
    }

    if let Some(mode) = filter_updater.mode {
        filter.mode = Some(mode);
    }

    if let Some(invert_match) = filter_updater.invert_match {
        filter.invert_match = Some(invert_match);
    }

    if let Some(comment) = &filter_updater.comment {
        filter.comment = Some(comment.into());
    }

    config
        .config
        .set_data(name, FILTER_TYPENAME, &filter)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save filter '{name}'"),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Delete existing filter
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the filter does not exist.
pub fn delete_filter(config: &mut Config, name: &str) -> Result<(), ApiError> {
    // Check if the filter exists
    let _ = get_filter(config, name)?;
    super::ensure_unused(config, name)?;

    config.config.sections.remove(name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::FilterModeOperator;
    use crate::Severity;

    fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }

    fn config_with_two_filters() -> Config {
        Config::new(
            "
filter: filter1
    min-severity info

filter: filter2
    min-severity warning
",
            "",
        )
        .unwrap()
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), ApiError> {
        let mut config = empty_config();
        assert!(update_filter(&mut config, "test", &Default::default(), None, None).is_err());
        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), ApiError> {
        let mut config = config_with_two_filters();
        assert!(update_filter(
            &mut config,
            "filter1",
            &Default::default(),
            None,
            Some(&[0u8; 32])
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_filter_update() -> Result<(), ApiError> {
        let mut config = config_with_two_filters();

        let digest = config.digest;

        update_filter(
            &mut config,
            "filter1",
            &FilterConfigUpdater {
                min_severity: Some(Severity::Error),
                mode: Some(FilterModeOperator::Or),
                invert_match: Some(true),
                comment: Some("new comment".into()),
            },
            None,
            Some(&digest),
        )?;

        let filter = get_filter(&config, "filter1")?;

        assert!(matches!(filter.mode, Some(FilterModeOperator::Or)));
        assert!(matches!(filter.min_severity, Some(Severity::Error)));
        assert_eq!(filter.invert_match, Some(true));
        assert_eq!(filter.comment, Some("new comment".into()));

        // Test property deletion
        update_filter(
            &mut config,
            "filter1",
            &Default::default(),
            Some(&[
                DeleteableFilterProperty::InvertMatch,
                DeleteableFilterProperty::Mode,
                DeleteableFilterProperty::InvertMatch,
                DeleteableFilterProperty::MinSeverity,
                DeleteableFilterProperty::Comment,
            ]),
            Some(&digest),
        )?;

        let filter = get_filter(&config, "filter1")?;

        assert_eq!(filter.invert_match, None);
        assert_eq!(filter.min_severity, None);
        assert!(matches!(filter.mode, None));
        assert_eq!(filter.comment, None);

        Ok(())
    }

    #[test]
    fn test_filter_delete() -> Result<(), ApiError> {
        let mut config = config_with_two_filters();

        delete_filter(&mut config, "filter1")?;
        assert!(delete_filter(&mut config, "filter1").is_err());
        assert_eq!(get_filters(&config)?.len(), 1);

        Ok(())
    }
}
