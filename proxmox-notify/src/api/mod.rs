use std::collections::HashSet;

use proxmox_http_error::HttpError;

use crate::Config;

pub mod common;
pub mod filter;
#[cfg(feature = "gotify")]
pub mod gotify;
pub mod group;
#[cfg(feature = "sendmail")]
pub mod sendmail;

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

    for group in group::get_groups(config)? {
        if group.endpoint.iter().any(|endpoint| endpoint == entity) {
            referrers.insert(group.name.clone());
        }

        if let Some(filter) = group.filter {
            if filter == entity {
                referrers.insert(group.name);
            }
        }
    }

    #[cfg(feature = "sendmail")]
    for endpoint in sendmail::get_endpoints(config)? {
        if let Some(filter) = endpoint.filter {
            if filter == entity {
                referrers.insert(endpoint.name);
            }
        }
    }

    #[cfg(feature = "gotify")]
    for endpoint in gotify::get_endpoints(config)? {
        if let Some(filter) = endpoint.filter {
            if filter == entity {
                referrers.insert(endpoint.name);
            }
        }
    }

    Ok(referrers)
}

fn ensure_unused(config: &Config, entity: &str) -> Result<(), HttpError> {
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
            if let Ok(group) = group::get_group(config, entity) {
                for target in group.endpoint {
                    new.insert(target.clone());
                }
            }

            #[cfg(feature = "sendmail")]
            if let Ok(target) = sendmail::get_endpoint(config, entity) {
                if let Some(filter) = target.filter {
                    new.insert(filter.clone());
                }
            }

            #[cfg(feature = "gotify")]
            if let Ok(target) = gotify::get_endpoint(config, entity) {
                if let Some(filter) = target.filter {
                    new.insert(filter.clone());
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

#[cfg(test)]
mod test_helpers {
    use crate::Config;

    #[allow(unused)]
    pub fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }
}

#[cfg(all(test, gotify, sendmail))]
mod tests {
    use super::*;
    use crate::endpoints::gotify::{GotifyConfig, GotifyPrivateConfig};
    use crate::endpoints::sendmail::SendmailConfig;
    use crate::filter::FilterConfig;
    use crate::group::GroupConfig;

    fn prepare_config() -> Result<Config, HttpError> {
        let mut config = super::test_helpers::empty_config();

        filter::add_filter(
            &mut config,
            &FilterConfig {
                name: "filter".to_string(),
                ..Default::default()
            },
        )?;

        sendmail::add_endpoint(
            &mut config,
            &SendmailConfig {
                name: "sendmail".to_string(),
                mailto: Some(vec!["foo@example.com".to_string()]),
                filter: Some("filter".to_string()),
                ..Default::default()
            },
        )?;

        gotify::add_endpoint(
            &mut config,
            &GotifyConfig {
                name: "gotify".to_string(),
                server: "localhost".to_string(),
                filter: Some("filter".to_string()),
                ..Default::default()
            },
            &GotifyPrivateConfig {
                name: "gotify".to_string(),
                token: "foo".to_string(),
            },
        )?;

        group::add_group(
            &mut config,
            &GroupConfig {
                name: "group".to_string(),
                endpoint: vec!["gotify".to_string(), "sendmail".to_string()],
                filter: Some("filter".to_string()),
                ..Default::default()
            },
        )?;

        Ok(config)
    }

    #[test]
    fn test_get_referenced_entities() {
        let config = prepare_config().unwrap();

        assert_eq!(
            get_referenced_entities(&config, "filter"),
            HashSet::from(["filter".to_string()])
        );
        assert_eq!(
            get_referenced_entities(&config, "sendmail"),
            HashSet::from(["filter".to_string(), "sendmail".to_string()])
        );
        assert_eq!(
            get_referenced_entities(&config, "gotify"),
            HashSet::from(["filter".to_string(), "gotify".to_string()])
        );
        assert_eq!(
            get_referenced_entities(&config, "group"),
            HashSet::from([
                "filter".to_string(),
                "gotify".to_string(),
                "sendmail".to_string(),
                "group".to_string()
            ])
        );
    }

    #[test]
    fn test_get_referrers_for_entity() -> Result<(), HttpError> {
        let config = prepare_config().unwrap();

        assert_eq!(
            get_referrers(&config, "filter")?,
            HashSet::from([
                "gotify".to_string(),
                "sendmail".to_string(),
                "group".to_string()
            ])
        );

        assert_eq!(
            get_referrers(&config, "sendmail")?,
            HashSet::from(["group".to_string()])
        );

        assert_eq!(
            get_referrers(&config, "gotify")?,
            HashSet::from(["group".to_string()])
        );

        assert!(get_referrers(&config, "group")?.is_empty(),);

        Ok(())
    }

    #[test]
    fn test_ensure_unused() {
        let config = prepare_config().unwrap();

        assert!(ensure_unused(&config, "filter").is_err());
        assert!(ensure_unused(&config, "gotify").is_err());
        assert!(ensure_unused(&config, "sendmail").is_err());
        assert!(ensure_unused(&config, "group").is_ok());
    }

    #[test]
    fn test_ensure_unique() {
        let config = prepare_config().unwrap();

        assert!(ensure_unique(&config, "sendmail").is_err());
        assert!(ensure_unique(&config, "group").is_err());
        assert!(ensure_unique(&config, "new").is_ok());
    }

    #[test]
    fn test_ensure_endpoints_exist() {
        let config = prepare_config().unwrap();

        assert!(ensure_endpoints_exist(&config, &vec!["sendmail", "gotify"]).is_ok());
        assert!(ensure_endpoints_exist(&config, &vec!["group", "filter"]).is_err());
    }
}
