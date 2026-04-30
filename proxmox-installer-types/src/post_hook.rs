//! Defines API types for the proxmox-auto-installer post-installation hook.

use serde::{Deserialize, Serialize};

use proxmox_network_types::ip_address::Cidr;

use crate::{
    answer::{FilesystemType, RebootMode},
    BootType, IsoInfo, ProxmoxProduct, SystemDMI, UdevProperties,
};

/// Re-export for convenience, since this is public API
pub use proxmox_node_status::KernelVersionInformation;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
/// Information about the system boot status.
pub struct BootInfo {
    /// Whether the system is booted using UEFI or legacy BIOS.
    pub mode: BootType,
    /// Whether SecureBoot is enabled for the installation.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub secureboot: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
/// Holds all the public keys for the different algorithms available.
pub struct SshPublicHostKeys {
    /// ECDSA-based public host key
    pub ecdsa: String,
    /// ED25519-based public host key
    pub ed25519: String,
    /// RSA-based public host key
    pub rsa: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Holds information about a single disk in the system.
pub struct DiskInfo {
    /// Size in bytes
    pub size: u64,
    /// Set to true if the disk is used for booting.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub is_bootdisk: bool,
    /// Properties about the device as given by udev.
    pub udev_properties: UdevProperties,
}

/// Holds information about the management network interface.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct NetworkInterfaceInfo {
    /// Name of the interface
    pub name: String,
    /// MAC address of the interface
    pub mac: String,
    /// (Designated) IP address of the interface
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Cidr>,
    /// Set to true if the interface is the chosen management interface during
    /// installation.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub is_management: bool,
    /// Set to true if the network interface name was pinned based on the MAC
    /// address during the installation.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub is_pinned: bool,
    /// Properties about the device as given by udev.
    pub udev_properties: UdevProperties,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Information about the installed product itself.
pub struct ProductInfo {
    /// Full name of the product
    pub fullname: String,
    /// Product abbreviation
    pub short: ProxmoxProduct,
    /// Version of the installed product
    pub version: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
/// Information about the CPU(s) installed in the system
pub struct CpuInfo {
    /// Number of physical CPU cores.
    pub cores: usize,
    /// Number of logical CPU cores aka. threads.
    pub cpus: usize,
    /// CPU feature flag set as a space-delimited list.
    pub flags: String,
    /// Whether hardware-accelerated virtualization is supported.
    pub hvm: bool,
    /// Reported model of the CPU(s)
    pub model: String,
    /// Number of physical CPU sockets
    pub sockets: usize,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Metadata of the hook, such as schema version of the document.
pub struct PostHookInfoSchema {
    /// major.minor version describing the schema version of this document, in a semanticy-version
    /// way.
    ///
    /// major: Incremented for incompatible/breaking API changes, e.g. removing an existing
    /// field.
    /// minor: Incremented when adding functionality in a backwards-compatible matter, e.g.
    /// adding a new field.
    pub version: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// All data sent as request payload with the post-installation-webhook POST request.
///
/// NOTE: The format is versioned through `schema.version` (`$schema.version` in the
/// resulting JSON), ensure you update it when this struct or any of its members gets modified.
pub struct PostHookInfo {
    // This field is prefixed by `$` on purpose, to indicate that it is document metadata and not
    // part of the actual content itself. (E.g. JSON Schema uses a similar naming scheme)
    #[serde(rename = "$schema")]
    /// Schema version information for this struct instance.
    pub schema: PostHookInfoSchema,
    /// major.minor version of Debian as installed, retrieved from /etc/debian_version
    pub debian_version: String,
    /// PVE/PMG/PBS/PDM version as reported by `pveversion`, `pmgversion`,
    /// `proxmox-backup-manager version` or `proxmox-datacenter-manager version`, respectively.
    pub product: ProductInfo,
    /// Release information for the ISO used for the installation.
    pub iso: IsoInfo,
    /// Installed kernel version
    pub kernel_version: KernelVersionInformation,
    /// Describes the boot mode of the machine and the SecureBoot status.
    pub boot_info: BootInfo,
    /// Information about the installed CPU(s)
    pub cpu_info: CpuInfo,
    /// DMI information about the system
    pub dmi: SystemDMI,
    /// Filesystem used for boot disk(s)
    pub filesystem: FilesystemType,
    /// Fully qualified domain name of the installed system
    pub fqdn: String,
    /// Unique systemd-id128 identifier of the installed system (128-bit, 16 bytes)
    pub machine_id: String,
    /// All disks detected on the system.
    pub disks: Vec<DiskInfo>,
    /// All network interfaces detected on the system.
    pub network_interfaces: Vec<NetworkInterfaceInfo>,
    /// Public parts of SSH host keys of the installed system
    pub ssh_public_host_keys: SshPublicHostKeys,
    /// Action to will be performed, i.e. either reboot or power off the machine.
    pub reboot_mode: RebootMode,
}

fn bool_is_false(value: &bool) -> bool {
    !value
}
