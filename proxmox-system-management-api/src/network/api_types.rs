use std::fmt;

use anyhow::{bail, Error};
use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;
use regex::Regex;

use proxmox_schema::api;
use proxmox_schema::ApiStringFormat;
use proxmox_schema::ArraySchema;
use proxmox_schema::Schema;
use proxmox_schema::StringSchema;
use proxmox_schema::api_types::SAFE_ID_REGEX;
use proxmox_schema::api_types::{IP_V4_SCHEMA, IP_V6_SCHEMA};
use proxmox_schema::api_types::{CIDR_V4_SCHEMA, CIDR_V6_SCHEMA};

lazy_static! {
    pub static ref PHYSICAL_NIC_REGEX: Regex = Regex::new(r"^(?:eth\d+|en[^:.]+|ib\d+)$").unwrap();
    pub static ref VLAN_INTERFACE_REGEX: Regex =
        Regex::new(r"^(?P<vlan_raw_device>\S+)\.(?P<vlan_id>\d+)|vlan(?P<vlan_id2>\d+)$").unwrap();
}

pub const NETWORK_INTERFACE_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&SAFE_ID_REGEX);

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Interface configuration method
pub enum NetworkConfigMethod {
    /// Configuration is done manually using other tools
    Manual,
    /// Define interfaces with statically allocated addresses.
    Static,
    /// Obtain an address via DHCP
    DHCP,
    /// Define the loopback interface.
    Loopback,
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
/// Linux Bond Mode
pub enum LinuxBondMode {
    /// Round-robin policy
    BalanceRr = 0,
    /// Active-backup policy
    ActiveBackup = 1,
    /// XOR policy
    BalanceXor = 2,
    /// Broadcast policy
    Broadcast = 3,
    /// IEEE 802.3ad Dynamic link aggregation
    #[serde(rename = "802.3ad")]
    Ieee802_3ad = 4,
    /// Adaptive transmit load balancing
    BalanceTlb = 5,
    /// Adaptive load balancing
    BalanceAlb = 6,
}

impl fmt::Display for LinuxBondMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            LinuxBondMode::BalanceRr => "balance-rr",
            LinuxBondMode::ActiveBackup => "active-backup",
            LinuxBondMode::BalanceXor => "balance-xor",
            LinuxBondMode::Broadcast => "broadcast",
            LinuxBondMode::Ieee802_3ad => "802.3ad",
            LinuxBondMode::BalanceTlb => "balance-tlb",
            LinuxBondMode::BalanceAlb => "balance-alb",
        })
    }
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
/// Bond Transmit Hash Policy for LACP (802.3ad)
pub enum BondXmitHashPolicy {
    /// Layer 2
    Layer2 = 0,
    /// Layer 2+3
    #[serde(rename = "layer2+3")]
    Layer2_3 = 1,
    /// Layer 3+4
    #[serde(rename = "layer3+4")]
    Layer3_4 = 2,
}

impl fmt::Display for BondXmitHashPolicy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            BondXmitHashPolicy::Layer2 => "layer2",
            BondXmitHashPolicy::Layer2_3 => "layer2+3",
            BondXmitHashPolicy::Layer3_4 => "layer3+4",
        })
    }
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Network interface type
pub enum NetworkInterfaceType {
    /// Loopback
    Loopback,
    /// Physical Ethernet device
    Eth,
    /// Linux Bridge
    Bridge,
    /// Linux Bond
    Bond,
    /// Linux VLAN (eth.10)
    Vlan,
    /// Interface Alias (eth:1)
    Alias,
    /// Unknown interface type
    Unknown,
}

pub const NETWORK_INTERFACE_NAME_SCHEMA: Schema = StringSchema::new("Network interface name.")
    .format(&NETWORK_INTERFACE_FORMAT)
    .min_length(1)
    .max_length(15) // libc::IFNAMSIZ-1
    .schema();

pub const NETWORK_INTERFACE_ARRAY_SCHEMA: Schema =
    ArraySchema::new("Network interface list.", &NETWORK_INTERFACE_NAME_SCHEMA).schema();

pub const NETWORK_INTERFACE_LIST_SCHEMA: Schema =
    StringSchema::new("A list of network devices, comma separated.")
        .format(&ApiStringFormat::PropertyString(
            &NETWORK_INTERFACE_ARRAY_SCHEMA,
        ))
        .schema();

