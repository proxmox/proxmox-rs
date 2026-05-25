//! Implements a wrapper around a (host, port) tuple, where host can either
//! be a plain IP address or a resolvable hostname.

use std::{
    fmt::{self, Display},
    net::IpAddr,
    str::FromStr,
};

#[cfg(feature = "api-types")]
use proxmox_schema::{
    ApiType, StringSchema, UpdaterType,
    api_types::{DNS_NAME_OR_IP_FORMAT, HOST_PORT_FORMAT},
};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Represents either a resolvable hostname or an IPv4/IPv6 address.
/// IPv6 address are correctly bracketed on [`Display`], and parsing
/// automatically tries parsing it as an IP address first, falling back to a
/// validated plain hostname in the other case. CIDR notation is rejected.
#[derive(Clone, Debug, PartialEq, Hash, Serialize)]
#[serde(untagged)]
pub enum HostnameOrIpAddr {
    Hostname(String),
    IpAddr(IpAddr),
}

#[derive(thiserror::Error, Debug)]
pub enum HostnameOrIpAddrParseError {
    #[error("invalid hostname or IP address: '{0}'")]
    Invalid(String),
}

impl FromStr for HostnameOrIpAddr {
    type Err = HostnameOrIpAddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // accept a `[fc00::1]` form for backward compat with stored values where
        // the surrounding brackets were not (yet) stripped by a higher layer
        let unbracketed = s
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
            .unwrap_or(s);
        if let Ok(ip_addr) = IpAddr::from_str(unbracketed) {
            return Ok(Self::IpAddr(ip_addr));
        }

        if is_valid_hostname(s) {
            return Ok(Self::Hostname(s.to_owned()));
        }

        Err(HostnameOrIpAddrParseError::Invalid(s.to_owned()))
    }
}

impl<'de> Deserialize<'de> for HostnameOrIpAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

fn is_valid_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
                && label
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

impl Display for HostnameOrIpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostnameOrIpAddr::Hostname(s) => Display::fmt(s, f),
            HostnameOrIpAddr::IpAddr(addr) => match addr {
                IpAddr::V4(v4) => Display::fmt(v4, f),
                IpAddr::V6(v6) => write!(f, "[{v6}]"),
            },
        }
    }
}

#[cfg(feature = "api-types")]
impl ApiType for HostnameOrIpAddr {
    const API_SCHEMA: proxmox_schema::Schema = StringSchema::new("DNS name or IP address.")
        .format(&DNS_NAME_OR_IP_FORMAT)
        .schema();
}

#[cfg(feature = "api-types")]
impl UpdaterType for HostnameOrIpAddr {
    type Updater = Option<Self>;
}

/// Represents a (host, port) tuple, where the host can either be a resolvable
/// hostname or an IPv4/IPv6 address.
#[derive(Clone, Debug, PartialEq, Hash, SerializeDisplay, DeserializeFromStr)]
pub struct ServiceEndpoint {
    host: HostnameOrIpAddr,
    port: u16,
}

impl ServiceEndpoint {
    pub fn new<S: AsRef<str>>(host: S, port: u16) -> Result<Self, HostnameOrIpAddrParseError> {
        Ok(Self {
            host: host.as_ref().parse()?,
            port,
        })
    }
}

impl Display for ServiceEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("host and port must be separated by a colon")]
    MissingSeparator,
    #[error("host part missing")]
    MissingHost,
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("invalid host: {0}")]
    InvalidHost(#[from] HostnameOrIpAddrParseError),
}

impl FromStr for ServiceEndpoint {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (mut host, port) = s.rsplit_once(':').ok_or(Self::Err::MissingSeparator)?;

        if host.is_empty() {
            return Err(Self::Err::MissingHost);
        }

        // [ and ] are not valid characters in a hostname, so strip them in case it
        // is a IPv6 address.
        host = host.trim_matches(['[', ']']);

        Ok(ServiceEndpoint {
            host: host.parse()?,
            port: port
                .parse()
                .map_err(|err: std::num::ParseIntError| Self::Err::InvalidPort(err.to_string()))?,
        })
    }
}

#[cfg(feature = "api-types")]
impl UpdaterType for ServiceEndpoint {
    type Updater = Option<Self>;
}

