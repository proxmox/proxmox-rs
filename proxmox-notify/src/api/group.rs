use crate::api::ApiError;
use crate::group::{DeleteableGroupProperty, GroupConfig, GroupConfigUpdater, GROUP_TYPENAME};
use crate::Config;

/// Get all notification groups
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all groups or an `ApiError` if the config is erroneous.
pub fn get_groups(config: &Config) -> Result<Vec<GroupConfig>, ApiError> {
    config
        .config
        .convert_to_typed_array(GROUP_TYPENAME)
        .map_err(|e| ApiError::internal_server_error("Could not fetch groups", Some(e.into())))
}

/// Get group with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or an `ApiError` if the group was not found.
pub fn get_group(config: &Config, name: &str) -> Result<GroupConfig, ApiError> {
    config
        .config
        .lookup(GROUP_TYPENAME, name)
        .map_err(|_| ApiError::not_found(format!("group '{name}' not found"), None))
}

/// Add a new group.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if a group with the same name already exists, or
/// if the group could not be saved
pub fn add_group(config: &mut Config, group_config: &GroupConfig) -> Result<(), ApiError> {
    if get_group(config, &group_config.name).is_ok() {
        return Err(ApiError::bad_request(
            format!("group '{}' already exists", group_config.name),
            None,
        ));
    }

    if group_config.endpoint.is_empty() {
        return Err(ApiError::bad_request(
            "group must contain at least one endpoint",
            None,
        ));
    }

    check_if_endpoints_exist(config, &group_config.endpoint)?;

    config
        .config
        .set_data(&group_config.name, GROUP_TYPENAME, group_config)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save group '{}'", group_config.name),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Update existing group
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the config could not be saved.
pub fn update_group(
    config: &mut Config,
    name: &str,
    updater: &GroupConfigUpdater,
    delete: Option<&[DeleteableGroupProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), ApiError> {
    super::verify_digest(config, digest)?;

    let mut group = get_group(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableGroupProperty::Comment => group.comment = None,
            }
        }
    }

    if let Some(endpoints) = &updater.endpoint {
        check_if_endpoints_exist(config, endpoints)?;
        if endpoints.is_empty() {
            return Err(ApiError::bad_request(
                "group must contain at least one endpoint",
                None,
            ));
        }
        group.endpoint = endpoints.iter().map(Into::into).collect()
    }

    if let Some(comment) = &updater.comment {
        group.comment = Some(comment.into());
    }

    config
        .config
        .set_data(name, GROUP_TYPENAME, &group)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save group '{name}'"),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Delete existing group
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the group does not exist.
pub fn delete_group(config: &mut Config, name: &str) -> Result<(), ApiError> {
    // Check if the group exists
    let _ = get_group(config, name)?;

    config.config.sections.remove(name);

    Ok(())
}

fn check_if_endpoints_exist(config: &Config, endpoints: &[String]) -> Result<(), ApiError> {
    for endpoint in endpoints {
        if !super::endpoint_exists(config, endpoint) {
            return Err(ApiError::not_found(
                format!("endoint '{endpoint}' does not exist"),
                None,
            ));
        }
    }

    Ok(())
}

// groups cannot be empty, so only  build the tests if we have the
// sendmail endpoint available
#[cfg(all(test, feature = "sendmail"))]
mod tests {
    use super::*;
    use crate::api::sendmail::tests::add_sendmail_endpoint_for_test;
    use crate::api::test_helpers::*;

    fn add_default_group(config: &mut Config) -> Result<(), ApiError> {
        add_sendmail_endpoint_for_test(config, "test")?;

        add_group(
            config,
            &GroupConfig {
                name: "group1".into(),
                endpoint: vec!["test".to_string()],
                comment: None,
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
            },
        )
        .is_err());
    }

    #[test]
    fn test_add_group() -> Result<(), ApiError> {
        let mut config = empty_config();
        assert!(add_default_group(&mut config).is_ok());
        Ok(())
    }

    #[test]
    fn test_update_group_fails_if_endpoint_does_not_exist() -> Result<(), ApiError> {
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
    fn test_update_group_fails_if_digest_invalid() -> Result<(), ApiError> {
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
    fn test_update_group() -> Result<(), ApiError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(update_group(
            &mut config,
            "group1",
            &GroupConfigUpdater {
                endpoint: None,
                comment: Some("newcomment".into())
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
    fn test_group_delete() -> Result<(), ApiError> {
        let mut config = empty_config();
        add_default_group(&mut config)?;

        assert!(delete_group(&mut config, "group1").is_ok());
        assert!(delete_group(&mut config, "group1").is_err());

        Ok(())
    }
}