#[api(
    properties: {
        name: {
            schema: NETWORK_INTERFACE_NAME_SCHEMA,
        },
        "type": {
            type: NetworkInterfaceType,
        },
        method: {
            type: NetworkConfigMethod,
            optional: true,
        },
        method6: {
            type: NetworkConfigMethod,
            optional: true,
        },
        cidr: {
            schema: CIDR_V4_SCHEMA,
            optional: true,
        },
        cidr6: {
            schema: CIDR_V6_SCHEMA,
            optional: true,
        },
        gateway: {
            schema: IP_V4_SCHEMA,
            optional: true,
        },
        gateway6: {
            schema: IP_V6_SCHEMA,
            optional: true,
        },
        options: {
            description: "Option list (inet)",
            type: Array,
            items: {
                description: "Optional attribute line.",
                type: String,
            },
        },
        options6: {
            description: "Option list (inet6)",
            type: Array,
            items: {
                description: "Optional attribute line.",
                type: String,
            },
        },
        comments: {
            description: "Comments (inet, may span multiple lines)",
            type: String,
            optional: true,
        },
        comments6: {
            description: "Comments (inet6, may span multiple lines)",
            type: String,
            optional: true,
        },
        bridge_ports: {
            schema: NETWORK_INTERFACE_ARRAY_SCHEMA,
            optional: true,
        },
        slaves: {
            schema: NETWORK_INTERFACE_ARRAY_SCHEMA,
            optional: true,
        },
        "vlan-id": {
            description: "VLAN ID.",
            type: u16,
            optional: true,
        },
        "vlan-raw-device": {
            schema: NETWORK_INTERFACE_NAME_SCHEMA,
            optional: true,
        },
        bond_mode: {
            type: LinuxBondMode,
            optional: true,
        },
        "bond-primary": {
            schema: NETWORK_INTERFACE_NAME_SCHEMA,
            optional: true,
        },
        bond_xmit_hash_policy: {
            type: BondXmitHashPolicy,
            optional: true,
        },
    }
)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
/// Network Interface configuration
pub struct Interface {
    /// Autostart interface
    pub autostart: bool,
    /// Interface is active (UP)
    pub active: bool,
    /// Interface name
    pub name: String,
    /// Interface type
    #[serde(rename = "type")]
    pub interface_type: NetworkInterfaceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<NetworkConfigMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method6: Option<NetworkConfigMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// IPv4 address with netmask
    pub cidr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// IPv4 gateway
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// IPv6 address with netmask
    pub cidr6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// IPv6 gateway
    pub gateway6: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options6: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments6: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum Transmission Unit
    pub mtu: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_ports: Option<Vec<String>>,
    /// Enable bridge vlan support.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_vlan_aware: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-id")]
    pub vlan_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-raw-device")]
    pub vlan_raw_device: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub slaves: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<LinuxBondMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bond-primary")]
    pub bond_primary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_xmit_hash_policy: Option<BondXmitHashPolicy>,
}

impl Interface {
    pub fn new(name: String) -> Self {
        Self {
            name,
            interface_type: NetworkInterfaceType::Unknown,
            autostart: false,
            active: false,
            method: None,
            method6: None,
            cidr: None,
            gateway: None,
            cidr6: None,
            gateway6: None,
            options: Vec::new(),
            options6: Vec::new(),
            comments: None,
            comments6: None,
            mtu: None,
            bridge_ports: None,
            bridge_vlan_aware: None,
            vlan_id: None,
            vlan_raw_device: None,
            slaves: None,
            bond_mode: None,
            bond_primary: None,
            bond_xmit_hash_policy: None,
        }
    }

    /// Setter for bridge ports (check if interface type is a bridge)
    pub fn set_bridge_ports(&mut self, ports: Vec<String>) -> Result<(), Error> {
        if self.interface_type != NetworkInterfaceType::Bridge {
            bail!(
                "interface '{}' is no bridge (type is {:?})",
                self.name,
                self.interface_type
            );
        }
        self.bridge_ports = Some(ports);
        Ok(())
    }

    /// Setter for bridge ports (check if interface type is a bridge)
    pub fn set_bridge_port_list(&mut self, ports: &str) -> Result<(), Error> {
        let ports = Self::split_interface_list(ports)?;
        self.set_bridge_ports(ports)
    }

    /// Setter for bond slaves (check if interface type is a bond)
    pub fn set_bond_slaves(&mut self, slaves: Vec<String>) -> Result<(), Error> {
        if self.interface_type != NetworkInterfaceType::Bond {
            bail!(
                "interface '{}' is no bond (type is {:?})",
                self.name,
                self.interface_type
            );
        }
        self.slaves = Some(slaves);
        Ok(())
    }

    /// Setter for bond slaves (check if interface type is a bond)
    pub fn set_bond_slave_list(&mut self, slaves: &str) -> Result<(), Error> {
        let slaves = Self::split_interface_list(slaves)?;
        self.set_bond_slaves(slaves)
    }

