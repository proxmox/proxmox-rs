use proxmox_http_error::HttpError;

use crate::api::{http_bail, http_err};
use crate::group::{DeleteableGroupProperty, GroupConfig, GroupConfigUpdater, GROUP_TYPENAME};
use crate::Config;

/// Get all notification groups
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all groups or a `HttpError` if the config is
/// erroneous (`500 Internal server error`).
pub fn get_groups(config: &Config) -> Result<Vec<GroupConfig>, HttpError> {
    config
        .config
        .convert_to_typed_array(GROUP_TYPENAME)
        .map_err(|e| http_err!(INTERNAL_SERVER_ERROR, "Could not fetch groups: {e}"))
}

/// Get group with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or an `HttpError` if the group was not found (`404 Not found`).
pub fn get_group(config: &Config, name: &str) -> Result<GroupConfig, HttpError> {
    config
        .config
        .lookup(GROUP_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "group '{name}' not found"))
}

/// Add a new group.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - a referenced filter does not exist (`400 Bad request`)
///   - no endpoints were passed (`400 Bad request`)
///   - referenced endpoints do not exist (`404 Not found`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn add_group(config: &mut Config, group_config: &GroupConfig) -> Result<(), HttpError> {
    super::ensure_unique(config, &group_config.name)?;

    if group_config.endpoint.is_empty() {
        http_bail!(BAD_REQUEST, "group must contain at least one endpoint",);
    }

    if let Some(filter) = &group_config.filter {
        // Check if filter exists
        super::filter::get_filter(config, filter)?;
    }

    super::ensure_endpoints_exist(config, &group_config.endpoint)?;

    config
        .config
        .set_data(&group_config.name, GROUP_TYPENAME, group_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save group '{}': {e}",
                group_config.name
            )
        })
}

/// Update existing group
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - a referenced filter does not exist (`400 Bad request`)
///   - an invalid digest was passed (`400 Bad request`)
///   - no endpoints were passed (`400 Bad request`)
///   - referenced endpoints do not exist (`404 Not found`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn update_group(
    config: &mut Config,
    name: &str,
    updater: &GroupConfigUpdater,
    delete: Option<&[DeleteableGroupProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut group = get_group(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableGroupProperty::Comment => group.comment = None,
                DeleteableGroupProperty::Filter => group.filter = None,
            }
        }
    }

    if let Some(endpoints) = &updater.endpoint {
        super::ensure_endpoints_exist(config, endpoints)?;
        if endpoints.is_empty() {
            http_bail!(BAD_REQUEST, "group must contain at least one endpoint",);
        }
        group.endpoint = endpoints.iter().map(Into::into).collect()
    }

    if let Some(comment) = &updater.comment {
        group.comment = Some(comment.into());
    }

    if let Some(filter) = &updater.filter {
        // Check if filter exists
        let _ = super::filter::get_filter(config, filter)?;
        group.filter = Some(filter.into());
    }

    config
        .config
        .set_data(name, GROUP_TYPENAME, &group)
        .map_err(|e| http_err!(INTERNAL_SERVER_ERROR, "could not save group '{name}': {e}"))
}

/// Delete existing group
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if the group does not exist (`404 Not found`).
pub fn delete_group(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the group exists
    let _ = get_group(config, name)?;

    config.config.sections.remove(name);

    Ok(())
}

// groups cannot be empty, so only  build the tests if we have the
// sendmail endpoint available
#[cfg(all(test, feature = "sendmail"))]
mod tests {
    use super::*;
    use crate::api::sendmail::tests::add_sendmail_endpoint_for_test;
    use crate::api::test_helpers::*;

    fn add_default_group(config: &mut Config) -> Result<(), HttpError> {
        add_sendmail_endpoint_for_test(config, "test")?;

        add_group(
            config,
            &GroupConfig {
                name: "group1".into(),
                endpoint: vec!["test".to_string()],
                comment: None,
                filter: None,
            },
        )?;

        Ok(())
    }

    #[test]
    fn test_add_group_fails_if_endpoint_does_not_exist() {
        let mut config = empty_config();
        assert!(add_group(
            &mut config,
            &GroupConfig {
                name: "group1".into(),
                endpoint: vec!["foo".into()],
                comment: None,
                filter: None,
            },
        )
        .is_err());
    }

    #[test]
    fn test_add_group() -> Result<(), HttpError> {
        let mut config = empty_config();
        assert!(add_default_group(&mut config).is_ok());
        Ok(())
    }

    #[test]
    fn test_update_group_fails_if_endpoint_does_not_exist() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(update_group(
            &mut config,
            "group1",
            &GroupConfigUpdater {
                endpoint: Some(vec!["foo".into()]),
                ..Default::default()
            },
            None,
            None
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn test_update_group_fails_if_digest_invalid() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(update_group(
            &mut config,
            "group1",
            &Default::default(),
            None,
            Some(&[0u8; 32])
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn test_update_group() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(update_group(
            &mut config,
            "group1",
            &GroupConfigUpdater {
                endpoint: None,
                comment: Some("newcomment".into()),
                filter: None
            },
            None,
            None,
        )
        .is_ok());
        let group = get_group(&config, "group1")?;
        assert_eq!(group.comment, Some("newcomment".into()));

        assert!(update_group(
            &mut config,
            "group1",
            &Default::default(),
            Some(&[DeleteableGroupProperty::Comment]),
            None
        )
        .is_ok());
        let group = get_group(&config, "group1")?;
        assert_eq!(group.comment, None);

        Ok(())
    }

    #[test]
    fn test_group_delete() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(delete_group(&mut config, "group1").is_ok());
        assert!(delete_group(&mut config, "group1").is_err());

        Ok(())
    }
}
