use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::Display;
use std::str::FromStr;

use context::context;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;

use proxmox_schema::api;
use proxmox_section_config::SectionConfigData;
use proxmox_uuid::Uuid;

pub mod matcher;
use matcher::{MatcherConfig, MATCHER_TYPENAME};

pub mod api;
pub mod config;
pub mod context;
pub mod endpoints;
pub mod filter;
pub mod group;
pub mod renderer;
pub mod schema;

#[derive(Debug)]
pub enum Error {
    /// There was an error serializing the config
    ConfigSerialization(Box<dyn StdError + Send + Sync>),
    /// There was an error deserializing the config
    ConfigDeserialization(Box<dyn StdError + Send + Sync>),
    /// An endpoint failed to send a notification
    NotifyFailed(String, Box<dyn StdError + Send + Sync>),
    /// A target does not exist
    TargetDoesNotExist(String),
    /// Testing one or more notification targets failed
    TargetTestFailed(Vec<Box<dyn StdError + Send + Sync>>),
    /// A filter could not be applied
    FilterFailed(String),
    /// The notification's template string could not be rendered
    RenderError(Box<dyn StdError + Send + Sync>),
    /// Generic error for anything else
    Generic(String),
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
            Error::Generic(message) => f.write_str(message),
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
            Error::Generic(_) => None,
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
    /// Unknown severity (e.g. forwarded system mails)
    Unknown,
}

impl Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Severity::Info => f.write_str("info"),
            Severity::Notice => f.write_str("notice"),
            Severity::Warning => f.write_str("warning"),
            Severity::Error => f.write_str("error"),
            Severity::Unknown => f.write_str("unknown"),
        }
    }
}

impl FromStr for Severity {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "info" => Ok(Self::Info),
            "notice" => Ok(Self::Notice),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "unknown" => Ok(Self::Unknown),
            _ => Err(Error::Generic(format!("invalid severity {s}"))),
        }
    }
}

#[api()]
#[derive(Clone, Debug, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// User-created config entry
    UserCreated,
    /// Config entry provided by the system
    Builtin,
    /// Config entry provided by the system, but modified by the user.
    ModifiedBuiltin,
}

/// Notification endpoint trait, implemented by all endpoint plugins
pub trait Endpoint {
    /// Send a documentation
    fn send(&self, notification: &Notification) -> Result<(), Error>;

    /// The name/identifier for this endpoint
    fn name(&self) -> &str;

