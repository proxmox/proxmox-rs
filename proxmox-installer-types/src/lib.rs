//! Defines API types used within the installer, primarily for interacting
//! with proxmox-auto-installer.
//!
//! [`BTreeMap`]s are used to store certain properties to keep the order of
//! them stable, compared to storing them in an ordinary [`HashMap`].

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unsafe_code, missing_docs)]

pub mod answer;
pub mod post_hook;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use proxmox_network_types::mac_address::MacAddress;

/// Default placeholder value for the administrator email address.
pub const EMAIL_DEFAULT_PLACEHOLDER: &str = "mail@example.invalid";

#[derive(Copy, Clone, Eq, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Whether the system boots using legacy BIOS or (U)EFI.
pub enum BootType {
    /// System boots using legacy BIOS.
    Bios,
    /// System boots using (U)EFI.
    Efi,
}

/// Uses a BTreeMap to have the keys sorted
pub type UdevProperties = BTreeMap<String, String>;

#[derive(Clone, Deserialize, Debug)]
/// Information extracted from udev about devices present in the system.
pub struct UdevInfo {
    /// udev information for each disk.
    pub disks: BTreeMap<String, UdevProperties>,
    /// udev information for each network interface card.
    pub nics: BTreeMap<String, UdevProperties>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// Information about the hardware and installer in use.
pub struct SystemInfo {
    /// Information about the product to be installed.
    pub product: ProductConfig,
    /// Information about the ISO.
    pub iso: IsoInfo,
    /// Raw DMI information of the system.
    pub dmi: SystemDMI,
    /// Network devices present on the system.
    pub network_interfaces: Vec<NetworkInterface>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// The per-product configuration of the installer.
pub struct ProductConfig {
    /// Full name of the product.
    pub fullname: String,
    /// The actual product the installer is for.
    pub product: ProxmoxProduct,
    /// Whether to enable installations on Btrfs.
    pub enable_btrfs: bool,
}

impl ProductConfig {
    /// A mocked ProductConfig simulating a Proxmox VE environment.
    pub fn mocked() -> Self {
        Self {
            fullname: String::from("Proxmox VE (mocked)"),
            product: ProxmoxProduct::Pve,
            enable_btrfs: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// Information about the ISO itself.
pub struct IsoInfo {
    /// Version of the product.
    pub release: String,
    /// Version of the ISO itself, e.g. the spin.
    pub isorelease: String,
}

impl IsoInfo {
    /// A mocked IsoInfo with some edge case to convey that this is not necessarily purely numeric.
    pub fn mocked() -> Self {
        Self {
            release: String::from("42.1"),
            isorelease: String::from("mocked-1"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// Collection of various DMI information categories.
pub struct SystemDMI {
    /// Information about the system baseboard.
    pub baseboard: HashMap<String, String>,
    /// Information about the system chassis.
    pub chassis: HashMap<String, String>,
    /// Information about the hardware itself, mostly identifiers.
    pub system: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
/// A unique network interface.
pub struct NetworkInterface {
    /// The network link name
    pub link: String,
    /// The MAC address of the network device
    pub mac: MacAddress,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
/// The name of the product.
pub enum ProxmoxProduct {
    /// Proxmox Virtual Environment
    Pve,
    /// Proxmox Backup Server
    Pbs,
    /// Proxmox Mail Gateway
    Pmg,
    /// Proxmox Datacenter Manager
    Pdm,
}

serde_plain::derive_fromstr_from_deserialize!(ProxmoxProduct);
serde_plain::derive_display_from_serialize!(ProxmoxProduct);

impl ProxmoxProduct {
    /// Returns the full name for the given product.
    pub fn full_name(&self) -> &str {
        match self {
            Self::Pve => "Proxmox Virtual Environment",
            Self::Pbs => "Proxmox Backup Server",
            Self::Pmg => "Proxmox Mail Gateway",
            Self::Pdm => "Proxmox Datacenter Manager",
        }
    }
}
