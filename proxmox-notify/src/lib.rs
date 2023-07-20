use std::collections::HashMap;
use std::fmt::Display;

use proxmox_schema::api;
use proxmox_section_config::SectionConfigData;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;

use std::error::Error as StdError;

pub mod api;
mod config;
pub mod endpoints;
pub mod schema;

#[derive(Debug)]
pub enum Error {
    ConfigSerialization(Box<dyn StdError + Send + Sync>),
    ConfigDeserialization(Box<dyn StdError + Send + Sync>),
    NotifyFailed(String, Box<dyn StdError + Send + Sync>),
    TargetDoesNotExist(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ConfigSerialization(err) => {
                write!(f, "could not serialize configuration: {err}")
            }
            Error::ConfigDeserialization(err) => {
                write!(f, "could not deserialize configuration: {err}")
            }
            Error::NotifyFailed(endpoint, err) => {
                write!(f, "could not notify via endpoint(s): {endpoint}: {err}")
            }
            Error::TargetDoesNotExist(target) => {
                write!(f, "notification target '{target}' does not exist")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::ConfigSerialization(err) => Some(&**err),
            Error::ConfigDeserialization(err) => Some(&**err),
            Error::NotifyFailed(_, err) => Some(&**err),
            Error::TargetDoesNotExist(_) => None,
        }
    }
}

#[api()]
#[derive(Clone, Debug, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
/// Severity of a notification
pub enum Severity {
    /// General information
    Info,
    /// A noteworthy event
    Notice,
    /// Warning
    Warning,
    /// Error
    Error,
}

/// Notification endpoint trait, implemented by all endpoint plugins
pub trait Endpoint {
    /// Send a documentation
    fn send(&self, notification: &Notification) -> Result<(), Error>;

    /// The name/identifier for this endpoint
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
/// Notification which can be sent
pub struct Notification {
    /// Notification severity
    pub severity: Severity,
    /// The title of the notification
    pub title: String,
    /// Notification text
    pub body: String,
    /// Additional metadata for the notification
    pub properties: Option<Value>,
}

/// Notification configuration
#[derive(Debug, Clone)]
pub struct Config {
    config: SectionConfigData,
    private_config: SectionConfigData,
    digest: [u8; 32],
}

impl Config {
    /// Parse raw config
    pub fn new(raw_config: &str, raw_private_config: &str) -> Result<Self, Error> {
        let (config, digest) = config::config(raw_config)?;
        let (private_config, _) = config::private_config(raw_private_config)?;

        Ok(Self {
            config,
            digest,
            private_config,
        })
    }

    /// Serialize config
    pub fn write(&self) -> Result<(String, String), Error> {
        Ok((
            config::write(&self.config)?,
            config::write_private(&self.private_config)?,
        ))
    }

    /// Returns the SHA256 digest of the configuration.
    /// The digest is only computed once when the configuration deserialized.
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

/// Notification bus - distributes notifications to all registered endpoints
// The reason for the split between `Config` and this struct is to make testing with mocked
// endpoints a bit easier.
#[derive(Default)]
pub struct Bus {
    endpoints: HashMap<String, Box<dyn Endpoint>>,
}

#[allow(unused_macros)]
macro_rules! parse_endpoints_with_private_config {
    ($config:ident, $public_config:ty, $private_config:ty, $endpoint_type:ident, $type_name:expr) => {
        (|| -> Result<Vec<Box<dyn Endpoint>>, Error> {
            let mut endpoints = Vec::<Box<dyn Endpoint>>::new();

            let configs: Vec<$public_config> = $config
                .config
                .convert_to_typed_array($type_name)
                .map_err(|err| Error::ConfigDeserialization(err.into()))?;

            for config in configs {
                match $config.private_config.sections.get(&config.name) {
                    Some((section_type_name, private_config)) => {
                        if $type_name != section_type_name {
                            log::error!(
                                "Could not instantiate endpoint '{name}': \
                                private config has wrong type",
                                name = config.name
                            );
                        }
                        let private_config = <$private_config>::deserialize(private_config)
                            .map_err(|err| Error::ConfigDeserialization(err.into()))?;

                        endpoints.push(Box::new($endpoint_type {
                            config,
                            private_config: private_config.clone(),
                        }));
                    }
                    None => log::error!(
                        "Could not instantiate endpoint '{name}': \
                            private config does not exist",
                        name = config.name
                    ),
                }
            }

            Ok(endpoints)
        })()
    };
}

#[allow(unused_macros)]
macro_rules! parse_endpoints_without_private_config {
    ($config:ident, $public_config:ty, $endpoint_type:ident, $type_name:expr) => {
        (|| -> Result<Vec<Box<dyn Endpoint>>, Error> {
            let mut endpoints = Vec::<Box<dyn Endpoint>>::new();

            let configs: Vec<$public_config> = $config
                .config
                .convert_to_typed_array($type_name)
                .map_err(|err| Error::ConfigDeserialization(err.into()))?;

            for config in configs {
                endpoints.push(Box::new($endpoint_type { config }));
            }

            Ok(endpoints)
        })()
    };
}

impl Bus {
    /// Instantiate notification bus from a given configuration.
    pub fn from_config(config: &Config) -> Result<Self, Error> {
        let mut endpoints = HashMap::new();

        Ok(Bus { endpoints })
    }

    #[cfg(test)]
    pub fn add_endpoint(&mut self, endpoint: Box<dyn Endpoint>) {
        self.endpoints.insert(endpoint.name().to_string(), endpoint);
    }

    pub fn send(&self, target: &str, notification: &Notification) -> Result<(), Error> {
        log::info!(
            "sending notification with title '{title}'",
            title = notification.title
        );

        let endpoint = self
            .endpoints
            .get(target)
            .ok_or(Error::TargetDoesNotExist(target.into()))?;

        endpoint.send(notification).unwrap_or_else(|e| {
            log::error!(
                "could not notfiy via endpoint `{name}`: {e}",
                name = endpoint.name()
            )
        });

        Ok(())
    }

    pub fn test_target(&self, target: &str) -> Result<(), Error> {
        let endpoint = self
            .endpoints
            .get(target)
            .ok_or(Error::TargetDoesNotExist(target.into()))?;

        endpoint.send(&Notification {
            severity: Severity::Info,
            title: "Test notification".into(),
            body: "This is a test of the notification target '{{ target }}'".into(),
            properties: Some(json!({ "target": target })),
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[derive(Default, Clone)]
    struct MockEndpoint {
        // Needs to be an Rc so that we can clone MockEndpoint before
        // passing it to Bus, while still retaining a handle to the Vec
        messages: Rc<RefCell<Vec<Notification>>>,
    }

    impl Endpoint for MockEndpoint {
        fn send(&self, message: &Notification) -> Result<(), Error> {
            self.messages.borrow_mut().push(message.clone());

            Ok(())
        }

        fn name(&self) -> &str {
            "mock-endpoint"
        }
    }

    impl MockEndpoint {
        fn messages(&self) -> Vec<Notification> {
            self.messages.borrow().clone()
        }
    }

    #[test]
    fn test_add_mock_endpoint() -> Result<(), Error> {
        let mock = MockEndpoint::default();

        let mut bus = Bus::default();

        bus.add_endpoint(Box::new(mock.clone()));

        bus.send(
            "mock-endpoint",
            &Notification {
                title: "Title".into(),
                body: "Body".into(),
                severity: Severity::Info,
                properties: Default::default(),
            },
        )?;
        let messages = mock.messages();
        assert_eq!(messages.len(), 1);

        Ok(())
    }
}
