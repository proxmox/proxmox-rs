//! Provides helpers to deal with IP addresses / CIDRs.
//!
//! # Examples
//!
//! Parsing a CIDR and checking if it contains an IP:
//! ```
//! use proxmox_network_types::Cidr;
//! use std::net::IpAddr;
//!
//! let cidr: Cidr = "192.168.0.0/24".parse().unwrap();
//! let ip: IpAddr = "192.168.0.10".parse().unwrap();
//!
//! assert!(cidr.contains_address(&ip));
//! ```
//!
//! Checking for overlap between two CIDRs:
//! ```
//! use proxmox_network_types::Ipv4Cidr;
//!
//! let a: Ipv4Cidr = "10.0.0.0/8".parse().unwrap();
//! let b: Ipv4Cidr = "10.1.0.0/16".parse().unwrap();
//!
//! assert!(a.overlaps(&b));
//!
//! // IPv6 is supported too:
//! use proxmox_network_types::Ipv6Cidr;
//!
//! let c: Ipv6Cidr = "3fff::/20".parse().unwrap();
//! let d: Ipv6Cidr = "3fff:1::/24".parse().unwrap();
//!
//! assert!(c.overlaps(&d));
//!
//! // or IP-family agnostic:
//! use proxmox_network_types::Cidr;
//!
//! let a: Cidr = a.into();
//! let c: Cidr = c.into();
//!
//! assert!(!a.overlaps(&c)); // different families never overlap
//!
//! ```
//!
//! Converting an IP range into a minimal set of CIDRs:
//! ```
//! use proxmox_network_types::IpRange;
//!
//! let range: IpRange = "192.168.1.1-192.168.1.3".parse().unwrap();
//! let cidrs = range.to_cidrs();
//!
//! assert_eq!(cidrs.len(), 2);
//! assert_eq!(cidrs[0].to_string(), "192.168.1.1/32");
//! assert_eq!(cidrs[1].to_string(), "192.168.1.2/31");
//! ```
//!
//! # API Integration
//!
//! When the `api-types` feature is enabled, types in this crate implement [`ApiType`] and can be
//! used directly with the `#[api]` macro that is re-exported from [`proxmox_schema`].
//!
//! Note that [`std::net::Ipv4Addr`] and [`std::net::Ipv6Addr`] do not implement `ApiType`. You must
//! use the wrappers provided in [`api_types`] (e.g., [`api_types::Ipv4Addr`]) within your API
//! structs.
//!
//! ```
//! # #[cfg(feature = "api-types")]
//! # mod api_example {
//! use proxmox_schema::api;
//! use proxmox_network_types::{Cidr, api_types::Ipv4Addr};
//! use serde::{Deserialize, Serialize};
//!
//! #[api]
//! #[derive(Deserialize, Serialize)]
//! /// A struct representing a network configuration.
//! pub struct NetworkConfig {
//!     /// The CIDR of the network.
//!     pub cidr: Cidr,
//!
//!     /// An optional gateway IPv4 address.
//!     #[serde(skip_serializing_if = "Option::is_none")]
//!     pub gateway: Option<Ipv4Addr>,
//! }
//! # }
//! ```

use std::net::{AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr};

use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

#[cfg(feature = "api-types")]
use proxmox_schema::{ApiType, Schema, UpdaterType};

#[cfg(feature = "api-types")]
use proxmox_schema::api_types::{CIDR_SCHEMA, CIDR_V4_SCHEMA, CIDR_V6_SCHEMA};

#[cfg(feature = "api-types")]
/// Wrapper types for `std::net` IP addresses that implement `proxmox_schema` traits.
pub mod api_types {
    use std::net::AddrParseError;
    use std::ops::{Deref, DerefMut};

    use proxmox_schema::{
        api_types::{IP_SCHEMA, IP_V4_SCHEMA, IP_V6_SCHEMA},
        ApiType, UpdaterType,
    };
    use serde_with::{DeserializeFromStr, SerializeDisplay};

    /// A wrapper around [`std::net::Ipv4Addr`] that implements [`ApiType`].
    #[derive(
        Debug,
        Clone,
        Copy,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        DeserializeFromStr,
        SerializeDisplay,
        Hash,
    )]
    #[repr(transparent)]
    pub struct Ipv4Addr(pub std::net::Ipv4Addr);

    impl ApiType for Ipv4Addr {
        const API_SCHEMA: proxmox_schema::Schema = IP_V4_SCHEMA;
    }

    impl UpdaterType for Ipv4Addr {
        type Updater = Option<Ipv4Addr>;
    }

    impl Deref for Ipv4Addr {
        type Target = std::net::Ipv4Addr;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Ipv4Addr {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl std::str::FromStr for Ipv4Addr {
        type Err = AddrParseError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            let ip_address = std::net::Ipv4Addr::from_str(value)?;
            Ok(Self(ip_address))
        }
    }

    impl std::fmt::Display for Ipv4Addr {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl From<std::net::Ipv4Addr> for Ipv4Addr {
        fn from(value: std::net::Ipv4Addr) -> Self {
            Self(value)
        }
    }

    /// A wrapper around [`std::net::Ipv6Addr`] that implements [`ApiType`].
    #[derive(
        Debug,
        Clone,
        Copy,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        DeserializeFromStr,
        SerializeDisplay,
        Hash,
    )]
    #[repr(transparent)]
    pub struct Ipv6Addr(pub std::net::Ipv6Addr);

    impl ApiType for Ipv6Addr {
        const API_SCHEMA: proxmox_schema::Schema = IP_V6_SCHEMA;
    }

    impl UpdaterType for Ipv6Addr {
        type Updater = Option<Ipv6Addr>;
    }

    impl Deref for Ipv6Addr {
        type Target = std::net::Ipv6Addr;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Ipv6Addr {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl std::str::FromStr for Ipv6Addr {
        type Err = AddrParseError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            let ip_address = std::net::Ipv6Addr::from_str(value)?;
            Ok(Self(ip_address))
        }
    }

    impl std::fmt::Display for Ipv6Addr {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl From<std::net::Ipv6Addr> for Ipv6Addr {
        fn from(value: std::net::Ipv6Addr) -> Self {
            Self(value)
        }
    }

    #[derive(
        Debug,
        Clone,
        Copy,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        DeserializeFromStr,
        SerializeDisplay,
        Hash,
    )]
    #[repr(transparent)]
    pub struct IpAddr(pub std::net::IpAddr);

    impl ApiType for IpAddr {
        const API_SCHEMA: proxmox_schema::Schema = IP_SCHEMA;
    }

    impl UpdaterType for IpAddr {
        type Updater = Option<IpAddr>;
    }

    impl Deref for IpAddr {
        type Target = std::net::IpAddr;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for IpAddr {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl std::str::FromStr for IpAddr {
        type Err = AddrParseError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            let ip_address = std::net::IpAddr::from_str(value)?;
            Ok(Self(ip_address))
        }
    }

    impl std::fmt::Display for IpAddr {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl From<std::net::IpAddr> for IpAddr {
        fn from(value: std::net::IpAddr) -> Self {
            Self(value)
        }
    }
}

/// The family (v4 or v6) of an IP address or CIDR prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    /// Internet Protocol version 4.
    V4,
    /// Internet Protocol version 6.
    V6,
}

impl Family {
    /// Returns true if the family is IPv4.
    pub fn is_ipv4(self) -> bool {
        self == Self::V4
    }

    /// Returns true if the family is IPv6.
    pub fn is_ipv6(self) -> bool {
        self == Self::V6
    }
}

impl std::fmt::Display for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Family::V4 => f.write_str("IPv4"),
            Family::V6 => f.write_str("IPv6"),
        }
    }
}

/// Errors that can occur when parsing or constructing a CIDR.
#[derive(Error, Debug)]
pub enum CidrError {
    #[error("invalid netmask")]
    InvalidNetmask,
    #[error("invalid IP address")]
    InvalidAddress(#[from] AddrParseError),
}

/// Represents either an [`Ipv4Cidr`] or [`Ipv6Cidr`] CIDR prefix.
///
/// # Example
/// ```
/// use std::str::FromStr;
/// use proxmox_network_types::Cidr;
///
/// let cidr = Cidr::from_str("192.168.1.0/24").unwrap();
/// assert!(cidr.is_ipv4());
/// ```
#[derive(
    Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, SerializeDisplay, DeserializeFromStr,
)]
pub enum Cidr {
    Ipv4(Ipv4Cidr),
    Ipv6(Ipv6Cidr),
}

#[cfg(feature = "api-types")]
impl ApiType for Cidr {
    const API_SCHEMA: Schema = CIDR_SCHEMA;
}

#[cfg(feature = "api-types")]
impl UpdaterType for Cidr {
    type Updater = Option<Cidr>;
}

impl Cidr {
    /// Creates a new IPv4 CIDR.
    pub fn new_v4(addr: impl Into<Ipv4Addr>, mask: u8) -> Result<Self, CidrError> {
        Ok(Cidr::Ipv4(Ipv4Cidr::new(addr, mask)?))
    }

    /// Creates a new IPv6 CIDR.
    pub fn new_v6(addr: impl Into<Ipv6Addr>, mask: u8) -> Result<Self, CidrError> {
        Ok(Cidr::Ipv6(Ipv6Cidr::new(addr, mask)?))
    }

    /// Constructs a new [`Cidr`] from an generic [`IpAddr`], which can either be a IPv4 or IPv6
    /// address
    pub fn new(addr: impl Into<IpAddr>, mask: u8) -> Result<Self, CidrError> {
        match addr.into() {
            IpAddr::V4(v4) => Self::new_v4(v4, mask),
            IpAddr::V6(v6) => Self::new_v6(v6, mask),
        }
    }

