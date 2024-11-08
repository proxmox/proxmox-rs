use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use proxmox_http_error::HttpError;
use proxmox_schema::api;

use crate::{Config, Origin};

pub mod common;
#[cfg(feature = "gotify")]
pub mod gotify;
pub mod matcher;
#[cfg(feature = "sendmail")]
pub mod sendmail;
#[cfg(feature = "smtp")]
pub mod smtp;
#[cfg(feature = "webhook")]
pub mod webhook;

// We have our own, local versions of http_err and http_bail, because
// we don't want to wrap the error in anyhow::Error. If we were to do that,
// we would need to downcast in the perlmod bindings, since we need
// to return `HttpError` from there.
#[macro_export]
macro_rules! http_err {
    ($status:ident, $($fmt:tt)+) => {{
        proxmox_http_error::HttpError::new(
            proxmox_http_error::StatusCode::$status,
            format!($($fmt)+)
        )
    }};
}

#[macro_export]
macro_rules! http_bail {
    ($status:ident, $($fmt:tt)+) => {{
        return Err($crate::api::http_err!($status, $($fmt)+));
    }};
}

pub use http_bail;
pub use http_err;

#[api]
#[derive(Clone, Debug, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
/// Type of the endpoint.
pub enum EndpointType {
    /// Sendmail endpoint
    #[cfg(feature = "sendmail")]
    Sendmail,
    /// SMTP endpoint
    #[cfg(feature = "smtp")]
    Smtp,
    /// Gotify endpoint
    #[cfg(feature = "gotify")]
    Gotify,
    /// Webhook endpoint
    #[cfg(feature = "webhook")]
    Webhook,
}

#[api]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
/// Target information
pub struct Target {
    /// Name of the endpoint
    name: String,
    /// Origin of the endpoint
    origin: Origin,
    /// Type of the endpoint
    #[serde(rename = "type")]
    endpoint_type: EndpointType,
    /// Target is disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    disable: Option<bool>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
}

/// Get a list of all notification targets.
pub fn get_targets(config: &Config) -> Result<Vec<Target>, HttpError> {
    let mut targets = Vec::new();

    #[cfg(feature = "gotify")]
    for endpoint in gotify::get_endpoints(config)? {
        targets.push(Target {
            name: endpoint.name,
            origin: endpoint.origin.unwrap_or(Origin::UserCreated),
            endpoint_type: EndpointType::Gotify,
            disable: endpoint.disable,
            comment: endpoint.comment,
        })
    }

    #[cfg(feature = "sendmail")]
    for endpoint in sendmail::get_endpoints(config)? {
        targets.push(Target {
            name: endpoint.name,
            origin: endpoint.origin.unwrap_or(Origin::UserCreated),
            endpoint_type: EndpointType::Sendmail,
            disable: endpoint.disable,
            comment: endpoint.comment,
        })
    }

    #[cfg(feature = "smtp")]
    for endpoint in smtp::get_endpoints(config)? {
        targets.push(Target {
            name: endpoint.name,
            origin: endpoint.origin.unwrap_or(Origin::UserCreated),
            endpoint_type: EndpointType::Smtp,
            disable: endpoint.disable,
            comment: endpoint.comment,
        })
    }

    #[cfg(feature = "webhook")]
    for endpoint in webhook::get_endpoints(config)? {
        targets.push(Target {
            name: endpoint.name,
            origin: endpoint.origin.unwrap_or(Origin::UserCreated),
            endpoint_type: EndpointType::Webhook,
            disable: endpoint.disable,
            comment: endpoint.comment,
        })
    }

    Ok(targets)
}

fn verify_digest(config: &Config, digest: Option<&[u8]>) -> Result<(), HttpError> {
    if let Some(digest) = digest {
        if config.digest != *digest {
            http_bail!(
                BAD_REQUEST,
                "detected modified configuration - file changed by other user? Try again."
            );
        }
    }

    Ok(())
}

fn ensure_endpoint_exists(#[allow(unused)] config: &Config, name: &str) -> Result<(), HttpError> {
    #[allow(unused_mut)]
    let mut exists = false;

    #[cfg(feature = "sendmail")]
    {
        exists = exists || sendmail::get_endpoint(config, name).is_ok();
    }
    #[cfg(feature = "gotify")]
    {
        exists = exists || gotify::get_endpoint(config, name).is_ok();
    }
    #[cfg(feature = "smtp")]
    {
        exists = exists || smtp::get_endpoint(config, name).is_ok();
    }
    #[cfg(feature = "webhook")]
    {
        exists = exists || webhook::get_endpoint(config, name).is_ok();
    }

    if !exists {
        http_bail!(NOT_FOUND, "endpoint '{name}' does not exist")
    } else {
        Ok(())
    }
}

fn ensure_endpoints_exist<T: AsRef<str>>(
    config: &Config,
    endpoints: &[T],
) -> Result<(), HttpError> {
    for endpoint in endpoints {
        ensure_endpoint_exists(config, endpoint.as_ref())?;
    }

    Ok(())
}

fn ensure_unique(config: &Config, entity: &str) -> Result<(), HttpError> {
    if config.config.sections.contains_key(entity) {
        http_bail!(
            BAD_REQUEST,
            "Cannot create '{entity}', an entity with the same name already exists"
        );
    }

    Ok(())
}

fn get_referrers(config: &Config, entity: &str) -> Result<HashSet<String>, HttpError> {
    let mut referrers = HashSet::new();

    for matcher in matcher::get_matchers(config)? {
        if matcher.target.iter().any(|target| target == entity) {
            referrers.insert(matcher.name.clone());
        }
    }

    Ok(referrers)
}

