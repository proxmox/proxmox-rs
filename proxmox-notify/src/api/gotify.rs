use proxmox_http_error::HttpError;

use crate::api::http_err;
use crate::endpoints::gotify::{
    DeleteableGotifyProperty, GotifyConfig, GotifyConfigUpdater, GotifyPrivateConfig,
    GotifyPrivateConfigUpdater, GOTIFY_TYPENAME,
};
use crate::Config;

/// Get a list of all gotify endpoints.
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all gotify endpoints or a `HttpError` if the config is
/// erroneous (`500 Internal server error`).
pub fn get_endpoints(config: &Config) -> Result<Vec<GotifyConfig>, HttpError> {
    config
        .config
        .convert_to_typed_array(GOTIFY_TYPENAME)
        .map_err(|e| http_err!(NOT_FOUND, "Could not fetch endpoints: {e}"))
}

/// Get gotify endpoint with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or a `HttpError` if the endpoint was not found (`404 Not found`).
pub fn get_endpoint(config: &Config, name: &str) -> Result<GotifyConfig, HttpError> {
    config
        .config
        .lookup(GOTIFY_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "endpoint '{name}' not found"))
}

/// Add a new gotify endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
///
/// Panics if the names of the private config and the public config do not match.
pub fn add_endpoint(
    config: &mut Config,
    endpoint_config: GotifyConfig,
    private_endpoint_config: GotifyPrivateConfig,
) -> Result<(), HttpError> {
    if endpoint_config.name != private_endpoint_config.name {
        // Programming error by the user of the crate, thus we panic
        panic!("name for endpoint config and private config must be identical");
    }

    super::ensure_unique(config, &endpoint_config.name)?;

    set_private_config_entry(config, &private_endpoint_config)?;

    config
        .config
        .set_data(&endpoint_config.name, GOTIFY_TYPENAME, &endpoint_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint_config.name
            )
        })
}

/// Update existing gotify endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn update_endpoint(
    config: &mut Config,
    name: &str,
    endpoint_config_updater: GotifyConfigUpdater,
    private_endpoint_config_updater: GotifyPrivateConfigUpdater,
    delete: Option<&[DeleteableGotifyProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut endpoint = get_endpoint(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableGotifyProperty::Comment => endpoint.comment = None,
                DeleteableGotifyProperty::Disable => endpoint.disable = None,
            }
        }
    }

    if let Some(server) = endpoint_config_updater.server {
        endpoint.server = server;
    }

    if let Some(token) = private_endpoint_config_updater.token {
        set_private_config_entry(
            config,
            &GotifyPrivateConfig {
                name: name.into(),
                token: token.into(),
            },
        )?;
    }

    if let Some(comment) = endpoint_config_updater.comment {
        endpoint.comment = Some(comment)
    }

    if let Some(disable) = endpoint_config_updater.disable {
        endpoint.disable = Some(disable);
    }

    config
        .config
        .set_data(name, GOTIFY_TYPENAME, &endpoint)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{name}': {e}"
            )
        })
}

/// Delete existing gotify endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the entity does not exist (`404 Not found`)
///   - the endpoint is still referenced by another entity (`400 Bad request`)
pub fn delete_gotify_endpoint(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the endpoint exists
    let _ = get_endpoint(config, name)?;
    super::ensure_safe_to_delete(config, name)?;

    remove_private_config_entry(config, name)?;
    config.config.sections.remove(name);

    Ok(())
}

fn set_private_config_entry(
    config: &mut Config,
    private_config: &GotifyPrivateConfig,
) -> Result<(), HttpError> {
    config
        .private_config
        .set_data(&private_config.name, GOTIFY_TYPENAME, private_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save private config for endpoint '{}': {e}",
                private_config.name
            )
        })
}

fn remove_private_config_entry(config: &mut Config, name: &str) -> Result<(), HttpError> {
    config.private_config.sections.remove(name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::test_helpers::empty_config;

    pub fn add_default_gotify_endpoint(config: &mut Config) -> Result<(), HttpError> {
        add_endpoint(
            config,
            GotifyConfig {
                name: "gotify-endpoint".into(),
                server: "localhost".into(),
                comment: Some("comment".into()),
                ..Default::default()
            },
            GotifyPrivateConfig {
                name: "gotify-endpoint".into(),
                token: "supersecrettoken".into(),
            },
        )?;

        assert!(get_endpoint(config, "gotify-endpoint").is_ok());
        Ok(())
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();

        assert!(update_endpoint(
            &mut config,
            "test",
            Default::default(),
            Default::default(),
            None,
            None
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        assert!(update_endpoint(
            &mut config,
            "gotify-endpoint",
            Default::default(),
            Default::default(),
            None,
            Some(&[0; 32])
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_gotify_update() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        let digest = config.digest;

        update_endpoint(
            &mut config,
            "gotify-endpoint",
            GotifyConfigUpdater {
                server: Some("newhost".into()),
                comment: Some("newcomment".into()),
                ..Default::default()
            },
            GotifyPrivateConfigUpdater {
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
            Default::default(),
            Default::default(),
            Some(&[DeleteableGotifyProperty::Comment]),
            None,
        )?;

        let endpoint = get_endpoint(&config, "gotify-endpoint")?;
        assert_eq!(endpoint.comment, None);

        Ok(())
    }

    #[test]
    fn test_gotify_endpoint_delete() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_gotify_endpoint(&mut config)?;

        delete_gotify_endpoint(&mut config, "gotify-endpoint")?;
        assert!(delete_gotify_endpoint(&mut config, "gotify-endpoint").is_err());
        assert_eq!(get_endpoints(&config)?.len(), 0);

        Ok(())
    }
}
