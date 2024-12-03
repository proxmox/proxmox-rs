//! CRUD API for webhook targets.
//!
//! All methods assume that the caller has already done any required permission checks.

use proxmox_http_error::HttpError;
use proxmox_schema::property_string::PropertyString;

use crate::api::http_err;
use crate::endpoints::webhook::{
    DeleteableWebhookProperty, KeyAndBase64Val, WebhookConfig, WebhookConfigUpdater,
    WebhookPrivateConfig, WEBHOOK_TYPENAME,
};
use crate::{http_bail, Config};

use super::remove_private_config_entry;
use super::set_private_config_entry;

/// Get a list of all webhook endpoints.
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all webhook endpoints or a [`HttpError`] if the config is
/// erroneous (`500 Internal server error`).
pub fn get_endpoints(config: &Config) -> Result<Vec<WebhookConfig>, HttpError> {
    let mut endpoints: Vec<WebhookConfig> = config
        .config
        .convert_to_typed_array(WEBHOOK_TYPENAME)
        .map_err(|e| http_err!(NOT_FOUND, "Could not fetch endpoints: {e}"))?;

    for endpoint in &mut endpoints {
        let priv_config: WebhookPrivateConfig = config
            .private_config
            .lookup(WEBHOOK_TYPENAME, &endpoint.name)
            .unwrap_or_default();

        let mut secret_names = Vec::new();
        // We only return *which* secrets we have stored, but not their values.
        for secret in priv_config.secret {
            secret_names.push(
                KeyAndBase64Val {
                    name: secret.name.clone(),
                    value: None,
                }
                .into(),
            )
        }

        endpoint.secret = secret_names;
    }

    Ok(endpoints)
}

/// Get webhook endpoint with given `name`
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or a [`HttpError`] if the endpoint was not found (`404 Not found`).
pub fn get_endpoint(config: &Config, name: &str) -> Result<WebhookConfig, HttpError> {
    let mut endpoint: WebhookConfig = config
        .config
        .lookup(WEBHOOK_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "endpoint '{name}' not found"))?;

    let priv_config: Option<WebhookPrivateConfig> = config
        .private_config
        .lookup(WEBHOOK_TYPENAME, &endpoint.name)
        .ok();

    let mut secret_names = Vec::new();
    if let Some(priv_config) = priv_config {
        for secret in &priv_config.secret {
            secret_names.push(
                KeyAndBase64Val {
                    name: secret.name.clone(),
                    value: None,
                }
                .into(),
            );
        }
    }

    endpoint.secret = secret_names;

    Ok(endpoint)
}

/// Add a new webhook endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a [`HttpError`] if:
///   - the target name is already used (`400 Bad request`)
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn add_endpoint(
    config: &mut Config,
    mut endpoint_config: WebhookConfig,
) -> Result<(), HttpError> {
    super::ensure_unique(config, &endpoint_config.name)?;

    let secrets = std::mem::take(&mut endpoint_config.secret);

    set_private_config_entry(
        config,
        &WebhookPrivateConfig {
            name: endpoint_config.name.clone(),
            secret: secrets,
        },
        WEBHOOK_TYPENAME,
        &endpoint_config.name,
    )?;

    config
        .config
        .set_data(&endpoint_config.name, WEBHOOK_TYPENAME, &endpoint_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint_config.name
            )
        })
}

/// Update existing webhook endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the passed `digest` does not match (`400 Bad request`)
///   - parameters are ill-formed (empty header value, invalid base64, unknown header/secret)
///     (`400 Bad request`)
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn update_endpoint(
    config: &mut Config,
    name: &str,
    config_updater: WebhookConfigUpdater,
    delete: Option<&[DeleteableWebhookProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut endpoint = get_endpoint(config, name)?;
    endpoint.secret.clear();

    let old_secrets = config
        .private_config
        .lookup::<WebhookPrivateConfig>(WEBHOOK_TYPENAME, name)
        .map_err(|err| http_err!(INTERNAL_SERVER_ERROR, "could not read secret config: {err}"))?
        .secret;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableWebhookProperty::Comment => endpoint.comment = None,
                DeleteableWebhookProperty::Disable => endpoint.disable = None,
                DeleteableWebhookProperty::Header => endpoint.header = Vec::new(),
                DeleteableWebhookProperty::Body => endpoint.body = None,
                DeleteableWebhookProperty::Secret => {
                    set_private_config_entry(
                        config,
                        &WebhookPrivateConfig {
                            name: name.into(),
                            secret: Vec::new(),
                        },
                        WEBHOOK_TYPENAME,
                        name,
                    )?;
                }
            }
        }
    }

    // Destructuring makes sure we don't forget any members
    let WebhookConfigUpdater {
        url,
        body,
        header,
        method,
        disable,
        comment,
        secret,
    } = config_updater;

    if let Some(url) = url {
        endpoint.url = url;
    }

    if let Some(body) = body {
        endpoint.body = Some(body);
    }

    if let Some(header) = header {
        for h in &header {
            if h.value.is_none() {
                http_bail!(BAD_REQUEST, "header '{}' has empty value", h.name);
            }
            if h.decode_value().is_err() {
                http_bail!(
                    BAD_REQUEST,
                    "header '{}' does not have valid base64 encoded data",
                    h.name
                )
            }
        }
        endpoint.header = header;
    }

    if let Some(method) = method {
        endpoint.method = method;
    }

    if let Some(disable) = disable {
        endpoint.disable = Some(disable);
    }

    if let Some(comment) = comment {
        endpoint.comment = Some(comment);
    }

    if let Some(secret) = secret {
        let mut new_secrets: Vec<PropertyString<KeyAndBase64Val>> = Vec::new();

        for new_secret in &secret {
            let sec = if new_secret.value.is_some() {
                // Updating or creating a secret

                // Make sure it is valid base64 encoded data
                if new_secret.decode_value().is_err() {
                    http_bail!(
                        BAD_REQUEST,
                        "secret '{}' does not have valid base64 encoded data",
                        new_secret.name
                    )
                }
                new_secret.clone()
            } else if let Some(old_secret) = old_secrets.iter().find(|v| v.name == new_secret.name)
            {
                // Keeping an already existing secret
                old_secret.clone()
            } else {
                http_bail!(BAD_REQUEST, "secret '{}' not known", new_secret.name);
            };

            if new_secrets.iter().any(|s| sec.name == s.name) {
                http_bail!(BAD_REQUEST, "secret '{}' defined multiple times", sec.name)
            }

            new_secrets.push(sec);
        }

        set_private_config_entry(
            config,
            &WebhookPrivateConfig {
                name: name.into(),
                secret: new_secrets,
            },
            WEBHOOK_TYPENAME,
            name,
        )?;
    }

    config
        .config
        .set_data(name, WEBHOOK_TYPENAME, &endpoint)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{name}': {e}"
            )
        })
}