    /// Check if the endpoint is disabled
    fn disabled(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Content {
    /// Title and body will be rendered as a template
    #[serde(rename_all = "kebab-case")]
    Template {
        /// Name of the used template
        template_name: String,
        /// Data that can be used for template rendering.
        data: Value,
    },
    #[cfg(feature = "mail-forwarder")]
    #[serde(rename_all = "kebab-case")]
    ForwardedMail {
        /// Raw mail contents
        raw: Vec<u8>,
        /// Fallback title
        title: String,
        /// Fallback body
        body: String,
        /// UID to use when calling sendmail
        #[allow(dead_code)] // Unused in some feature flag permutations
        uid: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Metadata {
    /// Notification severity
    severity: Severity,
    /// Timestamp of the notification as a UNIX epoch
    timestamp: i64,
    /// Additional fields for additional key-value metadata
    additional_fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Notification which can be sent
pub struct Notification {
    /// Notification content
    content: Content,
    /// Metadata
    metadata: Metadata,
    /// Unique ID
    id: Uuid,
}

impl Notification {
    pub fn from_template<S: AsRef<str>>(
        severity: Severity,
        template_name: S,
        template_data: Value,
        fields: HashMap<String, String>,
    ) -> Self {
        Self {
            metadata: Metadata {
                severity,
                additional_fields: fields,
                timestamp: proxmox_time::epoch_i64(),
            },
            content: Content::Template {
                template_name: template_name.as_ref().to_string(),
                data: template_data,
            },
            id: Uuid::generate(),
        }
    }
    #[cfg(feature = "mail-forwarder")]
    pub fn new_forwarded_mail(raw_mail: &[u8], uid: Option<u32>) -> Result<Self, Error> {
        let message = mail_parser::Message::parse(raw_mail)
            .ok_or_else(|| Error::Generic("could not parse forwarded email".to_string()))?;

        let title = message.subject().unwrap_or_default().into();
        let body = message.body_text(0).unwrap_or_default().into();

        let mut additional_fields = HashMap::new();
        additional_fields.insert("hostname".into(), proxmox_sys::nodename().into());
        additional_fields.insert("type".into(), "system-mail".into());

        Ok(Self {
            // Unfortunately we cannot reasonably infer the severity from the
            // mail contents, so just set it to the highest for now so that
            // it is not filtered out.
            content: Content::ForwardedMail {
                raw: raw_mail.into(),
                title,
                body,
                uid,
            },
            metadata: Metadata {
                severity: Severity::Unknown,
                additional_fields,
                timestamp: proxmox_time::epoch_i64(),
            },
            id: Uuid::generate(),
        })
    }

    /// Return the unique ID of this notification.
    pub fn id(&self) -> &Uuid {
        &self.id
    }
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
        let (mut config, digest) = config::config(raw_config)?;
        let (private_config, _) = config::private_config(raw_private_config)?;

        let default_config = context().default_config();

        let builtin_config = config::config_parser()
            .parse("<builtin>", default_config)
            .map_err(|err| Error::ConfigDeserialization(err.into()))?;

        for (key, (builtin_typename, builtin_value)) in &builtin_config.sections {
            if let Some((typename, value)) = config.sections.get_mut(key) {
                if builtin_typename == typename && value == builtin_value {
                    // Entry is built-in and the config entry section in notifications.cfg
                    // is exactly the same.
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("origin".to_string(), Value::String("builtin".into()));
                    } else {
                        log::error!(
                            "section config entry is not an object. This should not happen"
                        );
                    }
                } else {
                    // Entry is built-in, but it has been modified by the user.
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert(
                            "origin".to_string(),
                            Value::String("modified-builtin".into()),
                        );
                    } else {
                        log::error!(
                            "section config entry is not an object. This should not happen"
                        );
                    }
                }
            } else {
                let mut val = builtin_value.clone();

                if let Some(obj) = val.as_object_mut() {
                    obj.insert("origin".to_string(), Value::String("builtin".into()));
                } else {
                    log::error!("section config entry is not an object. This should not happen");
                }
                config
                    .set_data(key, builtin_typename, val)
                    .map_err(|err| Error::ConfigDeserialization(err.into()))?;
            }
        }

        for (_, (_, value)) in config.sections.iter_mut() {
            if let Some(obj) = value.as_object_mut() {
                if obj.get("origin").is_none() {
                    obj.insert("origin".to_string(), Value::String("user-created".into()));
                }
            }
        }

        Ok(Self {
            config,
            digest,
            private_config,
        })
    }

    /// Serialize config
    pub fn write(&self) -> Result<(String, String), Error> {
        let mut c = self.config.clone();
        for (_, (_, value)) in c.sections.iter_mut() {
            // Remove 'origin' parameter, we do not want it in our
            // config fields
            // TODO: Check if there is a better way for this, maybe a
            // separate type for API responses?
            if let Some(obj) = value.as_object_mut() {
                obj.remove("origin");
            } else {
                log::error!("section config entry is not an object. This should not happen");
            }
        }

        Ok((
            config::write(&c)?,
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
    matchers: Vec<MatcherConfig>,
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
        #[cfg(feature = "smtp")]
        {
            use endpoints::smtp::SMTP_TYPENAME;
            use endpoints::smtp::{SmtpConfig, SmtpEndpoint, SmtpPrivateConfig};
            endpoints.extend(
                parse_endpoints_with_private_config!(
                    config,
                    SmtpConfig,
                    SmtpPrivateConfig,
                    SmtpEndpoint,
                    SMTP_TYPENAME
                )?
                .into_iter()
                .map(|e| (e.name().into(), e)),
            );
        }

        let matchers = config
            .config
            .convert_to_typed_array(MATCHER_TYPENAME)
            .map_err(|err| Error::ConfigDeserialization(err.into()))?;

        Ok(Bus {
            endpoints,
            matchers,
        })
    }

    #[cfg(test)]
    pub fn add_endpoint(&mut self, endpoint: Box<dyn Endpoint>) {
        self.endpoints.insert(endpoint.name().to_string(), endpoint);
    }

    #[cfg(test)]
    pub fn add_matcher(&mut self, filter: MatcherConfig) {
        self.matchers.push(filter)
    }

    /// Send a notification. Notification matchers will determine which targets will receive
    /// the notification.
    ///
    /// Any errors will not be returned but only logged.
    pub fn send(&self, notification: &Notification) {
        let targets = matcher::check_matches(self.matchers.as_slice(), notification);

        for target in targets {
            if let Some(endpoint) = self.endpoints.get(target) {
                let name = endpoint.name();

                if endpoint.disabled() {
                    // Skip this target if it is disabled
                    log::info!("skipping disabled target '{name}'");
                    continue;
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
                log::error!("could not notify via target '{target}', it does not exist");
            }
        }
    }

    /// Send a test notification to a target (endpoint or group).
    ///
    /// In contrast to the `send` function, this function will return
    /// any errors to the caller.
    pub fn test_target(&self, target: &str) -> Result<(), Error> {
        let notification = Notification {
            metadata: Metadata {
                severity: Severity::Info,
                // TODO: what fields would make sense for test notifications?
                additional_fields: Default::default(),
                timestamp: proxmox_time::epoch_i64(),
            },
            content: Content::Template {
                template_name: "test".to_string(),
                data: json!({ "target": target }),
            },
            id: Uuid::generate(),
        };

        if let Some(endpoint) = self.endpoints.get(target) {
            endpoint.send(&notification)?;
        } else {
            return Err(Error::TargetDoesNotExist(target.to_string()));
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
    }

    impl Endpoint for MockEndpoint {
        fn send(&self, message: &Notification) -> Result<(), Error> {
            self.messages.borrow_mut().push(message.clone());

            Ok(())
        }

        fn name(&self) -> &str {
            self.name
        }

        fn disabled(&self) -> bool {
            false
        }
    }

    impl MockEndpoint {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                ..Default::default()
            }
        }

        fn messages(&self) -> Vec<Notification> {
            self.messages.borrow().clone()
        }
    }

    #[test]
    fn test_add_mock_endpoint() -> Result<(), Error> {
        let mock = MockEndpoint::new("endpoint");

        let mut bus = Bus::default();
        bus.add_endpoint(Box::new(mock.clone()));

        let matcher = MatcherConfig {
            target: vec!["endpoint".into()],
            ..Default::default()
        };

        bus.add_matcher(matcher);

        // Send directly to endpoint
        bus.send(&Notification::from_template(
            Severity::Info,
            "test",
            Default::default(),
            Default::default(),
        ));
        let messages = mock.messages();
        assert_eq!(messages.len(), 1);

        Ok(())
    }

    #[test]
    fn test_multiple_endpoints_with_different_matchers() -> Result<(), Error> {
        let endpoint1 = MockEndpoint::new("mock1");
        let endpoint2 = MockEndpoint::new("mock2");

        let mut bus = Bus::default();

        bus.add_endpoint(Box::new(endpoint1.clone()));
        bus.add_endpoint(Box::new(endpoint2.clone()));

        bus.add_matcher(MatcherConfig {
            name: "matcher1".into(),
            match_severity: vec!["warning,error".parse()?],
            target: vec!["mock1".into()],
            ..Default::default()
        });

        bus.add_matcher(MatcherConfig {
            name: "matcher2".into(),
            match_severity: vec!["error".parse()?],
            target: vec!["mock2".into()],
            ..Default::default()
        });

        let send_with_severity = |severity| {
            let notification = Notification::from_template(
                severity,
                "test",
                Default::default(),
                Default::default(),
            );

            bus.send(&notification);
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