    /// Returns the [`Family`] (v4 or v6) this CIDR belongs to.
    pub const fn family(&self) -> Family {
        match self {
            Cidr::Ipv4(_) => Family::V4,
            Cidr::Ipv6(_) => Family::V6,
        }
    }

    /// Returns true if the CIDR is IPv4.
    pub fn is_ipv4(&self) -> bool {
        matches!(self, Cidr::Ipv4(_))
    }

    /// Returns true if the CIDR is IPv6.
    pub fn is_ipv6(&self) -> bool {
        matches!(self, Cidr::Ipv6(_))
    }

    /// Returns the mask of the CIDR independent of the underlying IP family.
    pub fn mask(&self) -> u8 {
        match self {
            Cidr::Ipv4(ip) => ip.mask(),
            Cidr::Ipv6(ip) => ip.mask(),
        }
    }

    /// Returns the address portion of the CIDR independent of the underlying IP family.
    pub fn address(&self) -> IpAddr {
        match self {
            Cidr::Ipv4(ip) => IpAddr::V4(*ip.address()),
            Cidr::Ipv6(ip) => IpAddr::V6(*ip.address()),
        }
    }

    /// Checks if the two CIDRs overlap independent of their IP family.
    ///
    /// Returns false if the CIDRs are not of the same family.
    ///
    /// # Example
    /// ```
    /// use proxmox_network_types::Cidr;
    ///
    /// let a: Cidr = "192.168.0.0/24".parse().unwrap();
    /// let b: Cidr = "192.168.0.128/25".parse().unwrap();
    /// let c: Cidr = "10.0.0.0/8".parse().unwrap();
    /// let d: Cidr = "3fff::/20".parse().unwrap();
    /// let e: Cidr = "3fff:1::/36".parse().unwrap();
    ///
    /// assert!(a.overlaps(&b));
    /// assert!(!a.overlaps(&c));
    /// assert!(d.overlaps(&e));
    /// assert!(!a.overlaps(&d));
    /// ```
    pub fn overlaps(&self, other: &Cidr) -> bool {
        match (self, other) {
            (Cidr::Ipv4(a), Cidr::Ipv4(b)) => a.overlaps(b),
            (Cidr::Ipv6(a), Cidr::Ipv6(b)) => a.overlaps(b),
            _ => false,
        }
    }

    /// Checks whether a given IP address is contained in this [`Cidr`].
    ///
    /// When the CIDR and the IP address do *not* belong to the same family, they are compared as
    /// IPv4-mapped IPv6 addresses.
    ///
    /// # Example
    /// ```
    /// use proxmox_network_types::Cidr;
    /// use std::net::IpAddr;
    ///
    /// let cidr: Cidr = "192.168.0.0/24".parse().unwrap();
    /// let ip: IpAddr = "192.168.0.100".parse().unwrap();
    /// let ipv6: IpAddr = "::1".parse().unwrap();
    ///
    /// assert!(cidr.contains_address(&ip));
    /// assert!(!cidr.contains_address(&ipv6));
    ///
    /// // IPv4 mapped in IPv6 are normalized.
    /// let ipv4_mapped_v6: IpAddr = "::ffff:192.168.0.100".parse().unwrap();
    /// assert!(cidr.contains_address(&ipv4_mapped_v6));
    ///
    /// // Comparing a v4-mapped v6 CIDR works too, but remember that the mapped range goes from /96
    /// // to /128, so you need to add 96 to your IPv4 mask to get a matching mapped one.
    /// let cidr_v6: Cidr = "::ffff:192.168.0.0/120".parse().unwrap();
    /// assert!(cidr_v6.contains_address(&ip));
    /// ```
    pub fn contains_address(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (Cidr::Ipv4(cidr), IpAddr::V4(ip)) => cidr.contains_address(ip),
            (Cidr::Ipv6(cidr), IpAddr::V6(ip)) => cidr.contains_address(ip),
            (Cidr::Ipv4(cidr), IpAddr::V6(v6)) => v6
                .to_ipv4_mapped()
                .is_some_and(|v4| cidr.contains_address(&v4)),
            (Cidr::Ipv6(cidr), IpAddr::V4(v4)) => {
                let v6 = v4.to_ipv6_mapped();
                cidr.contains_address(&v6)
            }
        }
    }

    /// Parses a comma-separated list of CIDRs or IP ranges.
    ///
    /// The input string can contain a mix of CIDR notations and IP ranges, supporting both IPv4 and
    /// IPv6.
    ///
    /// # Example
    /// ```
    /// use proxmox_network_types::Cidr;
    ///
    /// let list = "10.0.0.1,2001:db8::1-2001:db8::2,192.168.0.0/16";
    /// let cidrs = Cidr::from_str_list(list).unwrap();
    ///
    /// assert_eq!(cidrs.len(), 4);
    /// assert_eq!(cidrs[0].to_string(), "10.0.0.1/32");
    /// assert_eq!(cidrs[1].to_string(), "2001:db8::1/128");
    /// assert_eq!(cidrs[2].to_string(), "2001:db8::2/128");
    /// assert_eq!(cidrs[3].to_string(), "192.168.0.0/16");
    /// ```
    pub fn from_str_list<S: AsRef<str>>(list: S) -> Result<Vec<Cidr>, IpRangeError> {
        let list = list.as_ref();
        let mut res = Vec::new();
        for s in list.split(',') {
            let s = s.trim();
            if s.is_empty() {
                continue;
            }

            if let Ok(cidr) = s.parse() {
                res.push(cidr);
                continue;
            }

            let range: IpRange = s.parse().map_err(|_| IpRangeError::InvalidFormat)?;
            res.extend(range.to_cidrs());
        }
        Ok(res)
    }
}

impl std::fmt::Display for Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Ipv4(ip) => std::fmt::Display::fmt(ip, f),
            Self::Ipv6(ip) => std::fmt::Display::fmt(ip, f),
        }
    }
}

impl std::str::FromStr for Cidr {
    type Err = CidrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(ip) = s.parse::<Ipv4Cidr>() {
            return Ok(Cidr::Ipv4(ip));
        }

        Ok(Cidr::Ipv6(s.parse()?))
    }
}

impl From<Ipv4Cidr> for Cidr {
    fn from(cidr: Ipv4Cidr) -> Self {
        Cidr::Ipv4(cidr)
    }
}

impl From<Ipv6Cidr> for Cidr {
    fn from(cidr: Ipv6Cidr) -> Self {
        Cidr::Ipv6(cidr)
    }
}

impl From<IpAddr> for Cidr {
    fn from(value: IpAddr) -> Self {
        match value {
            IpAddr::V4(addr) => Ipv4Cidr::from(addr).into(),
            IpAddr::V6(addr) => Ipv6Cidr::from(addr).into(),
        }
    }
}

const IPV4_LENGTH: u8 = 32;

/// An IPv4 CIDR (e.g. 192.0.2.0/24).
#[derive(
    Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, SerializeDisplay, DeserializeFromStr,
)]
pub struct Ipv4Cidr {
    addr: Ipv4Addr,
    mask: u8,
}

#[cfg(feature = "api-types")]
impl ApiType for Ipv4Cidr {
    const API_SCHEMA: Schema = CIDR_V4_SCHEMA;
}

#[cfg(feature = "api-types")]
impl UpdaterType for Ipv4Cidr {
    type Updater = Option<Ipv4Cidr>;
}

impl Ipv4Cidr {
    /// Creates a new IPv4 CIDR from an address and a mask.
    ///
    /// Returns an error if the mask is greater than 32.
    pub fn new(addr: impl Into<Ipv4Addr>, mask: u8) -> Result<Self, CidrError> {
        if mask > IPV4_LENGTH {
            return Err(CidrError::InvalidNetmask);
        }

        Ok(Self {
            addr: addr.into(),
            mask,
        })
    }

    /// Checks whether this CIDR contains a specific IPv4 address.
    pub fn contains_address(&self, other: &Ipv4Addr) -> bool {
        let bits = u32::from_be_bytes(self.addr.octets());
        let other_bits = u32::from_be_bytes(other.octets());

        let shift_amount: u32 = IPV4_LENGTH.saturating_sub(self.mask).into();

        bits.checked_shr(shift_amount).unwrap_or(0)
            == other_bits.checked_shr(shift_amount).unwrap_or(0)
    }

    /// Returns the address portion of the CIDR.
    pub fn address(&self) -> &Ipv4Addr {
        &self.addr
    }

    /// Returns the mask of the CIDR.
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Get the canonical representation of a IPv4 CIDR address.
    ///
    /// This normalizes the address, so we get the first address of a CIDR subnet (e.g.
    /// 2.2.2.200/24 -> 2.2.2.0) we do this by using a bitwise AND operation over the address and
    /// the u32::MAX (all ones) shifted by the mask.
    fn normalize(addr: u32, mask: u8) -> u32 {
        addr & u32::MAX.checked_shl((32 - mask).into()).unwrap_or(0)
    }