#[cfg(feature = "api-types")]
impl ApiType for ServiceEndpoint {
    const API_SCHEMA: proxmox_schema::Schema =
        StringSchema::new("service endpoint (DNS name or IP address with port).")
            .format(&HOST_PORT_FORMAT)
            .schema();
}

#[cfg(test)]
mod tests {
    use crate::endpoint::HostnameOrIpAddr;

    use super::ServiceEndpoint;

    #[test]
    fn display_works() {
        let s = ServiceEndpoint::new("127.0.0.1", 123).unwrap();
        assert_eq!(s.to_string(), "127.0.0.1:123");

        let s = ServiceEndpoint::new("fc00:f00d::4321", 123).unwrap();
        assert_eq!(s.to_string(), "[fc00:f00d::4321]:123");

        let s = ServiceEndpoint::new("::", 123).unwrap();
        assert_eq!(s.to_string(), "[::]:123");

        let s = ServiceEndpoint::new("fc00::", 123).unwrap();
        assert_eq!(s.to_string(), "[fc00::]:123");

        let s = ServiceEndpoint::new("example.com", 123).unwrap();
        assert_eq!(s.to_string(), "example.com:123");

        assert!(ServiceEndpoint::new("fc00::/64", 123).is_err());
        assert!(ServiceEndpoint::new("192.0.2.0/24", 123).is_err());
        assert!(ServiceEndpoint::new("foo/bar", 123).is_err());
    }

    #[test]
    fn hostname_or_ip_fromstr_works() {
        assert_eq!(
            "127.0.0.1".parse::<HostnameOrIpAddr>().unwrap(),
            HostnameOrIpAddr::IpAddr([127, 0, 0, 1].into()),
        );
        assert_eq!(
            "example.com".parse::<HostnameOrIpAddr>().unwrap(),
            HostnameOrIpAddr::Hostname("example.com".to_owned()),
        );
        assert_eq!(
            "foo".parse::<HostnameOrIpAddr>().unwrap(),
            HostnameOrIpAddr::Hostname("foo".to_owned()),
        );

        // brackets around IPv6 are accepted for backward compatibility
        let v6_loopback =
            HostnameOrIpAddr::IpAddr([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1].into());
        assert_eq!("::1".parse::<HostnameOrIpAddr>().unwrap(), v6_loopback);
        assert_eq!("[::1]".parse::<HostnameOrIpAddr>().unwrap(), v6_loopback);

        // brackets must not turn a hostname into an accepted value
        assert!("[example.com]".parse::<HostnameOrIpAddr>().is_err());
        // unbalanced brackets are rejected
        assert!("[::1".parse::<HostnameOrIpAddr>().is_err());
        assert!("::1]".parse::<HostnameOrIpAddr>().is_err());
        // bracketed CIDR is still garbage
        assert!("[fc00::/64]".parse::<HostnameOrIpAddr>().is_err());

        assert!("fc00::/64".parse::<HostnameOrIpAddr>().is_err());
        assert!("192.0.2.0/24".parse::<HostnameOrIpAddr>().is_err());
        assert!("foo/bar".parse::<HostnameOrIpAddr>().is_err());
    }

    #[test]
    fn fromstr_works() {
        assert_eq!(
            "127.0.0.1:123".parse::<ServiceEndpoint>().unwrap(),
            ServiceEndpoint {
                host: HostnameOrIpAddr::IpAddr([127, 0, 0, 1].into()),
                port: 123
            }
        );

        assert_eq!(
            "[::1]:123".parse::<ServiceEndpoint>().unwrap(),
            ServiceEndpoint {
                host: HostnameOrIpAddr::IpAddr(
                    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1].into()
                ),
                port: 123
            }
        );

        assert_eq!(
            "example.com:123".parse::<ServiceEndpoint>().unwrap(),
            ServiceEndpoint {
                host: HostnameOrIpAddr::Hostname("example.com".to_owned()),
                port: 123
            }
        );

        assert!("fc00::/64:123".parse::<ServiceEndpoint>().is_err());
        assert!("[fc00::/64]:123".parse::<ServiceEndpoint>().is_err());
        assert!("192.0.2.0/24:123".parse::<ServiceEndpoint>().is_err());
    }
}