fn ensure_safe_to_delete(config: &Config, entity: &str) -> Result<(), HttpError> {
    if let Some(entity_config) = config.config.sections.get(entity) {
        if let Ok(origin) = Origin::deserialize(&entity_config.1["origin"]) {
            // Built-ins are never actually removed, only reset to their default
            // It is thus safe to do the reset if another entity depends
            // on it
            if origin == Origin::Builtin || origin == Origin::ModifiedBuiltin {
                return Ok(());
            }
        }
    } else {
        http_bail!(NOT_FOUND, "entity '{entity}' does not exist");
    }

    let referrers = get_referrers(config, entity)?;

    if !referrers.is_empty() {
        let used_by = referrers.into_iter().collect::<Vec<_>>().join(", ");

        http_bail!(
            BAD_REQUEST,
            "cannot delete '{entity}', referenced by: {used_by}"
        );
    }

    Ok(())
}

fn get_referenced_entities(config: &Config, entity: &str) -> HashSet<String> {
    let mut to_expand = HashSet::new();
    let mut expanded = HashSet::new();
    to_expand.insert(entity.to_string());

    let expand = |entities: &HashSet<String>| -> HashSet<String> {
        let mut new = HashSet::new();

        for entity in entities {
            if let Ok(matcher) = matcher::get_matcher(config, entity) {
                for target in matcher.target {
                    new.insert(target.clone());
                }
            }
        }

        new
    };

    while !to_expand.is_empty() {
        let new = expand(&to_expand);
        expanded.extend(to_expand);
        to_expand = new;
    }

    expanded
}

#[allow(unused)]
fn set_private_config_entry<T: Serialize>(
    config: &mut Config,
    private_config: T,
    typename: &str,
    name: &str,
) -> Result<(), HttpError> {
    config
        .private_config
        .set_data(name, typename, &private_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save private config for endpoint '{}': {e}",
                name
            )
        })
}

#[allow(unused)]
fn remove_private_config_entry(config: &mut Config, name: &str) -> Result<(), HttpError> {
    config.private_config.sections.remove(name);
    Ok(())
}

#[cfg(test)]
mod test_helpers {
    use crate::Config;

    #[allow(unused)]
    pub fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }
}

#[cfg(all(test, feature = "gotify", feature = "sendmail"))]
mod tests {
    use super::*;
    use crate::endpoints::gotify::{GotifyConfig, GotifyPrivateConfig};
    use crate::endpoints::sendmail::SendmailConfig;
    use crate::matcher::MatcherConfig;

    fn prepare_config() -> Result<Config, HttpError> {
        let mut config = test_helpers::empty_config();

        sendmail::add_endpoint(
            &mut config,
            SendmailConfig {
                name: "sendmail".to_string(),
                mailto: vec!["foo@example.com".to_string()],
                ..Default::default()
            },
        )?;

        sendmail::add_endpoint(
            &mut config,
            SendmailConfig {
                name: "builtin".to_string(),
                mailto: vec!["foo@example.com".to_string()],
                origin: Some(Origin::Builtin),
                ..Default::default()
            },
        )?;

        gotify::add_endpoint(
            &mut config,
            GotifyConfig {
                name: "gotify".to_string(),
                server: "localhost".to_string(),
                ..Default::default()
            },
            GotifyPrivateConfig {
                name: "gotify".to_string(),
                token: "foo".to_string(),
            },
        )?;

        matcher::add_matcher(
            &mut config,
            MatcherConfig {
                name: "matcher".to_string(),
                target: vec![
                    "sendmail".to_string(),
                    "gotify".to_string(),
                    "builtin".to_string(),
                ],
                ..Default::default()
            },
        )?;

        Ok(config)
    }

    #[test]
    fn test_get_referenced_entities() {
        let config = prepare_config().unwrap();

        assert_eq!(
            get_referenced_entities(&config, "matcher"),
            HashSet::from([
                "matcher".to_string(),
                "sendmail".to_string(),
                "builtin".to_string(),
                "gotify".to_string()
            ])
        );
    }

    #[test]
    fn test_get_referrers_for_entity() -> Result<(), HttpError> {
        let config = prepare_config().unwrap();

        assert_eq!(
            get_referrers(&config, "sendmail")?,
            HashSet::from(["matcher".to_string()])
        );

        assert_eq!(
            get_referrers(&config, "gotify")?,
            HashSet::from(["matcher".to_string()])
        );

        Ok(())
    }

    #[test]
    fn test_ensure_safe_to_delete() {
        let config = prepare_config().unwrap();

        assert!(ensure_safe_to_delete(&config, "gotify").is_err());
        assert!(ensure_safe_to_delete(&config, "sendmail").is_err());
        assert!(ensure_safe_to_delete(&config, "matcher").is_ok());

        // built-ins are always safe to delete, since there is no way to actually
        // delete them... they will only be reset to their default settings and
        // will thus continue to exist
        assert!(ensure_safe_to_delete(&config, "builtin").is_ok());
    }

    #[test]
    fn test_ensure_unique() {
        let config = prepare_config().unwrap();

        assert!(ensure_unique(&config, "sendmail").is_err());
        assert!(ensure_unique(&config, "matcher").is_err());
        assert!(ensure_unique(&config, "new").is_ok());
    }

    #[test]
    fn test_ensure_endpoints_exist() {
        let config = prepare_config().unwrap();

        assert!(ensure_endpoints_exist(&config, &["sendmail", "gotify", "builtin"]).is_ok());
    }
}
