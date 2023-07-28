use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;

use proxmox_schema::api;
use proxmox_section_config::SectionConfigData;

pub mod filter;
use filter::{FilterConfig, FilterMatcher, FILTER_TYPENAME};

pub mod group;
use group::{GroupConfig, GROUP_TYPENAME};

pub mod api;
pub mod context;
pub mod endpoints;
pub mod renderer;
pub mod schema;

mod config;

#[derive(Debug)]
pub enum Error {
    ConfigSerialization(Box<dyn StdError + Send + Sync>),
    ConfigDeserialization(Box<dyn StdError + Send + Sync>),
    NotifyFailed(String, Box<dyn StdError + Send + Sync>),
    TargetDoesNotExist(String),
    TargetTestFailed(Vec<Box<dyn StdError + Send + Sync>>),
    FilterFailed(String),
    RenderError(Box<dyn StdError + Send + Sync>),
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
            Error::TargetTestFailed(errs) => {
                for err in errs {
                    writeln!(f, "{err}")?;
                }

                Ok(())
            }
            Error::FilterFailed(message) => {
                write!(f, "could not apply filter: {message}")
            }
            Error::RenderError(err) => write!(f, "could not render notification template: {err}"),
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
            Error::TargetTestFailed(errs) => Some(&*errs[0]),
            Error::FilterFailed(_) => None,
            Error::RenderError(err) => Some(&**err),
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

    /// The name of the filter to use
    fn filter(&self) -> Option<&str>;
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
    groups: HashMap<String, GroupConfig>,
    filters: Vec<FilterConfig>,
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
        #[allow(unused_mut)]
        let mut endpoints = HashMap::new();

        // Instantiate endpoints
        #[cfg(feature = "sendmail")]
        {
            use endpoints::sendmail::SENDMAIL_TYPENAME;
            use endpoints::sendmail::{SendmailConfig, SendmailEndpoint};
            endpoints.extend(
                parse_endpoints_without_private_config!(
                    config,
                    SendmailConfig,
                    SendmailEndpoint,
                    SENDMAIL_TYPENAME
                )?
                .into_iter()
                .map(|e| (e.name().into(), e)),
            );
        }

        #[cfg(feature = "gotify")]
        {
            use endpoints::gotify::GOTIFY_TYPENAME;
            use endpoints::gotify::{GotifyConfig, GotifyEndpoint, GotifyPrivateConfig};
            endpoints.extend(
                parse_endpoints_with_private_config!(
                    config,
                    GotifyConfig,
                    GotifyPrivateConfig,
                    GotifyEndpoint,
                    GOTIFY_TYPENAME
                )?
                .into_iter()
                .map(|e| (e.name().into(), e)),
            );
        }

        let groups: HashMap<String, GroupConfig> = config
            .config
            .convert_to_typed_array(GROUP_TYPENAME)
            .map_err(|err| Error::ConfigDeserialization(err.into()))?
            .into_iter()
            .map(|group: GroupConfig| (group.name.clone(), group))
            .collect();

        let filters = config
            .config
            .convert_to_typed_array(FILTER_TYPENAME)
            .map_err(|err| Error::ConfigDeserialization(err.into()))?;

