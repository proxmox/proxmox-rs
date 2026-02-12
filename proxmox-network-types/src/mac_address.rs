//! Utilities for handling EUI-48 MAC addresses.
//!
//! This module provides the [`MacAddress`] struct for parsing, formatting, and manipulating MAC
//! addresses, including conversion to IPv6 link-local addresses via EUI-64.
//!
//! # Examples
//!
//! Basic parsing and formatting:
//!
//! ```
//! use proxmox_network_types::MacAddress;
//!
//! let mac: MacAddress = "BC:24:11:49:8D:75".parse().expect("valid MAC");
//! assert_eq!(mac.to_string(), "BC:24:11:49:8D:75");
//! ```
//!
//! # API Schema
//!
//! When the `api-types` feature is enabled, [`MacAddress`] implements [`ApiType`] and provides a
//! schema definition. This allows it to be used seamlessly with `proxmox-schema` and the `api`
//! macro.
//!
//! Manual schema inspection:
//!
//! ```
//! # #[cfg(feature = "api-types")]
//! # fn main() {
//! # use proxmox_network_types::MacAddress;
//! # use proxmox_schema::{ApiType, Schema};
//! if let Schema::String(s) = MacAddress::API_SCHEMA {
//!     assert!(s.check_constraints("00:11:22:33:44:55").is_ok());
//! }
//! # }
//! # #[cfg(not(feature = "api-types"))]
//! # fn main() {}
//! ```
//!
//! Integration with the `#[api]` macro:
//!
//! ```
//! # #[cfg(feature = "api-types")]
//! # mod api_example {
//! use serde::{Deserialize, Serialize};
//! use proxmox_schema::api;
//! use proxmox_network_types::MacAddress;
//!
//! #[api]
//! #[derive(Deserialize, Serialize)]
//! /// A simple network interface.
//! struct NetworkInterface {
//!     /// The interface name.
//!     name: String,
//!     /// The MAC address (validation is handled automatically).
//!     mac: MacAddress,
//! }
//! # }
//! ```

use std::fmt::Display;
use std::net::Ipv6Addr;

use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

#[cfg(feature = "api-types")]
use proxmox_schema::{const_regex, ApiStringFormat, ApiType, Schema, StringSchema, UpdaterType};

/// Errors encountered when parsing a MAC address string.
#[derive(Error, Debug)]
pub enum MacAddressError {
    /// The input string did not contain the correct amount of octets.
    #[error("the hostname must be from 1 to 63 characters long")]
    InvalidLength,
    /// The input string contained characters that are not valid hexadecimal digits.
    #[error("the hostname contains invalid symbols")]
    InvalidSymbols,
}

#[cfg(feature = "api-types")]
const_regex! {
    /// Regex pattern validation for a standard 6-byte MAC address.
    pub MAC_ADDRESS_REGEX = r"([a-fA-F0-9]{2}:){5}[a-fA-F0-9]{2}";
}

#[cfg(feature = "api-types")]
/// API string format definition for MAC addresses using `MAC_ADDRESS_REGEX`.
pub const MAC_ADDRESS_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&MAC_ADDRESS_REGEX);

/// Represents an EUI-48 MAC Address.
///
/// Wraps a 6-byte array. Supports parsing from standard colon-separated hexadecimal strings (e.g.,
/// `00:11:22:33:44:55`) and formatting to uppercase.
///
/// When the `api-types` feature is enabled, this struct implements [`ApiType`] and [`UpdaterType`],
/// allowing it to be used as a field in structs derived with the `#[api]` macro without additional
/// schema configuration.
#[derive(
    Clone, Copy, Debug, DeserializeFromStr, SerializeDisplay, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct MacAddress([u8; 6]);

#[cfg(feature = "api-types")]
/// With the `api-types` feature enabled, the `MacAddress` type implements [`ApiType`] and provides
/// a JSON schema definition.
///
/// ```
/// use proxmox_network_types::MacAddress;
/// use proxmox_schema::{ApiType, Schema};
///
/// let schema = MacAddress::API_SCHEMA;
///
/// if let Schema::String(string_schema) = schema {
///     // The schema can be used directly to validate input strings, albeit it's normally used
///     // indirectly through the api-macro.
///     assert!(string_schema.check_constraints("00:11:22:33:44:55").is_ok());
///     assert!(string_schema.check_constraints("bad-mac-address").is_err());
/// } else {
///     panic!("MacAddress API schema is expected to be a StringSchema");
/// }
/// ```
impl ApiType for MacAddress {
    const API_SCHEMA: Schema = StringSchema::new("MAC address")
        .min_length(17)
        .max_length(17)
        .format(&MAC_ADDRESS_FORMAT)
        .schema();
}

