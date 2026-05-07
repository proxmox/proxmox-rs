//! Implements a interface for handling WireGuard configurations and serializing them into the
//! INI-style format as described in wg(8) "CONFIGURATION FILE FORMAT".
//!
//! WireGuard keys are 32-bytes securely-generated values, encoded as base64
//! for any usage where users might come in contact with them.
//!
//! [`PrivateKey`], [`PublicKey`] and [`PresharedKey`] implement all the needed
//! key primitives.
//!
//! By design there is no key pair, as keys should be treated as opaque from a
//! configuration perspective and not worked with.

#![forbid(unsafe_code, missing_docs)]

use std::fmt;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use x25519_dalek::StaticSecret;

use proxmox_network_types::{endpoint::ServiceEndpoint, ip_address::Cidr};
#[cfg(feature = "api-types")]
use proxmox_schema::{
    api_types::ED25519_BASE64_KEY_REGEX, ApiStringFormat, ApiType, StringSchema, UpdaterType,
};

/// Possible error when handling WireGuard configurations.
#[derive(thiserror::Error, Debug, PartialEq, Clone)]
pub enum Error {
    /// (Private) key generation failed
    #[error("failed to generate private key: {0}")]
    KeyGenFailed(String),
    /// Serialization to the WireGuard INI format failed
    #[error("failed to serialize config: {0}")]
    SerializationFailed(String),
}

impl From<proxmox_ini::Error> for Error {
    fn from(err: proxmox_ini::Error) -> Self {
        Self::SerializationFailed(err.to_string())
    }
}

/// Public key of a WireGuard peer.
#[derive(Clone, Copy, Deserialize, Serialize, Hash, Debug)]
#[serde(transparent)]
pub struct PublicKey(#[serde(with = "proxmox_serde::byte_array_as_base64")] [u8; 32]);

#[cfg(feature = "api-types")]
impl ApiType for PublicKey {
    const API_SCHEMA: proxmox_schema::Schema =
        StringSchema::new("ED25519 public key (base64 encoded)")
            .format(&ApiStringFormat::Pattern(&ED25519_BASE64_KEY_REGEX))
            .schema();
}

#[cfg(feature = "api-types")]
impl UpdaterType for PublicKey {
    type Updater = Option<PublicKey>;
}

/// Private key of a WireGuard peer.
#[derive(Serialize)]
#[serde(transparent)]
pub struct PrivateKey(#[serde(with = "proxmox_serde::byte_array_as_base64")] [u8; 32]);

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<private-key>")
    }
}

impl PrivateKey {
    /// Generates a new private key suitable for use with WireGuard.
    #[cfg(feature = "key-generation")]
    pub fn generate() -> Result<Self, Error> {
        Ok(Self(StaticSecret::random().to_bytes()))
    }

    /// Calculates the public key from the private key.
    pub fn public_key(&self) -> PublicKey {
        PublicKey(x25519_dalek::PublicKey::from(&StaticSecret::from(self.0)).to_bytes())
    }
}

impl From<[u8; 32]> for PrivateKey {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<x25519_dalek::StaticSecret> for PrivateKey {
    fn from(value: x25519_dalek::StaticSecret) -> Self {
        Self(value.to_bytes())
    }
}

/// Preshared key between two WireGuard peers.
#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct PresharedKey(
    #[serde(with = "proxmox_serde::byte_array_as_base64")] ed25519_dalek::SecretKey,
);

impl fmt::Debug for PresharedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<preshared-key>")
    }
}

impl PresharedKey {
    /// Length of the raw private key data in bytes.
    pub const RAW_LENGTH: usize = ed25519_dalek::SECRET_KEY_LENGTH;

    /// Generates a new preshared key suitable for use with WireGuard.
    #[cfg(feature = "key-generation")]
    pub fn generate() -> Result<Self, Error> {
        generate_key().map(Self)
    }

    /// Builds a new [`PrivateKey`] from raw key material.
    #[must_use]
    pub fn from_raw(data: ed25519_dalek::SecretKey) -> Self {
        // [`SigningKey`] takes care of correct key clamping.
        Self(SigningKey::from(&data).to_bytes())
    }
}

impl AsRef<ed25519_dalek::SecretKey> for PresharedKey {
    /// Returns the raw preshared key material.
    fn as_ref(&self) -> &ed25519_dalek::SecretKey {
        &self.0
    }
}