    /// Checks if the two CIDRs overlap.
    ///
    /// CIDRs are always disjoint so we only need to check if one CIDR contains
    /// the other. We do this by simply comparing the prefix.
    ///
    /// # Example
    /// ```
    /// use proxmox_network_types::Ipv4Cidr;
    ///
    /// let a: Ipv4Cidr = "192.168.1.0/24".parse().unwrap();
    /// let b: Ipv4Cidr = "192.168.1.128/25".parse().unwrap();
    /// let c: Ipv4Cidr = "10.0.0.0/8".parse().unwrap();
    ///
    /// assert!(a.overlaps(&b));
    /// assert!(!a.overlaps(&c));
    /// ```
    pub fn overlaps(&self, other: &Ipv4Cidr) -> bool {
        // we normalize by the smallest mask, so the larger of the two subnets.
        let min_mask = self.mask().min(other.mask());
        // if the prefix is the same we have an overlap
        Self::normalize(self.address().to_bits(), min_mask)
            == Self::normalize(other.address().to_bits(), min_mask)
    }

    /// Get the canonical version of the CIDR.
    ///
    /// A canonicalized CIDR is a the normalized address, so the first address in the subnet
    /// (sometimes also called "network address"). E.g. 2.2.2.5/24 -> 2.2.2.0/24
    pub fn canonical(&self) -> Self {
        Self {
            addr: Ipv4Addr::from_bits(Self::normalize(self.addr.to_bits(), self.mask())),
            mask: self.mask(),
        }
    }
}

impl<T: Into<Ipv4Addr>> From<T> for Ipv4Cidr {
    fn from(value: T) -> Self {
        Self {
            addr: value.into(),
            mask: 32,
        }
    }
}

impl std::str::FromStr for Ipv4Cidr {
    type Err = CidrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once('/') {
            None => Self {
                addr: s.parse()?,
                mask: 32,
            },
            Some((addr, mask)) => Self::new(
                addr.parse::<Ipv4Addr>()?,
                mask.parse::<u8>().map_err(|_| CidrError::InvalidNetmask)?,
            )?,
        })
    }
}

impl std::fmt::Display for Ipv4Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.mask)
    }
}

const IPV6_LENGTH: u8 = 128;

/// An IPv6 CIDR (e.g. 2001:db8::/32).
#[derive(
    Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, SerializeDisplay, DeserializeFromStr,
)]
pub struct Ipv6Cidr {
    addr: Ipv6Addr,
    mask: u8,
}

#[cfg(feature = "api-types")]
impl ApiType for Ipv6Cidr {
    const API_SCHEMA: Schema = CIDR_V6_SCHEMA;
}

#[cfg(feature = "api-types")]
impl UpdaterType for Ipv6Cidr {
    type Updater = Option<Ipv6Cidr>;
}

impl Ipv6Cidr {
    /// Creates a new IPv6 CIDR from an address and a mask.
    ///
    /// Returns an error if the mask is greater than 128.
    pub fn new(addr: impl Into<Ipv6Addr>, mask: u8) -> Result<Self, CidrError> {
        if mask > IPV6_LENGTH {
            return Err(CidrError::InvalidNetmask);
        }

        Ok(Self {
            addr: addr.into(),
            mask,
        })
    }

    /// Checks whether this CIDR contains a given IPv6 address.
    pub fn contains_address(&self, other: &Ipv6Addr) -> bool {
        let bits = u128::from_be_bytes(self.addr.octets());
        let other_bits = u128::from_be_bytes(other.octets());

        let shift_amount: u32 = IPV6_LENGTH.saturating_sub(self.mask).into();

        bits.checked_shr(shift_amount).unwrap_or(0)
            == other_bits.checked_shr(shift_amount).unwrap_or(0)
    }

    /// Returns the address portion of the CIDR.
    pub fn address(&self) -> &Ipv6Addr {
        &self.addr
    }

    /// Returns the mask of the CIDR.
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Get the canonical representation of a IPv6 CIDR address.
    ///
    /// This normalizes the address, so we get the first address of a CIDR subnet (e.g.
    /// 2001:db8::4/64 -> 2001:db8::0/64) we do this by using a bitwise AND operation over the address and
    /// the u128::MAX (all ones) shifted by the mask.
    fn normalize(addr: u128, mask: u8) -> u128 {
        addr & u128::MAX.checked_shl((128 - mask).into()).unwrap_or(0)
    }

    /// Checks if the two CIDRs overlap.
    ///
    /// CIDRs are always disjoint so we only need to check if one CIDR contains
    /// the other. We do this by simply comparing the prefix.
    pub fn overlaps(&self, other: &Ipv6Cidr) -> bool {
        // we normalize by the smallest mask, so the larger of the two subnets.
        let min_mask = self.mask().min(other.mask());
        // if the prefix is the same we have an overlap
        Self::normalize(self.address().to_bits(), min_mask)
            == Self::normalize(other.address().to_bits(), min_mask)
    }

    /// Get the canonical version of the CIDR.
    ///
    /// A canonicalized CIDR is a the normalized address, so the first address in the subnet
    /// (sometimes also called "network address"). E.g. 2001:db8::5/64 -> 2001:db8::0/64
    pub fn canonical(&self) -> Self {
        Self {
            addr: Ipv6Addr::from_bits(Self::normalize(self.addr.to_bits(), self.mask())),
            mask: self.mask(),
        }
    }
}

impl std::str::FromStr for Ipv6Cidr {
    type Err = CidrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once('/') {
            None => Self {
                addr: s.parse()?,
                mask: 128,
            },
            Some((addr, mask)) => Self::new(
                addr.parse::<Ipv6Addr>()?,
                mask.parse::<u8>().map_err(|_| CidrError::InvalidNetmask)?,
            )?,
        })
    }
}

impl std::fmt::Display for Ipv6Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.mask)
    }
}

impl<T: Into<Ipv6Addr>> From<T> for Ipv6Cidr {
    fn from(addr: T) -> Self {
        Self {
            addr: addr.into(),
            mask: 128,
        }
    }
}

/// Errors that can occur when handling IP ranges.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Error)]
pub enum IpRangeError {
    #[error("mismatched ip address families")]
    MismatchedFamilies,
    #[error("start is greater than last")]
    StartGreaterThanLast,
    #[error("invalid ip range format")]
    InvalidFormat,
}

/// Represents a range of IPv4 or IPv6 addresses.
///
/// For more information see [`AddressRange`].
///
/// # Example
/// ```
/// use proxmox_network_types::IpRange;
/// use std::str::FromStr;
///
/// let range = IpRange::from_str("192.168.1.5-192.168.1.10").unwrap();
/// let cidrs = range.to_cidrs();
/// // Result: 192.168.1.5/32, 192.168.1.6/31, 192.168.1.8/31, 192.168.1.10/32
/// ```
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, SerializeDisplay, DeserializeFromStr,
)]
pub enum IpRange {
    V4(AddressRange<Ipv4Addr>),
    V6(AddressRange<Ipv6Addr>),
}

impl IpRange {
    /// Returns the family of the IpRange.
    pub fn family(&self) -> Family {
        match self {
            IpRange::V4(_) => Family::V4,
            IpRange::V6(_) => Family::V6,
        }
    }

    /// Creates a new [`IpRange`] from two [`IpAddr`].
    ///
    /// # Errors
    ///
    /// This function will return an error if start and last IP address are not from the same family.
    pub fn new(start: impl Into<IpAddr>, last: impl Into<IpAddr>) -> Result<Self, IpRangeError> {
        match (start.into(), last.into()) {
            (IpAddr::V4(start), IpAddr::V4(last)) => Self::new_v4(start, last),
            (IpAddr::V6(start), IpAddr::V6(last)) => Self::new_v6(start, last),
            _ => Err(IpRangeError::MismatchedFamilies),
        }
    }

    /// Constructs a new IPv4 Range.
    pub fn new_v4(
        start: impl Into<Ipv4Addr>,
        last: impl Into<Ipv4Addr>,
    ) -> Result<Self, IpRangeError> {
        Ok(IpRange::V4(AddressRange::new_v4(start, last)?))
    }

    /// Constructs a new IPv6 Range.
    pub fn new_v6(
        start: impl Into<Ipv6Addr>,
        last: impl Into<Ipv6Addr>,
    ) -> Result<Self, IpRangeError> {
        Ok(IpRange::V6(AddressRange::new_v6(start, last)?))
    }

    /// Converts an IpRange into the minimal amount of CIDRs.
    ///
    /// See the concrete implementations of [`AddressRange<Ipv4Addr>`] or [`AddressRange<Ipv6Addr>`]
    /// respectively.
    pub fn to_cidrs(&self) -> Vec<Cidr> {
        match self {
            IpRange::V4(range) => range.to_cidrs().into_iter().map(Cidr::from).collect(),
            IpRange::V6(range) => range.to_cidrs().into_iter().map(Cidr::from).collect(),
        }
    }
}

impl std::str::FromStr for IpRange {
    type Err = IpRangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(range) = s.parse() {
            return Ok(IpRange::V4(range));
        }

        if let Ok(range) = s.parse() {
            return Ok(IpRange::V6(range));
        }

        Err(IpRangeError::InvalidFormat)
    }
}

impl std::fmt::Display for IpRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpRange::V4(range) => range.fmt(f),
            IpRange::V6(range) => range.fmt(f),
        }
    }
}

/// Represents a range of IP addresses from start to last.
///
/// This type is for encapsulation purposes for the [`IpRange`] enum and should be instantiated via
/// that enum.
///
/// # Invariants
///
/// * start and last have the same IP address family
/// * start is less than or equal to last
///
/// # Textual representation
///
/// Two IP addresses separated by a hyphen, e.g.: `127.0.0.1-127.0.0.255`
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, SerializeDisplay, DeserializeFromStr,
)]
pub struct AddressRange<T> {
    start: T,
    last: T,
}