#[cfg(feature = "api-types")]
impl UpdaterType for MacAddress {
    type Updater = Option<MacAddress>;
}

static LOCAL_PART: [u8; 8] = [0xFE, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
static EUI64_MIDDLE_PART: [u8; 2] = [0xFF, 0xFE];

impl MacAddress {
    /// Creates a new `MacAddress` from a raw 6-byte array.
    ///
    /// # Arguments
    ///
    /// * `address` - A 6-byte array representing the MAC address.
    pub fn new(address: [u8; 6]) -> Self {
        Self(address)
    }

    /// Generates a link-local IPv6 address according to RFC 4291 (Appendix A).
    ///
    /// This method performs the EUI-64 transformation, inserting `FF:FE` in the middle of the MAC
    /// address and flipping the universal/local bit (7th bit of the first byte).
    ///
    /// # Examples
    ///
    /// ```
    /// use proxmox_network_types::MacAddress;
    ///
    /// let mac: MacAddress = "BC:24:11:49:8D:75".parse().expect("valid MAC");
    /// let ipv6 = mac.eui64_link_local_address();
    ///
    /// assert_eq!(ipv6.to_string(), "fe80::be24:11ff:fe49:8d75");
    /// ```
    pub fn eui64_link_local_address(&self) -> Ipv6Addr {
        let head = &self.0[..3];
        let tail = &self.0[3..];

        let mut eui64_address: Vec<u8> = LOCAL_PART
            .iter()
            .chain(head.iter())
            .chain(EUI64_MIDDLE_PART.iter())
            .chain(tail.iter())
            .copied()
            .collect();

        // we need to flip the 7th bit of the first eui64 byte
        eui64_address[8] ^= 0x02;

        Ipv6Addr::from(
            TryInto::<[u8; 16]>::try_into(eui64_address).expect("is an u8 array with 16 entries"),
        )
    }
}

impl std::str::FromStr for MacAddress {
    type Err = MacAddressError;

    /// Parses a standard colon-separated hexadecimal MAC address string.
    ///
    /// Expects exactly 6 octets separated by colons (e.g., `01:23:45:67:89:AB`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':');

        let parsed = split
            .into_iter()
            .map(|elem| u8::from_str_radix(elem, 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| MacAddressError::InvalidSymbols)?;

        if parsed.len() != 6 {
            return Err(MacAddressError::InvalidLength);
        }

        // SAFETY: ok because of length check
        Ok(Self(parsed.try_into().unwrap()))
    }
}

impl Display for MacAddress {
    /// Formats the MAC address as an uppercase hexadecimal string separated by colons.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<02X}:{:<02X}:{:<02X}:{:<02X}:{:<02X}:{:<02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_parse_mac_address() {
        for input in [
            "aa:aa:aa:11:22:33",
            "AA:BB:FF:11:22:33",
            "bc:24:11:AA:bb:Ef",
        ] {
            let mac_address = input.parse::<MacAddress>().expect("valid mac address");

            assert_eq!(input.to_uppercase(), mac_address.to_string());
        }

        for input in [
            "aa:aa:aa:11:22:33:aa",
            "AA:BB:FF:11:22",
            "AA:BB:GG:11:22:33",
            "AABBGG112233",
            "",
        ] {
            input
                .parse::<MacAddress>()
                .expect_err("invalid mac address");
        }
    }

    #[test]
    fn test_eui64_link_local_address() {
        let mac_address: MacAddress = "BC:24:11:49:8D:75".parse().expect("valid MAC address");

        let link_local_address =
            Ipv6Addr::from_str("fe80::be24:11ff:fe49:8d75").expect("valid IPv6 address");

        assert_eq!(link_local_address, mac_address.eui64_link_local_address());
    }
}