/// A single WireGuard peer.
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct WireGuardPeer {
    /// Public key, matching the private key of of the remote peer.
    pub public_key: PublicKey,
    /// Additional key preshared between two peers. Adds an additional layer of symmetric-key
    /// cryptography to be mixed into the already existing public-key cryptography, for
    /// post-quantum resistance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preshared_key: Option<PresharedKey>,
    /// List of IPv4/v6 CIDRs from which incoming traffic for this peer is allowed and to which
    /// outgoing traffic for this peer is directed. The catch-all 0.0.0.0/0 may be specified for
    /// matching all IPv4 addresses, and ::/0 may be specified for matching all IPv6 addresses.
    #[serde(rename = "AllowedIPs", skip_serializing_if = "Vec::is_empty")]
    pub allowed_ips: Vec<Cidr>,
    /// Remote peer endpoint address to connect to. Optional; only needed on the connecting side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<ServiceEndpoint>,
    /// A seconds interval, between 1 and 65535 inclusive, of how often to send an authenticated
    /// empty packet to the peer for the purpose of keeping a stateful firewall or NAT mapping
    /// valid persistently. For example, if the interface very rarely sends traffic, but it might
    /// at anytime receive traffic from a peer, and it is behind NAT, the interface might benefit
    /// from having a persistent keepalive interval of 25 seconds. If unset or set to 0, it is
    /// turned off.
    #[serde(skip_serializing_if = "persistent_keepalive_is_off")]
    pub persistent_keepalive: Option<u16>,
}

/// Determines whether the given `PersistentKeepalive` value means that it is
/// turned off. Useful for usage with serde's `skip_serializing_if`.
fn persistent_keepalive_is_off(value: &Option<u16>) -> bool {
    value.map(|v| v == 0).unwrap_or(true)
}

/// Properties of a WireGuard interface.
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct WireGuardInterface {
    /// Private key for this interface.
    pub private_key: PrivateKey,
    /// Port to listen on. Optional; if not specified, chosen randomly. Only needed on the "server"
    /// side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    /// Fwmark for outgoing packets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fw_mark: Option<u32>,
}

/// Top-level WireGuard configuration for WireGuard network interface. Holds all
/// parameters for the interface itself, as well as its remote peers.
#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct WireGuardConfig {
    /// The WireGuard-specific network interface configuration.
    pub interface: WireGuardInterface,
    /// Peers for this WireGuard interface.
    #[serde(rename = "Peer")]
    pub peers: Vec<WireGuardPeer>,
}

impl WireGuardConfig {
    /// Generate a raw, INI-style configuration file as accepted by wg(8).
    pub fn to_raw_config(self) -> Result<String, Error> {
        Ok(proxmox_ini::to_string(&self)?)
    }
}