impl AddressRange<Ipv4Addr> {
    pub(crate) fn new_v4(
        start: impl Into<Ipv4Addr>,
        last: impl Into<Ipv4Addr>,
    ) -> Result<AddressRange<Ipv4Addr>, IpRangeError> {
        let (start, last) = (start.into(), last.into());

        if start > last {
            return Err(IpRangeError::StartGreaterThanLast);
        }

        Ok(Self { start, last })
    }

    /// Returns the minimum amount of CIDRs that exactly represent the range.
    ///
    /// The idea behind this algorithm is as follows:
    ///
    /// Start iterating with current = start of the IP range
    ///
    /// Find two netmasks:
    /// * The largest CIDR that the current IP can be the first of
    /// * The largest CIDR that *only* contains IPs from current - last
    ///
    /// Add the smaller of the two CIDRs to our result and set current to the first IP that is in
    /// the range but not in the CIDR we just added. Proceed until we reached the last of the IP
    /// range.
    ///
    /// # Example
    /// ```
    /// use proxmox_network_types::{IpRange, Cidr};
    ///
    /// let range: IpRange = "192.168.1.1-192.168.1.3".parse().unwrap();
    /// let cidrs = range.to_cidrs();
    /// // Result: 192.168.1.1/32, 192.168.1.2/31
    /// ```
    ///
    /// Note: For an IP-family-agnostic function that parses a list of [`AddressRange`] and
    /// [`Cidr`], see [`Cidr::from_str_list`].
    pub fn to_cidrs(&self) -> Vec<Ipv4Cidr> {
        let mut cidrs = Vec::new();

        let mut current = u32::from_be_bytes(self.start.octets());
        let last = u32::from_be_bytes(self.last.octets());

        if current == last {
            // valid Ipv4 since netmask is 32
            cidrs.push(Ipv4Cidr::new(current, 32).unwrap());
            return cidrs;
        }

        // special case this, since this is the only possibility of overflow
        // when calculating delta_min_mask - makes everything a lot easier
        if current == u32::MIN && last == u32::MAX {
            // valid Ipv4 since it is `0.0.0.0/0`
            cidrs.push(Ipv4Cidr::new(current, 0).unwrap());
            return cidrs;
        }

        while current <= last {
            // netmask of largest CIDR that current IP can be the first of
            // cast is safe, because trailing zeroes can at most be 32
            let current_max_mask = IPV4_LENGTH - (current.trailing_zeros() as u8);

            // netmask of largest CIDR that *only* contains IPs of the remaining range
            // is at most 32 due to unwrap_or returning 32 and ilog2 being at most 31
            let delta_min_mask = ((last - current) + 1) // safe due to special case above
                .checked_ilog2() // should never occur due to special case, but for good measure
                .map(|mask| IPV4_LENGTH - mask as u8)
                .unwrap_or(IPV4_LENGTH);

            // at most 32, due to current/delta being at most 32
            let netmask = u8::max(current_max_mask, delta_min_mask);

            // netmask is at most 32, therefore safe to unwrap
            cidrs.push(Ipv4Cidr::new(current, netmask).unwrap());

            let delta = 2u32.saturating_pow((IPV4_LENGTH - netmask).into());

            if let Some(result) = current.checked_add(delta) {
                current = result
            } else {
                // we reached the end of IP address space
                break;
            }
        }

        cidrs
    }
}

impl AddressRange<Ipv6Addr> {
    pub(crate) fn new_v6(
        start: impl Into<Ipv6Addr>,
        last: impl Into<Ipv6Addr>,
    ) -> Result<AddressRange<Ipv6Addr>, IpRangeError> {
        let (start, last) = (start.into(), last.into());

        if start > last {
            return Err(IpRangeError::StartGreaterThanLast);
        }

        Ok(Self { start, last })
    }

    /// Returns the minimum amount of CIDRs that exactly represent the [`AddressRange`].
    ///
    /// This function works analogous to the IPv4 version, please refer to the respective
    /// documentation of [`AddressRange<Ipv4Addr>`].
    pub fn to_cidrs(&self) -> Vec<Ipv6Cidr> {
        let mut cidrs = Vec::new();

        let mut current = u128::from_be_bytes(self.start.octets());
        let last = u128::from_be_bytes(self.last.octets());

        if current == last {
            // valid Ipv6 since netmask is 128
            cidrs.push(Ipv6Cidr::new(current, 128).unwrap());
            return cidrs;
        }

        // special case this, since this is the only possibility of overflow
        // when calculating delta_min_mask - makes everything a lot easier
        if current == u128::MIN && last == u128::MAX {
            // valid Ipv6 since it is `::/0`
            cidrs.push(Ipv6Cidr::new(current, 0).unwrap());
            return cidrs;
        }

        while current <= last {
            // netmask of largest CIDR that current IP can be the first of
            // cast is safe, because trailing zeroes can at most be 128
            let current_max_mask = IPV6_LENGTH - (current.trailing_zeros() as u8);

            // netmask of largest CIDR that *only* contains IPs of the remaining range
            // is at most 128 due to unwrap_or returning 128 and ilog2 being at most 31
            let delta_min_mask = ((last - current) + 1) // safe due to special case above
                .checked_ilog2() // should never occur due to special case, but for good measure
                .map(|mask| IPV6_LENGTH - mask as u8)
                .unwrap_or(IPV6_LENGTH);

            // at most 128, due to current/delta being at most 128
            let netmask = u8::max(current_max_mask, delta_min_mask);

            // netmask is at most 128, therefore safe to unwrap
            cidrs.push(Ipv6Cidr::new(current, netmask).unwrap());

            let delta = 2u128.saturating_pow((IPV6_LENGTH - netmask).into());

            if let Some(result) = current.checked_add(delta) {
                current = result
            } else {
                // we reached the end of IP address space
                break;
            }
        }

        cidrs
    }
}

impl<T> AddressRange<T> {
    /// The first IP address contained in this [`AddressRange`].
    pub fn start(&self) -> &T {
        &self.start
    }

    /// The last IP address contained in this [`AddressRange`].
    pub fn last(&self) -> &T {
        &self.last
    }
}

impl std::str::FromStr for AddressRange<Ipv4Addr> {
    type Err = IpRangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((start, last)) = s.split_once('-') {
            let start_address = start
                .parse::<Ipv4Addr>()
                .map_err(|_| IpRangeError::InvalidFormat)?;

            let last_address = last
                .parse::<Ipv4Addr>()
                .map_err(|_| IpRangeError::InvalidFormat)?;

            return Self::new_v4(start_address, last_address);
        }

        Err(IpRangeError::InvalidFormat)
    }
}

impl std::str::FromStr for AddressRange<Ipv6Addr> {
    type Err = IpRangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((start, last)) = s.split_once('-') {
            let start_address = start
                .parse::<Ipv6Addr>()
                .map_err(|_| IpRangeError::InvalidFormat)?;

            let last_address = last
                .parse::<Ipv6Addr>()
                .map_err(|_| IpRangeError::InvalidFormat)?;

            return Self::new_v6(start_address, last_address);
        }

        Err(IpRangeError::InvalidFormat)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for AddressRange<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_v4_cidr() {
        let mut cidr: Ipv4Cidr = "0.0.0.0/0".parse().expect("valid IPv4 CIDR");

        assert_eq!(cidr.addr, Ipv4Addr::new(0, 0, 0, 0));
        assert_eq!(cidr.mask, 0);

        assert!(cidr.contains_address(&Ipv4Addr::new(0, 0, 0, 0)));
        assert!(cidr.contains_address(&Ipv4Addr::new(255, 255, 255, 255)));

        cidr = "192.168.100.1".parse().expect("valid IPv4 CIDR");

        assert_eq!(cidr.addr, Ipv4Addr::new(192, 168, 100, 1));
        assert_eq!(cidr.mask, 32);

        assert!(cidr.contains_address(&Ipv4Addr::new(192, 168, 100, 1)));
        assert!(!cidr.contains_address(&Ipv4Addr::new(192, 168, 100, 2)));
        assert!(!cidr.contains_address(&Ipv4Addr::new(192, 168, 100, 0)));

        cidr = "10.100.5.0/24".parse().expect("valid IPv4 CIDR");

        assert_eq!(cidr.mask, 24);

        assert!(cidr.contains_address(&Ipv4Addr::new(10, 100, 5, 0)));
        assert!(cidr.contains_address(&Ipv4Addr::new(10, 100, 5, 1)));
        assert!(cidr.contains_address(&Ipv4Addr::new(10, 100, 5, 100)));
        assert!(cidr.contains_address(&Ipv4Addr::new(10, 100, 5, 255)));
        assert!(!cidr.contains_address(&Ipv4Addr::new(10, 100, 4, 255)));
        assert!(!cidr.contains_address(&Ipv4Addr::new(10, 100, 6, 0)));

        "0.0.0.0/-1".parse::<Ipv4Cidr>().unwrap_err();
        "0.0.0.0/33".parse::<Ipv4Cidr>().unwrap_err();
        "256.256.256.256/10".parse::<Ipv4Cidr>().unwrap_err();

        "fe80::1/64".parse::<Ipv4Cidr>().unwrap_err();
        "qweasd".parse::<Ipv4Cidr>().unwrap_err();
        "".parse::<Ipv4Cidr>().unwrap_err();
    }