/// Delete existing webhook endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the entity does not exist (`404 Not found`)
///   - the endpoint is still referenced by another entity (`400 Bad request`)
pub fn delete_endpoint(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the endpoint exists
    let _ = get_endpoint(config, name)?;
    super::ensure_safe_to_delete(config, name)?;

    remove_private_config_entry(config, name)?;
    config.config.sections.remove(name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::test_helpers::empty_config, endpoints::webhook::HttpMethod};

    use base64::encode;

    pub fn add_default_webhook_endpoint(config: &mut Config) -> Result<(), HttpError> {
        add_endpoint(
            config,
            WebhookConfig {
                name: "webhook-endpoint".into(),
                method: HttpMethod::Post,
                url: "http://example.com/webhook".into(),
                header: vec![KeyAndBase64Val::new_with_plain_value(
                    "Content-Type",
                    "application/json",
                )
                .into()],
                body: Some(encode("this is the body")),
                comment: Some("comment".into()),
                disable: Some(false),
                secret: vec![KeyAndBase64Val::new_with_plain_value("token", "secret").into()],
                ..Default::default()
            },
        )?;

        assert!(get_endpoint(config, "webhook-endpoint").is_ok());
        Ok(())
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();

        assert!(update_endpoint(&mut config, "test", Default::default(), None, None).is_err());

        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_webhook_endpoint(&mut config)?;

        assert!(update_endpoint(
            &mut config,
            "webhook-endpoint",
            Default::default(),
            None,
            Some(&[0; 32])
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_update() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_webhook_endpoint(&mut config)?;

        let digest = config.digest;

        update_endpoint(
            &mut config,
            "webhook-endpoint",
            WebhookConfigUpdater {
                url: Some("http://new.example.com/webhook".into()),
                comment: Some("newcomment".into()),
                method: Some(HttpMethod::Put),
                // Keep the old token and set a new one
                secret: Some(vec![
                    KeyAndBase64Val::new_with_plain_value("token2", "newsecret").into(),
                    KeyAndBase64Val {
                        name: "token".into(),
                        value: None,
                    }
                    .into(),
                ]),
                ..Default::default()
            },
            None,
            Some(&digest),
        )?;

        let endpoint = get_endpoint(&config, "webhook-endpoint")?;

        assert_eq!(endpoint.url, "http://new.example.com/webhook".to_string());
        assert_eq!(endpoint.comment, Some("newcomment".to_string()));
        assert!(matches!(endpoint.method, HttpMethod::Put));

        let secrets = config
            .private_config
            .lookup::<WebhookPrivateConfig>(WEBHOOK_TYPENAME, "webhook-endpoint")
            .unwrap()
            .secret;

        assert_eq!(secrets[1].name, "token".to_string());
        assert_eq!(secrets[1].value, Some(encode("secret")));
        assert_eq!(secrets[0].name, "token2".to_string());
        assert_eq!(secrets[0].value, Some(encode("newsecret")));

        // Test property deletion
        update_endpoint(
            &mut config,
            "webhook-endpoint",
            Default::default(),
            Some(&[
                DeleteableWebhookProperty::Comment,
                DeleteableWebhookProperty::Secret,
            ]),
            None,
        )?;

        let endpoint = get_endpoint(&config, "webhook-endpoint")?;
        assert_eq!(endpoint.comment, None);

        let secrets = config
            .private_config
            .lookup::<WebhookPrivateConfig>(WEBHOOK_TYPENAME, "webhook-endpoint")
            .unwrap()
            .secret;

        assert!(secrets.is_empty());

        Ok(())
    }

    #[test]
    fn test_delete() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_default_webhook_endpoint(&mut config)?;

        delete_endpoint(&mut config, "webhook-endpoint")?;
        assert!(delete_endpoint(&mut config, "webhook-endpoint").is_err());
        assert_eq!(get_endpoints(&config)?.len(), 0);

        Ok(())
    }
}
