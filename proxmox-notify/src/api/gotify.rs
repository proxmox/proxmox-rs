use crate::api::ApiError;
use crate::endpoints::gotify::{
    DeleteableGotifyProperty, GotifyConfig, GotifyConfigUpdater, GotifyPrivateConfig,
    GotifyPrivateConfigUpdater, GOTIFY_TYPENAME,
};
use crate::Config;

/// Get a list of all gotify endpoints.
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all gotify endpoints or an `ApiError` if the config is erroneous.
pub fn get_endpoints(config: &Config) -> Result<Vec<GotifyConfig>, ApiError> {
    config
        .config
        .convert_to_typed_array(GOTIFY_TYPENAME)
        .map_err(|e| ApiError::internal_server_error("Could not fetch endpoints", Some(e.into())))
}

/// Get gotify endpoint with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or an `ApiError` if the endpoint was not found.
pub fn get_endpoint(config: &Config, name: &str) -> Result<GotifyConfig, ApiError> {
    config
        .config
        .lookup(GOTIFY_TYPENAME, name)
        .map_err(|_| ApiError::not_found(format!("endpoint '{name}' not found"), None))
}

/// Add a new gotify endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if an endpoint with the same name already exists,
/// or if the endpoint could not be saved.
pub fn add_endpoint(
    config: &mut Config,
    endpoint_config: &GotifyConfig,
    private_endpoint_config: &GotifyPrivateConfig,
) -> Result<(), ApiError> {
    if endpoint_config.name != private_endpoint_config.name {
        // Programming error by the user of the crate, thus we panic
        panic!("name for endpoint config and private config must be identical");
    }

    if super::endpoint_exists(config, &endpoint_config.name) {
        return Err(ApiError::bad_request(
            format!(
                "endpoint with name '{}' already exists!",
                endpoint_config.name
            ),
            None,
        ));
    }

    if let Some(filter) = &endpoint_config.filter {
        // Check if filter exists
        super::filter::get_filter(config, filter)?;
    }

    set_private_config_entry(config, private_endpoint_config)?;

    config
        .config
        .set_data(&endpoint_config.name, GOTIFY_TYPENAME, endpoint_config)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save endpoint '{}'", endpoint_config.name),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Update existing gotify endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the config could not be saved.
pub fn update_endpoint(
    config: &mut Config,
    name: &str,
    endpoint_config_updater: &GotifyConfigUpdater,
    private_endpoint_config_updater: &GotifyPrivateConfigUpdater,
    delete: Option<&[DeleteableGotifyProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), ApiError> {
    super::verify_digest(config, digest)?;

    let mut endpoint = get_endpoint(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableGotifyProperty::Comment => endpoint.comment = None,
                DeleteableGotifyProperty::Filter => endpoint.filter = None,
            }
        }
    }

    if let Some(server) = &endpoint_config_updater.server {
        endpoint.server = server.into();
    }

    if let Some(token) = &private_endpoint_config_updater.token {
        set_private_config_entry(
            config,
            &GotifyPrivateConfig {
                name: name.into(),
                token: token.into(),
            },
        )?;
    }

    if let Some(comment) = &endpoint_config_updater.comment {
        endpoint.comment = Some(comment.into());
    }

    if let Some(filter) = &endpoint_config_updater.filter {
        // Check if filter exists
        let _ = super::filter::get_filter(config, filter)?;

        endpoint.filter = Some(filter.into());
    }

    config
        .config
        .set_data(name, GOTIFY_TYPENAME, &endpoint)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!("could not save endpoint '{name}'"),
                Some(e.into()),
            )
        })?;

    Ok(())
}

/// Delete existing gotify endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns an `ApiError` if the endpoint does not exist.
pub fn delete_gotify_endpoint(config: &mut Config, name: &str) -> Result<(), ApiError> {
    // Check if the endpoint exists
    let _ = get_endpoint(config, name)?;
    super::ensure_unused(config, name)?;

    remove_private_config_entry(config, name)?;
    config.config.sections.remove(name);

    Ok(())
}

fn set_private_config_entry(
    config: &mut Config,
    private_config: &GotifyPrivateConfig,
) -> Result<(), ApiError> {
    config
        .private_config
        .set_data(&private_config.name, GOTIFY_TYPENAME, private_config)
        .map_err(|e| {
            ApiError::internal_server_error(
                format!(
                    "could not save private config for endpoint '{}'",
                    private_config.name
                ),
                Some(e.into()),
            )
        })
}

fn remove_private_config_entry(config: &mut Config, name: &str) -> Result<(), ApiError> {
    config.private_config.sections.remove(name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::test_helpers::empty_config;

    pub fn add_default_gotify_endpoint(config: &mut Config) -> Result<(), ApiError> {
        add_endpoint(
            config,
            &GotifyConfig {
                name: "gotify-endpoint".into(),
                server: "localhost".into(),
                comment: Some("comment".into()),
                filter: None,
            },
            &GotifyPrivateConfig {
                name: "gotify-endpoint".into(),
                token: "supersecrettoken".into(),
            },
        )?;

        assert!(get_endpoint(config, "gotify-endpoint").is_ok());
        Ok(())
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), ApiError> {
        let mut config = empty_config();

        assert!(update_endpoint(
            &mut config,
            "test",
            &Default::default(),
            &Default::default(),
            None,
            None
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), ApiError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        assert!(update_endpoint(
            &mut config,
            "gotify-endpoint",
            &Default::default(),
            &Default::default(),
            None,
            Some(&[0; 32])
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_gotify_update() -> Result<(), ApiError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        let digest = config.digest;

        update_endpoint(
            &mut config,
            "gotify-endpoint",
            &GotifyConfigUpdater {
                server: Some("newhost".into()),
                comment: Some("newcomment".into()),
                filter: None,
            },
            &GotifyPrivateConfigUpdater {
                token: Some("changedtoken".into()),
            },
            None,
            Some(&digest),
        )?;

        let endpoint = get_endpoint(&config, "gotify-endpoint")?;

        assert_eq!(endpoint.server, "newhost".to_string());

        let token = config
            .private_config
            .lookup::<GotifyPrivateConfig>(GOTIFY_TYPENAME, "gotify-endpoint")
            .unwrap()
            .token;

        assert_eq!(token, "changedtoken".to_string());
        assert_eq!(endpoint.comment, Some("newcomment".to_string()));

        // Test property deletion
        update_endpoint(
            &mut config,
            "gotify-endpoint",
            &Default::default(),
            &Default::default(),
            Some(&[DeleteableGotifyProperty::Comment]),
            None,
        )?;

        let endpoint = get_endpoint(&config, "gotify-endpoint")?;
        assert_eq!(endpoint.comment, None);

        Ok(())
    }

    #[test]
    fn test_gotify_endpoint_delete() -> Result<(), ApiError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        delete_gotify_endpoint(&mut config, "gotify-endpoint")?;
        assert!(delete_gotify_endpoint(&mut config, "gotify-endpoint").is_err());
        assert_eq!(get_endpoints(&config)?.len(), 0);

        Ok(())
    }
}
