//! Implements a wrapper around a (host, port) tuple, where host can either
//! be a plain IP address or a resolvable hostname.

use std::{
    fmt::{self, Display},
    net::IpAddr,
    str::FromStr,
};

#[cfg(feature = "api-types")]
use proxmox_schema::StringSchema;
#[cfg(feature = "api-types")]
use proxmox_schema::{ApiType, UpdaterType};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Represents either a resolvable hostname or an IPv4/IPv6 address.
/// IPv6 address are correctly bracketed on [`Display`], and parsing
/// automatically tries parsing it as an IP address first, falling back to a
/// plain hostname in the other case.
#[derive(Clone, Debug, PartialEq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum HostnameOrIpAddr {
    Hostname(String),
    IpAddr(IpAddr),
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

impl<S: Into<String>> From<S> for HostnameOrIpAddr {
    fn from(value: S) -> Self {
        let s = value.into();
        if let Ok(ip_addr) = IpAddr::from_str(&s) {
            Self::IpAddr(ip_addr)
        } else {
            Self::Hostname(s)
        }
    }
}

#[cfg(feature = "api-types")]
impl ApiType for HostnameOrIpAddr {
    const API_SCHEMA: proxmox_schema::Schema = StringSchema::new("hostname or ip address").schema();
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
    pub fn new<S: Into<String>>(host: S, port: u16) -> Self {
        let s = host.into();
        Self {
            host: s.into(),
            port,
        }
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
            host: host.into(),
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
    const API_SCHEMA: proxmox_schema::Schema = StringSchema::new("service endpoint").schema();
}

#[cfg(test)]
mod tests {
    use crate::endpoint::HostnameOrIpAddr;

    use super::ServiceEndpoint;

    #[test]
    fn display_works() {
        let s = ServiceEndpoint::new("127.0.0.1", 123);
        assert_eq!(s.to_string(), "127.0.0.1:123");

        let s = ServiceEndpoint::new("fc00:f00d::4321", 123);
        assert_eq!(s.to_string(), "[fc00:f00d::4321]:123");

        let s = ServiceEndpoint::new("::", 123);
        assert_eq!(s.to_string(), "[::]:123");

        let s = ServiceEndpoint::new("fc00::", 123);
        assert_eq!(s.to_string(), "[fc00::]:123");

        let s = ServiceEndpoint::new("example.com", 123);
        assert_eq!(s.to_string(), "example.com:123");
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
    }
}