    /// Split a network interface list into an array of interface names.
    pub fn split_interface_list(list: &str) -> Result<Vec<String>, Error> {
        let value = NETWORK_INTERFACE_ARRAY_SCHEMA.parse_property_string(list)?;
        Ok(value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect())
    }
}

#[api()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Deletable property name
pub enum DeletableInterfaceProperty {
    /// Delete the IPv4 address property.
    Cidr,
    /// Delete the IPv6 address property.
    Cidr6,
    /// Delete the IPv4 gateway property.
    Gateway,
    /// Delete the IPv6 gateway property.
    Gateway6,
    /// Delete the whole IPv4 configuration entry.
    Method,
    /// Delete the whole IPv6 configuration entry.
    Method6,
    /// Delete IPv4 comments
    Comments,
    /// Delete IPv6 comments
    Comments6,
    /// Delete mtu.
    Mtu,
    /// Delete autostart flag
    Autostart,
    /// Delete bridge ports (set to 'none')
    #[serde(rename = "bridge_ports")]
    BridgePorts,
    /// Delete bridge-vlan-aware flag
    #[serde(rename = "bridge_vlan_aware")]
    BridgeVlanAware,
    /// Delete bond-slaves (set to 'none')
    Slaves,
    /// Delete bond-primary
    BondPrimary,
    /// Delete bond transmit hash policy
    #[serde(rename = "bond_xmit_hash_policy")]
    BondXmitHashPolicy,
}

#[api(
         properties: {
            "type": {
                type: NetworkInterfaceType,
                optional: true,
            },
            autostart: {
                description: "Autostart interface.",
                type: bool,
                optional: true,
            },
            method: {
                type: NetworkConfigMethod,
                optional: true,
            },
            method6: {
                type: NetworkConfigMethod,
                optional: true,
            },
            comments: {
                description: "Comments (inet, may span multiple lines)",
                type: String,
                optional: true,
            },
            comments6: {
                description: "Comments (inet5, may span multiple lines)",
                type: String,
                optional: true,
            },
            cidr: {
                schema: CIDR_V4_SCHEMA,
                optional: true,
            },
            cidr6: {
                schema: CIDR_V6_SCHEMA,
                optional: true,
            },
            gateway: {
                schema: IP_V4_SCHEMA,
                optional: true,
            },
            gateway6: {
                schema: IP_V6_SCHEMA,
                optional: true,
            },
            mtu: {
                description: "Maximum Transmission Unit.",
                optional: true,
                minimum: 46,
                maximum: 65535,
                default: 1500,
            },
            bridge_ports: {
                schema: NETWORK_INTERFACE_LIST_SCHEMA,
                optional: true,
            },
            bridge_vlan_aware: {
                description: "Enable bridge vlan support.",
                type: bool,
                optional: true,
            },
            "vlan-id": {
                description: "VLAN ID.",
                type: u16,
                optional: true,
            },
            "vlan-raw-device": {
                schema: NETWORK_INTERFACE_NAME_SCHEMA,
                optional: true,
            },
            bond_mode: {
                type: LinuxBondMode,
                optional: true,
            },
            "bond-primary": {
                schema: NETWORK_INTERFACE_NAME_SCHEMA,
                optional: true,
            },
            bond_xmit_hash_policy: {
                type: BondXmitHashPolicy,
                optional: true,
            },
            slaves: {
                schema: NETWORK_INTERFACE_LIST_SCHEMA,
                optional: true,
            },
        },
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Update network interface config.
pub struct InterfaceUpdater   {
    #[serde(rename = "type")]
    pub interface_type: Option<NetworkInterfaceType>,
    pub autostart: Option<bool>,
    pub method: Option<NetworkConfigMethod>,
    pub method6: Option<NetworkConfigMethod>,
    pub comments: Option<String>,
    pub comments6: Option<String>,
    pub cidr: Option<String>,
    pub gateway: Option<String>,
    pub cidr6: Option<String>,
    pub gateway6: Option<String>,
    pub mtu: Option<u64>,
    #[serde(rename = "bridge_ports")]
    pub bridge_ports: Option<String>,
    #[serde(rename = "bridge_vlan_aware")]
    pub bridge_vlan_aware: Option<bool>,
    pub vlan_id: Option<u16>,
    pub vlan_raw_device: Option<String>,
    #[serde(rename = "bond_mode")]
    pub bond_mode: Option<LinuxBondMode>,
    pub bond_primary: Option<String>,
    #[serde(rename = "bond_xmit_hash_policy")]
    pub bond_xmit_hash_policy: Option<BondXmitHashPolicy>,
    pub slaves: Option<String>,
}