    #[test]
    fn test_v6_cidr() {
        let mut cidr: Ipv6Cidr = "abab::1/64".parse().expect("valid IPv6 CIDR");

        assert_eq!(cidr.addr, Ipv6Addr::new(0xABAB, 0, 0, 0, 0, 0, 0, 1));
        assert_eq!(cidr.mask, 64);

        assert!(cidr.contains_address(&Ipv6Addr::new(0xABAB, 0, 0, 0, 0, 0, 0, 0)));
        assert!(cidr.contains_address(&Ipv6Addr::new(
            0xABAB, 0, 0, 0, 0xAAAA, 0xAAAA, 0xAAAA, 0xAAAA
        )));
        assert!(cidr.contains_address(&Ipv6Addr::new(
            0xABAB, 0, 0, 0, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF
        )));
        assert!(!cidr.contains_address(&Ipv6Addr::new(0xABAB, 0, 0, 1, 0, 0, 0, 0)));
        assert!(!cidr.contains_address(&Ipv6Addr::new(
            0xABAA, 0, 0, 0, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF
        )));

        cidr = "eeee::1".parse().expect("valid IPv6 CIDR");

        assert_eq!(cidr.mask, 128);

        assert!(cidr.contains_address(&Ipv6Addr::new(0xEEEE, 0, 0, 0, 0, 0, 0, 1)));
        assert!(!cidr.contains_address(&Ipv6Addr::new(
            0xEEED, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF
        )));
        assert!(!cidr.contains_address(&Ipv6Addr::new(0xEEEE, 0, 0, 0, 0, 0, 0, 0)));

        "eeee::1/-1".parse::<Ipv6Cidr>().unwrap_err();
        "eeee::1/129".parse::<Ipv6Cidr>().unwrap_err();
        "gggg::1/64".parse::<Ipv6Cidr>().unwrap_err();

        "192.168.0.1".parse::<Ipv6Cidr>().unwrap_err();
        "qweasd".parse::<Ipv6Cidr>().unwrap_err();
        "".parse::<Ipv6Cidr>().unwrap_err();
    }

    #[test]
    fn test_ip_range() {
        IpRange::new([10, 0, 0, 2], [10, 0, 0, 1]).unwrap_err();

        IpRange::new(
            [0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x1000],
            [0x2001, 0x0db8, 0, 0, 0, 0, 0, 0],
        )
        .unwrap_err();

        let v4_range = IpRange::new([10, 0, 0, 0], [10, 0, 0, 100]).unwrap();
        assert_eq!(v4_range.family(), Family::V4);

        let v6_range = IpRange::new(
            [0x2001, 0x0db8, 0, 0, 0, 0, 0, 0],
            [0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x1000],
        )
        .unwrap();
        assert_eq!(v6_range.family(), Family::V6);

        "10.0.0.1-10.0.0.100".parse::<IpRange>().unwrap();
        "2001:db8::1-2001:db8::f".parse::<IpRange>().unwrap();

        "10.0.0.1-2001:db8::1000".parse::<IpRange>().unwrap_err();
        "2001:db8::1-192.168.0.2".parse::<IpRange>().unwrap_err();

        "10.0.0.1-10.0.0.0".parse::<IpRange>().unwrap_err();
        "2001:db8::1-2001:db8::0".parse::<IpRange>().unwrap_err();
    }