/// Generates a new ED25519 private key.
#[cfg(feature = "key-generation")]
fn generate_key() -> Result<ed25519_dalek::SecretKey, Error> {
    let mut secret = ed25519_dalek::SecretKey::default();
    proxmox_sys::linux::fill_with_random_data(&mut secret)
        .map_err(|err| Error::KeyGenFailed(err.to_string()))?;

    // [`SigningKey`] takes care of correct key clamping.
    Ok(SigningKey::from(&secret).to_bytes())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use proxmox_network_types::ip_address::Cidr;

    use crate::{PresharedKey, PrivateKey, WireGuardConfig, WireGuardInterface, WireGuardPeer};

    fn mock_private_key(v: u8) -> PrivateKey {
        let base = v * 32;
        let key: [u8; 32] = (base..base + 32).collect::<Vec<u8>>().try_into().unwrap();
        PrivateKey(key.into())
    }

    fn mock_preshared_key(v: u8) -> PresharedKey {
        let base = v * 32;
        PresharedKey((base..base + 32).collect::<Vec<u8>>().try_into().unwrap())
    }

    #[test]
    fn single_peer() {
        let config = WireGuardConfig {
            interface: WireGuardInterface {
                private_key: mock_private_key(0),
                listen_port: Some(51820),
                fw_mark: Some(127),
            },
            peers: vec![WireGuardPeer {
                public_key: mock_private_key(1).public_key(),
                preshared_key: Some(mock_preshared_key(1)),
                allowed_ips: vec![Cidr::new_v4(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap()],
                endpoint: Some("foo.example.com:51820".parse().unwrap()),
                persistent_keepalive: Some(25),
            }],
        };

        pretty_assertions::assert_eq!(
            config.to_raw_config().unwrap(),
            "[Interface]
PrivateKey = AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=
ListenPort = 51820
FwMark = 127

[Peer]
PublicKey = NYBy1jZYgNGu6jKa35EhODhR7SGijjt16WXQ0s0WYlQ=
PresharedKey = ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=
AllowedIPs = 192.168.0.0/24
Endpoint = foo.example.com:51820
PersistentKeepalive = 25
"
        );
    }

    #[test]
    fn multiple_peers() {
        let config = WireGuardConfig {
            interface: WireGuardInterface {
                private_key: mock_private_key(0),
                listen_port: Some(51820),
                fw_mark: None,
            },
            peers: vec![
                WireGuardPeer {
                    public_key: mock_private_key(1).public_key(),
                    preshared_key: Some(mock_preshared_key(1)),
                    allowed_ips: vec![Cidr::new_v4(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap()],
                    endpoint: Some("foo.example.com:51820".parse().unwrap()),
                    persistent_keepalive: None,
                },
                WireGuardPeer {
                    public_key: mock_private_key(2).public_key(),
                    preshared_key: Some(mock_preshared_key(2)),
                    allowed_ips: vec![Cidr::new_v4(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap()],
                    endpoint: None,
                    persistent_keepalive: Some(25),
                },
                WireGuardPeer {
                    public_key: mock_private_key(3).public_key(),
                    preshared_key: Some(mock_preshared_key(3)),
                    allowed_ips: vec![Cidr::new_v4(Ipv4Addr::new(192, 168, 2, 0), 24).unwrap()],
                    endpoint: None,
                    persistent_keepalive: None,
                },
                WireGuardPeer {
                    public_key: mock_private_key(4).public_key(),
                    preshared_key: Some(mock_preshared_key(4)),
                    allowed_ips: vec![],
                    endpoint: Some("10.0.0.1:51820".parse().unwrap()),
                    persistent_keepalive: Some(25),
                },
            ],
        };

        pretty_assertions::assert_eq!(
            config.to_raw_config().unwrap(),
            "[Interface]
PrivateKey = AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=
ListenPort = 51820

[Peer]
PublicKey = NYBy1jZYgNGu6jKa35EhODhR7SGijjt16WXQ0s0WYlQ=
PresharedKey = ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=
AllowedIPs = 192.168.0.0/24
Endpoint = foo.example.com:51820

[Peer]
PublicKey = eaYx7t4b+cmPEgMs3q3Q56B5OY/HhriMyEbsia+FpRo=
PresharedKey = QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=
AllowedIPs = 192.168.1.0/24
PersistentKeepalive = 25

[Peer]
PublicKey = Z13VdO13iTELPS52gfN5C0ZsdzsVIf7PNld5WDcepS8=
PresharedKey = YGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn8=
AllowedIPs = 192.168.2.0/24

[Peer]
PublicKey = ST6C/HRGSlkmiBdiPSBTxeuOLMSpiLT+4XnsawENUx0=
PresharedKey = gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp8=
Endpoint = 10.0.0.1:51820
PersistentKeepalive = 25
"
        );
    }

    #[test]
    fn non_listening_peer() {
        let config = WireGuardConfig {
            interface: WireGuardInterface {
                private_key: mock_private_key(0),
                listen_port: None,
                fw_mark: None,
            },
            peers: vec![WireGuardPeer {
                public_key: mock_private_key(1).public_key(),
                preshared_key: Some(mock_preshared_key(1)),
                allowed_ips: vec![Cidr::new_v4(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap()],
                endpoint: Some("10.0.0.1:51820".parse().unwrap()),
                persistent_keepalive: Some(25),
            }],
        };

        pretty_assertions::assert_eq!(
            config.to_raw_config().unwrap(),
            "[Interface]
PrivateKey = AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=

[Peer]
PublicKey = NYBy1jZYgNGu6jKa35EhODhR7SGijjt16WXQ0s0WYlQ=
PresharedKey = ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=
AllowedIPs = 192.168.0.0/24
Endpoint = 10.0.0.1:51820
PersistentKeepalive = 25
"
        );
    }

    #[test]
    fn empty_peers() {
        let config = WireGuardConfig {
            interface: WireGuardInterface {
                private_key: mock_private_key(0),
                listen_port: Some(51830),
                fw_mark: Some(240),
            },
            peers: vec![],
        };

        pretty_assertions::assert_eq!(
            config.to_raw_config().unwrap(),
            "[Interface]
PrivateKey = AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=
ListenPort = 51830
FwMark = 240
"
        );
    }
}