        Ok(Bus {
            endpoints,
            groups,
            filters,
        })
    }

    #[cfg(test)]
    pub fn add_endpoint(&mut self, endpoint: Box<dyn Endpoint>) {
        self.endpoints.insert(endpoint.name().to_string(), endpoint);
    }

    #[cfg(test)]
    pub fn add_group(&mut self, group: GroupConfig) {
        self.groups.insert(group.name.clone(), group);
    }

    #[cfg(test)]
    pub fn add_filter(&mut self, filter: FilterConfig) {
        self.filters.push(filter)
    }

    /// Send a notification to a given target (endpoint or group).
    ///
    /// Any errors will not be returned but only logged.
    pub fn send(&self, endpoint_or_group: &str, notification: &Notification) {
        let mut filter_matcher = FilterMatcher::new(&self.filters, notification);

        if let Some(group) = self.groups.get(endpoint_or_group) {
            if !Bus::check_filter(&mut filter_matcher, group.filter.as_deref()) {
                log::info!("skipped target '{endpoint_or_group}', filter did not match");
                return;
            }

            log::info!("target '{endpoint_or_group}' is a group, notifying all members...");

            for endpoint in &group.endpoint {
                self.send_via_single_endpoint(endpoint, notification, &mut filter_matcher);
            }
        } else {
            self.send_via_single_endpoint(endpoint_or_group, notification, &mut filter_matcher);
        }
    }

    fn check_filter(filter_matcher: &mut FilterMatcher, filter: Option<&str>) -> bool {
        if let Some(filter) = filter {
            match filter_matcher.check_filter_match(filter) {
                // If the filter does not match, do nothing
                Ok(r) => r,
                Err(err) => {
                    // If there is an error, only log it and still send
                    log::error!("could not apply filter '{filter}': {err}");
                    true
                }
            }
        } else {
            true
        }
    }

    fn send_via_single_endpoint(
        &self,
        endpoint: &str,
        notification: &Notification,
        filter_matcher: &mut FilterMatcher,
    ) {
        if let Some(endpoint) = self.endpoints.get(endpoint) {
            let name = endpoint.name();
            if !Bus::check_filter(filter_matcher, endpoint.filter()) {
                log::info!("skipped target '{name}', filter did not match");
                return;
            }

            match endpoint.send(notification) {
                Ok(_) => {
                    log::info!("notified via target `{name}`");
                }
                Err(e) => {
                    // Only log on errors, do not propagate fail to the caller.
                    log::error!("could not notify via target `{name}`: {e}");
                }
            }
        } else {
            log::error!("could not notify via target '{endpoint}', it does not exist");
        }
    }

    /// Send a test notification to a target (endpoint or group).
    ///
    /// In contrast to the `send` function, this function will return
    /// any errors to the caller.
    pub fn test_target(&self, target: &str) -> Result<(), Error> {
        let notification = Notification {
            severity: Severity::Info,
            title: "Test notification".into(),
            body: "This is a test of the notification target '{{ target }}'".into(),
            properties: Some(json!({ "target": target })),
        };

        let mut errors: Vec<Box<dyn StdError + Send + Sync>> = Vec::new();

        let mut my_send = |target: &str| -> Result<(), Error> {
            if let Some(endpoint) = self.endpoints.get(target) {
                if let Err(e) = endpoint.send(&notification) {
                    errors.push(Box::new(e));
                }
            } else {
                return Err(Error::TargetDoesNotExist(target.to_string()));
            }
            Ok(())
        };

        if let Some(group) = self.groups.get(target) {
            for endpoint_name in &group.endpoint {
                my_send(endpoint_name)?;
            }
        } else {
            my_send(target)?;
        }

        if !errors.is_empty() {
            return Err(Error::TargetTestFailed(errors));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[derive(Default, Clone)]
    struct MockEndpoint {
        name: &'static str,
        // Needs to be an Rc so that we can clone MockEndpoint before
        // passing it to Bus, while still retaining a handle to the Vec
        messages: Rc<RefCell<Vec<Notification>>>,
        filter: Option<String>,
    }

    impl Endpoint for MockEndpoint {
        fn send(&self, message: &Notification) -> Result<(), Error> {
            self.messages.borrow_mut().push(message.clone());

            Ok(())
        }

        fn name(&self) -> &str {
            self.name
        }

        fn filter(&self) -> Option<&str> {
            self.filter.as_deref()
        }
    }

    impl MockEndpoint {
        fn new(name: &'static str, filter: Option<String>) -> Self {
            Self {
                name,
                filter,
                ..Default::default()
            }
        }

        fn messages(&self) -> Vec<Notification> {
            self.messages.borrow().clone()
        }
    }

    #[test]
    fn test_add_mock_endpoint() -> Result<(), Error> {
        let mock = MockEndpoint::new("endpoint", None);

        let mut bus = Bus::default();
        bus.add_endpoint(Box::new(mock.clone()));

        // Send directly to endpoint
        bus.send(
            "endpoint",
            &Notification {
                title: "Title".into(),
                body: "Body".into(),
                severity: Severity::Info,
                properties: Default::default(),
            },
        );
        let messages = mock.messages();
        assert_eq!(messages.len(), 1);

        Ok(())
    }

    #[test]
    fn test_groups() -> Result<(), Error> {
        let endpoint1 = MockEndpoint::new("mock1", None);
        let endpoint2 = MockEndpoint::new("mock2", None);

        let mut bus = Bus::default();

        bus.add_group(GroupConfig {
            name: "group1".to_string(),
            endpoint: vec!["mock1".into()],
            comment: None,
            filter: None,
        });

        bus.add_group(GroupConfig {
            name: "group2".to_string(),
            endpoint: vec!["mock2".into()],
            comment: None,
            filter: None,
        });

        bus.add_endpoint(Box::new(endpoint1.clone()));
        bus.add_endpoint(Box::new(endpoint2.clone()));

        let send_to_group = |channel| {
            bus.send(
                channel,
                &Notification {
                    title: "Title".into(),
                    body: "Body".into(),
                    severity: Severity::Info,
                    properties: Default::default(),
                },
            )
        };

        send_to_group("group1");
        assert_eq!(endpoint1.messages().len(), 1);
        assert_eq!(endpoint2.messages().len(), 0);

        send_to_group("group2");
        assert_eq!(endpoint1.messages().len(), 1);
        assert_eq!(endpoint2.messages().len(), 1);

        Ok(())
    }

    #[test]
    fn test_severity_ordering() {
        // Not intended to be exhaustive, just a quick
        // sanity check ;)

        assert!(Severity::Info < Severity::Notice);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Info < Severity::Error);
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Notice);
    }

    #[test]
    fn test_multiple_endpoints_with_different_filters() -> Result<(), Error> {
        let endpoint1 = MockEndpoint::new("mock1", Some("filter1".into()));
        let endpoint2 = MockEndpoint::new("mock2", Some("filter2".into()));

        let mut bus = Bus::default();

        bus.add_endpoint(Box::new(endpoint1.clone()));
        bus.add_endpoint(Box::new(endpoint2.clone()));

        bus.add_group(GroupConfig {
            name: "channel1".to_string(),
            endpoint: vec!["mock1".into(), "mock2".into()],
            comment: None,
            filter: None,
        });

        bus.add_filter(FilterConfig {
            name: "filter1".into(),
            min_severity: Some(Severity::Warning),
            mode: None,
            invert_match: None,
            comment: None,
        });

        bus.add_filter(FilterConfig {
            name: "filter2".into(),
            min_severity: Some(Severity::Error),
            mode: None,
            invert_match: None,
            comment: None,
        });

        let send_with_severity = |severity| {
            bus.send(
                "channel1",
                &Notification {
                    title: "Title".into(),
                    body: "Body".into(),
                    severity,
                    properties: Default::default(),
                },
            );
        };

        send_with_severity(Severity::Info);
        assert_eq!(endpoint1.messages().len(), 0);
        assert_eq!(endpoint2.messages().len(), 0);

        send_with_severity(Severity::Warning);
        assert_eq!(endpoint1.messages().len(), 1);
        assert_eq!(endpoint2.messages().len(), 0);

        send_with_severity(Severity::Error);
        assert_eq!(endpoint1.messages().len(), 2);
        assert_eq!(endpoint2.messages().len(), 1);

        Ok(())
    }
}