    #[test]
    fn test_ipv4_to_cidrs() {
        let range = AddressRange::new_v4([192, 168, 0, 100], [192, 168, 0, 100]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([192, 168, 0, 100], 32).unwrap()],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([192, 168, 0, 100], [192, 168, 0, 200]).unwrap();

        assert_eq!(
            [
                Ipv4Cidr::new([192, 168, 0, 100], 30).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 104], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 112], 28).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 128], 26).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 192], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 200], 32).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([192, 168, 0, 101], [192, 168, 0, 200]).unwrap();

        assert_eq!(
            [
                Ipv4Cidr::new([192, 168, 0, 101], 32).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 102], 31).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 104], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 112], 28).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 128], 26).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 192], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 200], 32).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([192, 168, 0, 101], [192, 168, 0, 101]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([192, 168, 0, 101], 32).unwrap()],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([192, 168, 0, 101], [192, 168, 0, 201]).unwrap();

        assert_eq!(
            [
                Ipv4Cidr::new([192, 168, 0, 101], 32).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 102], 31).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 104], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 112], 28).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 128], 26).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 192], 29).unwrap(),
                Ipv4Cidr::new([192, 168, 0, 200], 31).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([192, 168, 0, 0], [192, 168, 0, 255]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([192, 168, 0, 0], 24).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([0, 0, 0, 0], [255, 255, 255, 255]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([0, 0, 0, 0], 0).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([0, 0, 0, 1], [255, 255, 255, 255]).unwrap();

        assert_eq!(
            [
                Ipv4Cidr::new([0, 0, 0, 1], 32).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 2], 31).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 4], 30).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 8], 29).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 16], 28).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 32], 27).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 64], 26).unwrap(),
                Ipv4Cidr::new([0, 0, 0, 128], 25).unwrap(),
                Ipv4Cidr::new([0, 0, 1, 0], 24).unwrap(),
                Ipv4Cidr::new([0, 0, 2, 0], 23).unwrap(),
                Ipv4Cidr::new([0, 0, 4, 0], 22).unwrap(),
                Ipv4Cidr::new([0, 0, 8, 0], 21).unwrap(),
                Ipv4Cidr::new([0, 0, 16, 0], 20).unwrap(),
                Ipv4Cidr::new([0, 0, 32, 0], 19).unwrap(),
                Ipv4Cidr::new([0, 0, 64, 0], 18).unwrap(),
                Ipv4Cidr::new([0, 0, 128, 0], 17).unwrap(),
                Ipv4Cidr::new([0, 1, 0, 0], 16).unwrap(),
                Ipv4Cidr::new([0, 2, 0, 0], 15).unwrap(),
                Ipv4Cidr::new([0, 4, 0, 0], 14).unwrap(),
                Ipv4Cidr::new([0, 8, 0, 0], 13).unwrap(),
                Ipv4Cidr::new([0, 16, 0, 0], 12).unwrap(),
                Ipv4Cidr::new([0, 32, 0, 0], 11).unwrap(),
                Ipv4Cidr::new([0, 64, 0, 0], 10).unwrap(),
                Ipv4Cidr::new([0, 128, 0, 0], 9).unwrap(),
                Ipv4Cidr::new([1, 0, 0, 0], 8).unwrap(),
                Ipv4Cidr::new([2, 0, 0, 0], 7).unwrap(),
                Ipv4Cidr::new([4, 0, 0, 0], 6).unwrap(),
                Ipv4Cidr::new([8, 0, 0, 0], 5).unwrap(),
                Ipv4Cidr::new([16, 0, 0, 0], 4).unwrap(),
                Ipv4Cidr::new([32, 0, 0, 0], 3).unwrap(),
                Ipv4Cidr::new([64, 0, 0, 0], 2).unwrap(),
                Ipv4Cidr::new([128, 0, 0, 0], 1).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([0, 0, 0, 0], [255, 255, 255, 254]).unwrap();

        assert_eq!(
            [
                Ipv4Cidr::new([0, 0, 0, 0], 1).unwrap(),
                Ipv4Cidr::new([128, 0, 0, 0], 2).unwrap(),
                Ipv4Cidr::new([192, 0, 0, 0], 3).unwrap(),
                Ipv4Cidr::new([224, 0, 0, 0], 4).unwrap(),
                Ipv4Cidr::new([240, 0, 0, 0], 5).unwrap(),
                Ipv4Cidr::new([248, 0, 0, 0], 6).unwrap(),
                Ipv4Cidr::new([252, 0, 0, 0], 7).unwrap(),
                Ipv4Cidr::new([254, 0, 0, 0], 8).unwrap(),
                Ipv4Cidr::new([255, 0, 0, 0], 9).unwrap(),
                Ipv4Cidr::new([255, 128, 0, 0], 10).unwrap(),
                Ipv4Cidr::new([255, 192, 0, 0], 11).unwrap(),
                Ipv4Cidr::new([255, 224, 0, 0], 12).unwrap(),
                Ipv4Cidr::new([255, 240, 0, 0], 13).unwrap(),
                Ipv4Cidr::new([255, 248, 0, 0], 14).unwrap(),
                Ipv4Cidr::new([255, 252, 0, 0], 15).unwrap(),
                Ipv4Cidr::new([255, 254, 0, 0], 16).unwrap(),
                Ipv4Cidr::new([255, 255, 0, 0], 17).unwrap(),
                Ipv4Cidr::new([255, 255, 128, 0], 18).unwrap(),
                Ipv4Cidr::new([255, 255, 192, 0], 19).unwrap(),
                Ipv4Cidr::new([255, 255, 224, 0], 20).unwrap(),
                Ipv4Cidr::new([255, 255, 240, 0], 21).unwrap(),
                Ipv4Cidr::new([255, 255, 248, 0], 22).unwrap(),
                Ipv4Cidr::new([255, 255, 252, 0], 23).unwrap(),
                Ipv4Cidr::new([255, 255, 254, 0], 24).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 0], 25).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 128], 26).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 192], 27).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 224], 28).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 240], 29).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 248], 30).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 252], 31).unwrap(),
                Ipv4Cidr::new([255, 255, 255, 254], 32).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([0, 0, 0, 0], [0, 0, 0, 0]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([0, 0, 0, 0], 32).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v4([255, 255, 255, 255], [255, 255, 255, 255]).unwrap();

        assert_eq!(
            [Ipv4Cidr::new([255, 255, 255, 255], 32).unwrap(),],
            range.to_cidrs().as_slice()
        );
    }

    #[test]
    fn test_ipv6_to_cidrs() {
        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1000],
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1000],
        )
        .unwrap();

        assert_eq!(
            [Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1000], 128).unwrap()],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1000],
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2000],
        )
        .unwrap();

        assert_eq!(
            [
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1000], 116).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2000], 128).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001],
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2000],
        )
        .unwrap();

        assert_eq!(
            [
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001], 128).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1002], 127).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1004], 126).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1008], 125).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1010], 124).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1020], 123).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1040], 122).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1080], 121).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1100], 120).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1200], 119).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1400], 118).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1800], 117).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2000], 128).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001],
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001],
        )
        .unwrap();

        assert_eq!(
            [Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001], 128).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001],
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2001],
        )
        .unwrap();

        assert_eq!(
            [
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1001], 128).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1002], 127).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1004], 126).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1008], 125).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1010], 124).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1020], 123).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1040], 122).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1080], 121).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1100], 120).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1200], 119).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1400], 118).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x1800], 117).unwrap(),
                Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0x2000], 127).unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0],
            [0x2001, 0x0DB8, 0, 0, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF],
        )
        .unwrap();

        assert_eq!(
            [Ipv6Cidr::new([0x2001, 0x0DB8, 0, 0, 0, 0, 0, 0], 64).unwrap()],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0, 0, 0, 0, 0, 0, 0, 0],
            [
                0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            ],
        )
        .unwrap();

        assert_eq!(
            [Ipv6Cidr::new([0, 0, 0, 0, 0, 0, 0, 0], 0).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0, 0, 0, 0, 0, 0, 0, 0x0001],
            [
                0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            ],
        )
        .unwrap();

        assert_eq!(
            [
                "::1/128".parse::<Ipv6Cidr>().unwrap(),
                "::2/127".parse::<Ipv6Cidr>().unwrap(),
                "::4/126".parse::<Ipv6Cidr>().unwrap(),
                "::8/125".parse::<Ipv6Cidr>().unwrap(),
                "::10/124".parse::<Ipv6Cidr>().unwrap(),
                "::20/123".parse::<Ipv6Cidr>().unwrap(),
                "::40/122".parse::<Ipv6Cidr>().unwrap(),
                "::80/121".parse::<Ipv6Cidr>().unwrap(),
                "::100/120".parse::<Ipv6Cidr>().unwrap(),
                "::200/119".parse::<Ipv6Cidr>().unwrap(),
                "::400/118".parse::<Ipv6Cidr>().unwrap(),
                "::800/117".parse::<Ipv6Cidr>().unwrap(),
                "::1000/116".parse::<Ipv6Cidr>().unwrap(),
                "::2000/115".parse::<Ipv6Cidr>().unwrap(),
                "::4000/114".parse::<Ipv6Cidr>().unwrap(),
                "::8000/113".parse::<Ipv6Cidr>().unwrap(),
                "::1:0/112".parse::<Ipv6Cidr>().unwrap(),
                "::2:0/111".parse::<Ipv6Cidr>().unwrap(),
                "::4:0/110".parse::<Ipv6Cidr>().unwrap(),
                "::8:0/109".parse::<Ipv6Cidr>().unwrap(),
                "::10:0/108".parse::<Ipv6Cidr>().unwrap(),
                "::20:0/107".parse::<Ipv6Cidr>().unwrap(),
                "::40:0/106".parse::<Ipv6Cidr>().unwrap(),
                "::80:0/105".parse::<Ipv6Cidr>().unwrap(),
                "::100:0/104".parse::<Ipv6Cidr>().unwrap(),
                "::200:0/103".parse::<Ipv6Cidr>().unwrap(),
                "::400:0/102".parse::<Ipv6Cidr>().unwrap(),
                "::800:0/101".parse::<Ipv6Cidr>().unwrap(),
                "::1000:0/100".parse::<Ipv6Cidr>().unwrap(),
                "::2000:0/99".parse::<Ipv6Cidr>().unwrap(),
                "::4000:0/98".parse::<Ipv6Cidr>().unwrap(),
                "::8000:0/97".parse::<Ipv6Cidr>().unwrap(),
                "::1:0:0/96".parse::<Ipv6Cidr>().unwrap(),
                "::2:0:0/95".parse::<Ipv6Cidr>().unwrap(),
                "::4:0:0/94".parse::<Ipv6Cidr>().unwrap(),
                "::8:0:0/93".parse::<Ipv6Cidr>().unwrap(),
                "::10:0:0/92".parse::<Ipv6Cidr>().unwrap(),
                "::20:0:0/91".parse::<Ipv6Cidr>().unwrap(),
                "::40:0:0/90".parse::<Ipv6Cidr>().unwrap(),
                "::80:0:0/89".parse::<Ipv6Cidr>().unwrap(),
                "::100:0:0/88".parse::<Ipv6Cidr>().unwrap(),
                "::200:0:0/87".parse::<Ipv6Cidr>().unwrap(),
                "::400:0:0/86".parse::<Ipv6Cidr>().unwrap(),
                "::800:0:0/85".parse::<Ipv6Cidr>().unwrap(),
                "::1000:0:0/84".parse::<Ipv6Cidr>().unwrap(),
                "::2000:0:0/83".parse::<Ipv6Cidr>().unwrap(),
                "::4000:0:0/82".parse::<Ipv6Cidr>().unwrap(),
                "::8000:0:0/81".parse::<Ipv6Cidr>().unwrap(),
                "::1:0:0:0/80".parse::<Ipv6Cidr>().unwrap(),
                "::2:0:0:0/79".parse::<Ipv6Cidr>().unwrap(),
                "::4:0:0:0/78".parse::<Ipv6Cidr>().unwrap(),
                "::8:0:0:0/77".parse::<Ipv6Cidr>().unwrap(),
                "::10:0:0:0/76".parse::<Ipv6Cidr>().unwrap(),
                "::20:0:0:0/75".parse::<Ipv6Cidr>().unwrap(),
                "::40:0:0:0/74".parse::<Ipv6Cidr>().unwrap(),
                "::80:0:0:0/73".parse::<Ipv6Cidr>().unwrap(),
                "::100:0:0:0/72".parse::<Ipv6Cidr>().unwrap(),
                "::200:0:0:0/71".parse::<Ipv6Cidr>().unwrap(),
                "::400:0:0:0/70".parse::<Ipv6Cidr>().unwrap(),
                "::800:0:0:0/69".parse::<Ipv6Cidr>().unwrap(),
                "::1000:0:0:0/68".parse::<Ipv6Cidr>().unwrap(),
                "::2000:0:0:0/67".parse::<Ipv6Cidr>().unwrap(),
                "::4000:0:0:0/66".parse::<Ipv6Cidr>().unwrap(),
                "::8000:0:0:0/65".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:1::/64".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:2::/63".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:4::/62".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:8::/61".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:10::/60".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:20::/59".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:40::/58".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:80::/57".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:100::/56".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:200::/55".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:400::/54".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:800::/53".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:1000::/52".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:2000::/51".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:4000::/50".parse::<Ipv6Cidr>().unwrap(),
                "0:0:0:8000::/49".parse::<Ipv6Cidr>().unwrap(),
                "0:0:1::/48".parse::<Ipv6Cidr>().unwrap(),
                "0:0:2::/47".parse::<Ipv6Cidr>().unwrap(),
                "0:0:4::/46".parse::<Ipv6Cidr>().unwrap(),
                "0:0:8::/45".parse::<Ipv6Cidr>().unwrap(),
                "0:0:10::/44".parse::<Ipv6Cidr>().unwrap(),
                "0:0:20::/43".parse::<Ipv6Cidr>().unwrap(),
                "0:0:40::/42".parse::<Ipv6Cidr>().unwrap(),
                "0:0:80::/41".parse::<Ipv6Cidr>().unwrap(),
                "0:0:100::/40".parse::<Ipv6Cidr>().unwrap(),
                "0:0:200::/39".parse::<Ipv6Cidr>().unwrap(),
                "0:0:400::/38".parse::<Ipv6Cidr>().unwrap(),
                "0:0:800::/37".parse::<Ipv6Cidr>().unwrap(),
                "0:0:1000::/36".parse::<Ipv6Cidr>().unwrap(),
                "0:0:2000::/35".parse::<Ipv6Cidr>().unwrap(),
                "0:0:4000::/34".parse::<Ipv6Cidr>().unwrap(),
                "0:0:8000::/33".parse::<Ipv6Cidr>().unwrap(),
                "0:1::/32".parse::<Ipv6Cidr>().unwrap(),
                "0:2::/31".parse::<Ipv6Cidr>().unwrap(),
                "0:4::/30".parse::<Ipv6Cidr>().unwrap(),
                "0:8::/29".parse::<Ipv6Cidr>().unwrap(),
                "0:10::/28".parse::<Ipv6Cidr>().unwrap(),
                "0:20::/27".parse::<Ipv6Cidr>().unwrap(),
                "0:40::/26".parse::<Ipv6Cidr>().unwrap(),
                "0:80::/25".parse::<Ipv6Cidr>().unwrap(),
                "0:100::/24".parse::<Ipv6Cidr>().unwrap(),
                "0:200::/23".parse::<Ipv6Cidr>().unwrap(),
                "0:400::/22".parse::<Ipv6Cidr>().unwrap(),
                "0:800::/21".parse::<Ipv6Cidr>().unwrap(),
                "0:1000::/20".parse::<Ipv6Cidr>().unwrap(),
                "0:2000::/19".parse::<Ipv6Cidr>().unwrap(),
                "0:4000::/18".parse::<Ipv6Cidr>().unwrap(),
                "0:8000::/17".parse::<Ipv6Cidr>().unwrap(),
                "1::/16".parse::<Ipv6Cidr>().unwrap(),
                "2::/15".parse::<Ipv6Cidr>().unwrap(),
                "4::/14".parse::<Ipv6Cidr>().unwrap(),
                "8::/13".parse::<Ipv6Cidr>().unwrap(),
                "10::/12".parse::<Ipv6Cidr>().unwrap(),
                "20::/11".parse::<Ipv6Cidr>().unwrap(),
                "40::/10".parse::<Ipv6Cidr>().unwrap(),
                "80::/9".parse::<Ipv6Cidr>().unwrap(),
                "100::/8".parse::<Ipv6Cidr>().unwrap(),
                "200::/7".parse::<Ipv6Cidr>().unwrap(),
                "400::/6".parse::<Ipv6Cidr>().unwrap(),
                "800::/5".parse::<Ipv6Cidr>().unwrap(),
                "1000::/4".parse::<Ipv6Cidr>().unwrap(),
                "2000::/3".parse::<Ipv6Cidr>().unwrap(),
                "4000::/2".parse::<Ipv6Cidr>().unwrap(),
                "8000::/1".parse::<Ipv6Cidr>().unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [0, 0, 0, 0, 0, 0, 0, 0],
            [
                0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFE,
            ],
        )
        .unwrap();

        assert_eq!(
            [
                "::/1".parse::<Ipv6Cidr>().unwrap(),
                "8000::/2".parse::<Ipv6Cidr>().unwrap(),
                "c000::/3".parse::<Ipv6Cidr>().unwrap(),
                "e000::/4".parse::<Ipv6Cidr>().unwrap(),
                "f000::/5".parse::<Ipv6Cidr>().unwrap(),
                "f800::/6".parse::<Ipv6Cidr>().unwrap(),
                "fc00::/7".parse::<Ipv6Cidr>().unwrap(),
                "fe00::/8".parse::<Ipv6Cidr>().unwrap(),
                "ff00::/9".parse::<Ipv6Cidr>().unwrap(),
                "ff80::/10".parse::<Ipv6Cidr>().unwrap(),
                "ffc0::/11".parse::<Ipv6Cidr>().unwrap(),
                "ffe0::/12".parse::<Ipv6Cidr>().unwrap(),
                "fff0::/13".parse::<Ipv6Cidr>().unwrap(),
                "fff8::/14".parse::<Ipv6Cidr>().unwrap(),
                "fffc::/15".parse::<Ipv6Cidr>().unwrap(),
                "fffe::/16".parse::<Ipv6Cidr>().unwrap(),
                "ffff::/17".parse::<Ipv6Cidr>().unwrap(),
                "ffff:8000::/18".parse::<Ipv6Cidr>().unwrap(),
                "ffff:c000::/19".parse::<Ipv6Cidr>().unwrap(),
                "ffff:e000::/20".parse::<Ipv6Cidr>().unwrap(),
                "ffff:f000::/21".parse::<Ipv6Cidr>().unwrap(),
                "ffff:f800::/22".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fc00::/23".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fe00::/24".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ff00::/25".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ff80::/26".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffc0::/27".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffe0::/28".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fff0::/29".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fff8::/30".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fffc::/31".parse::<Ipv6Cidr>().unwrap(),
                "ffff:fffe::/32".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff::/33".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:8000::/34".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:c000::/35".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:e000::/36".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:f000::/37".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:f800::/38".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fc00::/39".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fe00::/40".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ff00::/41".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ff80::/42".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffc0::/43".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffe0::/44".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fff0::/45".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fff8::/46".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fffc::/47".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:fffe::/48".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff::/49".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:8000::/50".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:c000::/51".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:e000::/52".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:f000::/53".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:f800::/54".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fc00::/55".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fe00::/56".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ff00::/57".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ff80::/58".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffc0::/59".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffe0::/60".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fff0::/61".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fff8::/62".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fffc::/63".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:fffe::/64".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff::/65".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:8000::/66".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:c000::/67".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:e000::/68".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:f000::/69".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:f800::/70".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fc00::/71".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fe00::/72".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ff00::/73".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ff80::/74".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ffc0::/75".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ffe0::/76".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fff0::/77".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fff8::/78".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fffc::/79".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:fffe::/80".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ffff::/81".parse::<Ipv6Cidr>().unwrap(),
                "ffff:ffff:ffff:ffff:ffff:8000::/82"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:c000::/83"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:e000::/84"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:f000::/85"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:f800::/86"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fc00::/87"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fe00::/88"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ff00::/89"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ff80::/90"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffc0::/91"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffe0::/92"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fff0::/93"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fff8::/94"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fffc::/95"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:fffe::/96"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff::/97"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:8000:0/98"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:c000:0/99"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:e000:0/100"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:f000:0/101"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:f800:0/102"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fc00:0/103"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fe00:0/104"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ff00:0/105"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ff80:0/106"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffc0:0/107"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffe0:0/108"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fff0:0/109"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fff8:0/110"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fffc:0/111"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:fffe:0/112"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:0/113"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:8000/114"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:c000/115"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:e000/116"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:f000/117"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:f800/118"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fc00/119"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fe00/120"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ff00/121"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ff80/122"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffc0/123"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffe0/124"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fff0/125"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fff8/126"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffc/127"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:fffe/128"
                    .parse::<Ipv6Cidr>()
                    .unwrap(),
            ],
            range.to_cidrs().as_slice()
        );

        let range =
            AddressRange::new_v6([0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0, 0, 0]).unwrap();

        assert_eq!(
            [Ipv6Cidr::new([0, 0, 0, 0, 0, 0, 0, 0], 128).unwrap(),],
            range.to_cidrs().as_slice()
        );

        let range = AddressRange::new_v6(
            [
                0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            ],
            [
                0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF,
            ],
        )
        .unwrap();

        assert_eq!(
            [Ipv6Cidr::new(
                [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF],
                128
            )
            .unwrap(),],
            range.to_cidrs().as_slice()
        );
    }

    #[test]
    fn test_ipv4_overlap() {
        assert!(
            Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24).unwrap())
        );

        assert!(
            Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24).unwrap())
        );

        assert!(
            !Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.1.0".parse::<Ipv4Addr>().unwrap(), 24).unwrap())
        );

        assert!(
            Ipv4Cidr::new("192.168.0.200".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("192.168.0.100".parse::<Ipv4Addr>().unwrap(), 24).unwrap()
                )
        );

        assert!(
            Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("192.168.0.128".parse::<Ipv4Addr>().unwrap(), 25).unwrap()
                )
        );

        assert!(
            Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 24)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("192.168.0.129".parse::<Ipv4Addr>().unwrap(), 25).unwrap()
                )
        );

        assert!(
            Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 16)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("192.168.0.129".parse::<Ipv4Addr>().unwrap(), 30).unwrap()
                )
        );

        assert!(Ipv4Cidr::new("10.0.0.1".parse::<Ipv4Addr>().unwrap(), 32)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("10.0.0.1".parse::<Ipv4Addr>().unwrap(), 32).unwrap()));

        assert!(!Ipv4Cidr::new("10.0.0.1".parse::<Ipv4Addr>().unwrap(), 32)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("10.0.0.2".parse::<Ipv4Addr>().unwrap(), 32).unwrap()));

        assert!(Ipv4Cidr::new("10.0.0.0".parse::<Ipv4Addr>().unwrap(), 8)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("10.5.10.100".parse::<Ipv4Addr>().unwrap(), 32).unwrap()));

        assert!(Ipv4Cidr::new("0.0.0.0".parse::<Ipv4Addr>().unwrap(), 0)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("172.16.0.0".parse::<Ipv4Addr>().unwrap(), 12).unwrap()));

        assert!(
            !Ipv4Cidr::new("192.168.1.0".parse::<Ipv4Addr>().unwrap(), 30)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.1.4".parse::<Ipv4Addr>().unwrap(), 30).unwrap())
        );

        assert!(
            Ipv4Cidr::new("192.168.1.0".parse::<Ipv4Addr>().unwrap(), 30)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.1.2".parse::<Ipv4Addr>().unwrap(), 31).unwrap())
        );

        assert!(!Ipv4Cidr::new("10.0.0.0".parse::<Ipv4Addr>().unwrap(), 8)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("172.16.0.0".parse::<Ipv4Addr>().unwrap(), 12).unwrap()));

        assert!(
            !Ipv4Cidr::new("172.16.0.0".parse::<Ipv4Addr>().unwrap(), 12)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 16).unwrap())
        );

        assert!(
            !Ipv4Cidr::new("192.168.0.0".parse::<Ipv4Addr>().unwrap(), 25)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("192.168.0.128".parse::<Ipv4Addr>().unwrap(), 25).unwrap()
                )
        );

        assert!(
            Ipv4Cidr::new("192.168.0.64".parse::<Ipv4Addr>().unwrap(), 26)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("192.168.0.96".parse::<Ipv4Addr>().unwrap(), 27).unwrap())
        );

        assert!(
            !Ipv4Cidr::new("203.0.113.0".parse::<Ipv4Addr>().unwrap(), 31)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("203.0.113.2".parse::<Ipv4Addr>().unwrap(), 31).unwrap())
        );

        assert!(Ipv4Cidr::new("0.0.0.0".parse::<Ipv4Addr>().unwrap(), 0)
            .unwrap()
            .overlaps(&Ipv4Cidr::new("0.0.0.0".parse::<Ipv4Addr>().unwrap(), 0).unwrap()));

        assert!(
            Ipv4Cidr::new("255.255.255.255".parse::<Ipv4Addr>().unwrap(), 0)
                .unwrap()
                .overlaps(&Ipv4Cidr::new("0.0.0.0".parse::<Ipv4Addr>().unwrap(), 32).unwrap())
        );

        assert!(
            Ipv4Cidr::new("255.255.255.255".parse::<Ipv4Addr>().unwrap(), 0)
                .unwrap()
                .overlaps(
                    &Ipv4Cidr::new("255.255.255.255".parse::<Ipv4Addr>().unwrap(), 0).unwrap()
                )
        );
    }

    #[test]
    fn test_ipv6_overlap() {
        assert!(
            Ipv6Cidr::new("2001:db8::0".parse::<Ipv6Addr>().unwrap(), 64)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8::127".parse::<Ipv6Addr>().unwrap(), 64).unwrap()
                )
        );

        assert!(
            !Ipv6Cidr::new("2001:db8:abc:1234::1".parse::<Ipv6Addr>().unwrap(), 64)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8:abc:1235::1".parse::<Ipv6Addr>().unwrap(), 64)
                        .unwrap()
                )
        );

        assert!(
            Ipv6Cidr::new("2001:db8:abc:1235::1".parse::<Ipv6Addr>().unwrap(), 64)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8:abc:1235::7".parse::<Ipv6Addr>().unwrap(), 64)
                        .unwrap()
                )
        );

        assert!(
            Ipv6Cidr::new("2001:db8::200".parse::<Ipv6Addr>().unwrap(), 64)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8::100".parse::<Ipv6Addr>().unwrap(), 70).unwrap()
                )
        );

        assert!(
            Ipv6Cidr::new("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 128)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 128).unwrap())
        );
        assert!(
            !Ipv6Cidr::new("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 128)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::2".parse::<Ipv6Addr>().unwrap(), 128).unwrap())
        );

        assert!(Ipv6Cidr::new("2001:db8::".parse::<Ipv6Addr>().unwrap(), 32)
            .unwrap()
            .overlaps(
                &Ipv6Cidr::new(
                    "2001:db8:cafe:babe::dead:beef".parse::<Ipv6Addr>().unwrap(),
                    128
                )
                .unwrap()
            ));

        assert!(Ipv6Cidr::new("::0".parse::<Ipv6Addr>().unwrap(), 0)
            .unwrap()
            .overlaps(&Ipv6Cidr::new("fe80::".parse::<Ipv6Addr>().unwrap(), 10).unwrap()));

        assert!(!Ipv6Cidr::new("fe80::".parse::<Ipv6Addr>().unwrap(), 10)
            .unwrap()
            .overlaps(&Ipv6Cidr::new("2001:db8::".parse::<Ipv6Addr>().unwrap(), 32).unwrap()));

        assert!(!Ipv6Cidr::new("fc00::".parse::<Ipv6Addr>().unwrap(), 7)
            .unwrap()
            .overlaps(&Ipv6Cidr::new("2001:db8::".parse::<Ipv6Addr>().unwrap(), 32).unwrap()));

        assert!(Ipv6Cidr::new("2001:db8::".parse::<Ipv6Addr>().unwrap(), 16)
            .unwrap()
            .overlaps(
                &Ipv6Cidr::new("2001:db8:1234:5678::abcd".parse::<Ipv6Addr>().unwrap(), 64)
                    .unwrap()
            ));

        assert!(
            !Ipv6Cidr::new("2001:db8:0000::".parse::<Ipv6Addr>().unwrap(), 48)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8:0001::".parse::<Ipv6Addr>().unwrap(), 48).unwrap()
                )
        );

        assert!(
            Ipv6Cidr::new("2001:db8:1234::".parse::<Ipv6Addr>().unwrap(), 48)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new("2001:db8:1234:5678::".parse::<Ipv6Addr>().unwrap(), 64)
                        .unwrap()
                )
        );

        assert!(
            !Ipv6Cidr::new("2001:db8::0".parse::<Ipv6Addr>().unwrap(), 127)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::2".parse::<Ipv6Addr>().unwrap(), 127).unwrap())
        );

        assert!(
            Ipv6Cidr::new("2001:db8::0".parse::<Ipv6Addr>().unwrap(), 127)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 127).unwrap())
        );

        assert!(
            !Ipv6Cidr::new("2001:db8::0".parse::<Ipv6Addr>().unwrap(), 126)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::4".parse::<Ipv6Addr>().unwrap(), 126).unwrap())
        );
        assert!(
            Ipv6Cidr::new("2001:db8::0".parse::<Ipv6Addr>().unwrap(), 126)
                .unwrap()
                .overlaps(&Ipv6Cidr::new("2001:db8::2".parse::<Ipv6Addr>().unwrap(), 127).unwrap())
        );

        assert!(
            Ipv6Cidr::new("2001:db8:1::".parse::<Ipv6Addr>().unwrap(), 64)
                .unwrap()
                .overlaps(
                    &Ipv6Cidr::new(
                        "2001:db8:1:0:ebcd:eebf::efee".parse::<Ipv6Addr>().unwrap(),
                        80
                    )
                    .unwrap()
                )
        );
    }

    #[test]
    fn test_ipv4_canonical() {
        let cidr = Ipv4Cidr::new("192.168.1.100".parse::<Ipv4Addr>().unwrap(), 24).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(canonical.mask, 24);

        let cidr = Ipv4Cidr::new("10.50.75.200".parse::<Ipv4Addr>().unwrap(), 16).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(10, 50, 0, 0));
        assert_eq!(canonical.mask, 16);

        let cidr = Ipv4Cidr::new("172.16.100.50".parse::<Ipv4Addr>().unwrap(), 8).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(172, 0, 0, 0));
        assert_eq!(canonical.mask, 8);

        let cidr = Ipv4Cidr::new("192.168.1.1".parse::<Ipv4Addr>().unwrap(), 32).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(canonical.mask, 32);

        let cidr = Ipv4Cidr::new("255.255.255.255".parse::<Ipv4Addr>().unwrap(), 0).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(0, 0, 0, 0));
        assert_eq!(canonical.mask, 0);

        let cidr = Ipv4Cidr::new("192.168.1.103".parse::<Ipv4Addr>().unwrap(), 30).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(canonical.mask, 30);

        let cidr = Ipv4Cidr::new("10.10.15.128".parse::<Ipv4Addr>().unwrap(), 23).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv4Addr::new(10, 10, 14, 0));
        assert_eq!(canonical.mask, 23);

        let cidr = Ipv4Cidr::new("203.0.113.99".parse::<Ipv4Addr>().unwrap(), 25).unwrap();
        let canonical1 = cidr.canonical();
        let canonical2 = canonical1.canonical();
        assert_eq!(canonical1.addr, canonical2.addr);
        assert_eq!(canonical1.mask, canonical2.mask);
    }

    #[test]
    fn test_ipv6_canonical() {
        let cidr = Ipv6Cidr::new(
            "2001:db8:85a3::8a2e:370:7334".parse::<Ipv6Addr>().unwrap(),
            64,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0x85a3, 0, 0, 0, 0, 0)
        );
        assert_eq!(canonical.mask, 64);

        let cidr = Ipv6Cidr::new(
            "2001:db8:1234:5678:9abc:def0:1234:5678"
                .parse::<Ipv6Addr>()
                .unwrap(),
            48,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0x1234, 0, 0, 0, 0, 0)
        );
        assert_eq!(canonical.mask, 48);

        let cidr = Ipv6Cidr::new(
            "2001:db8:abcd:ef01:2345:6789:abcd:ef01"
                .parse::<Ipv6Addr>()
                .unwrap(),
            32,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)
        );
        assert_eq!(canonical.mask, 32);

        let cidr = Ipv6Cidr::new("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 128).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)
        );
        assert_eq!(canonical.mask, 128);

        let cidr = Ipv6Cidr::new(
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"
                .parse::<Ipv6Addr>()
                .unwrap(),
            0,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));
        assert_eq!(canonical.mask, 0);

        let cidr = Ipv6Cidr::new(
            "2001:db8:1234:5600:ffff:ffff:ffff:ffff"
                .parse::<Ipv6Addr>()
                .unwrap(),
            56,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0x1234, 0x5600, 0, 0, 0, 0)
        );
        assert_eq!(canonical.mask, 56);

        let cidr = Ipv6Cidr::new(
            "2001:db8:1234:5678:9abc:def0:ffff:ffff"
                .parse::<Ipv6Addr>()
                .unwrap(),
            96,
        )
        .unwrap();
        let canonical = cidr.canonical();
        assert_eq!(
            canonical.addr,
            Ipv6Addr::new(0x2001, 0xdb8, 0x1234, 0x5678, 0x9abc, 0xdef0, 0, 0)
        );
        assert_eq!(canonical.mask, 96);

        let cidr = Ipv6Cidr::new(
            "2001:db8:cafe:face:dead:beef:1234:5678"
                .parse::<Ipv6Addr>()
                .unwrap(),
            80,
        )
        .unwrap();
        let canonical1 = cidr.canonical();
        let canonical2 = canonical1.canonical();
        assert_eq!(canonical1.addr, canonical2.addr);
        assert_eq!(canonical1.mask, canonical2.mask);

        let cidr = Ipv6Cidr::new("fe80::1".parse::<Ipv6Addr>().unwrap(), 64).unwrap();
        let canonical = cidr.canonical();
        assert_eq!(canonical.addr, Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0));
        assert_eq!(canonical.mask, 64);
    }
}
