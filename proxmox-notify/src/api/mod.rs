use std::collections::HashSet;
use std::error::Error as StdError;
use std::fmt::Display;

use crate::Config;
use serde::Serialize;

pub mod common;
pub mod filter;
#[cfg(feature = "gotify")]
pub mod gotify;
pub mod group;
#[cfg(feature = "sendmail")]
pub mod sendmail;

#[derive(Debug, Serialize)]
pub struct ApiError {
    /// HTTP Error code
    code: u16,
    /// Error message
    message: String,
    #[serde(skip_serializing)]
    /// The underlying cause of the error
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl ApiError {
    fn new<S: AsRef<str>>(
        message: S,
        code: u16,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            message: message.as_ref().into(),
            code,
            source,
        }
    }

    pub fn bad_request<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 400, source)
    }

    pub fn not_found<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 404, source)
    }

    pub fn internal_server_error<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 500, source)
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{} {}", self.code, self.message))
    }
}

impl StdError for ApiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.source {
            None => None,
            Some(source) => Some(&**source),
        }
    }
}

fn verify_digest(config: &Config, digest: Option<&[u8]>) -> Result<(), ApiError> {
    if let Some(digest) = digest {
        if config.digest != *digest {
            return Err(ApiError::bad_request(
                "detected modified configuration - file changed by other user? Try again.",
                None,
            ));
        }
    }

    Ok(())
}

fn endpoint_exists(config: &Config, name: &str) -> bool {
    let mut exists = false;

    #[cfg(feature = "sendmail")]
    {
        exists = exists || sendmail::get_endpoint(config, name).is_ok();
    }
    #[cfg(feature = "gotify")]
    {
        exists = exists || gotify::get_endpoint(config, name).is_ok();
    }

    exists
}

fn get_referrers(config: &Config, entity: &str) -> Result<HashSet<String>, ApiError> {
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

fn ensure_unused(config: &Config, entity: &str) -> Result<(), ApiError> {
    let referrers = get_referrers(config, entity)?;

    if !referrers.is_empty() {
        let used_by = referrers.into_iter().collect::<Vec<_>>().join(", ");

        return Err(ApiError::bad_request(
            format!("cannot delete '{entity}', referenced by: {used_by}"),
            None,
        ));
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

    pub fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoints::gotify::{GotifyConfig, GotifyPrivateConfig};
    use crate::endpoints::sendmail::SendmailConfig;
    use crate::filter::FilterConfig;
    use crate::group::GroupConfig;

    fn prepare_config() -> Result<Config, ApiError> {
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
    fn test_get_referrers_for_entity() -> Result<(), ApiError> {
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
}
