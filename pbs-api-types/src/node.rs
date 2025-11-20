use std::ffi::OsStr;

use serde::{Deserialize, Serialize};

use proxmox_auth_api::types::Authid;
#[cfg(feature = "enum-fallback")]
use proxmox_fixed_string::FixedString;
use proxmox_schema::*;

use crate::StorageStatus;

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Node memory usage counters
pub struct NodeMemoryCounters {
    /// Total memory
    pub total: u64,
    /// Used memory
    pub used: u64,
    /// Free memory
    pub free: u64,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Node swap usage counters
pub struct NodeSwapCounters {
    /// Total swap
    pub total: u64,
    /// Used swap
    pub used: u64,
    /// Free swap
    pub free: u64,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Contains general node information such as the fingerprint`
pub struct NodeInformation {
    /// The SSL Fingerprint
    pub fingerprint: String,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
/// The current kernel version (output of `uname`)
pub struct KernelVersionInformation {
    /// The systemname/nodename
    pub sysname: String,
    /// The kernel release number
    pub release: String,
    /// The kernel version
    pub version: String,
    /// The machine architecture
    pub machine: String,
}

impl KernelVersionInformation {
    pub fn from_uname_parts(
        sysname: &OsStr,
        release: &OsStr,
        version: &OsStr,
        machine: &OsStr,
    ) -> Self {
        KernelVersionInformation {
            sysname: sysname.to_str().map(String::from).unwrap_or_default(),
            release: release.to_str().map(String::from).unwrap_or_default(),
            version: version.to_str().map(String::from).unwrap_or_default(),
            machine: machine.to_str().map(String::from).unwrap_or_default(),
        }
    }

    pub fn get_legacy(&self) -> String {
        format!("{} {} {}", self.sysname, self.release, self.version)
    }
}

#[api]
#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "kebab-case")]
/// The possible BootModes
pub enum BootMode {
    /// The BootMode is EFI/UEFI
    Efi,
    /// The BootMode is Legacy BIOS
    LegacyBios,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

#[api]
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
/// Holds the Bootmodes
pub struct BootModeInformation {
    /// The BootMode, either Efi or Bios
    pub mode: BootMode,
    /// SecureBoot status
    pub secureboot: bool,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Information about the CPU
pub struct NodeCpuInformation {
    /// The CPU model
    pub model: String,
    /// The number of CPU sockets
    pub sockets: usize,
    /// The number of CPU cores (incl. threads)
    pub cpus: usize,
}

#[api(
    properties: {
        memory: {
            type: NodeMemoryCounters,
        },
        root: {
            type: StorageStatus,
        },
        swap: {
            type: NodeSwapCounters,
        },
        loadavg: {
            type: Array,
            items: {
                type: Number,
                description: "the load",
            }
        },
        cpuinfo: {
            type: NodeCpuInformation,
        },
        info: {
            type: NodeInformation,
        }
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// The Node status
pub struct NodeStatus {
    pub memory: NodeMemoryCounters,
    pub root: StorageStatus,
    pub swap: NodeSwapCounters,
    /// The current uptime of the server.
    pub uptime: u64,
    /// Load for 1, 5 and 15 minutes.
    pub loadavg: [f64; 3],
    /// The current kernel version (NEW struct type).
    pub current_kernel: KernelVersionInformation,
    /// The current kernel version (LEGACY string type).
    pub kversion: String,
    /// Total CPU usage since last query.
    pub cpu: f64,
    /// Total IO wait since last query.
    pub wait: f64,
    pub cpuinfo: NodeCpuInformation,
    pub info: NodeInformation,
    /// Current boot mode
    pub boot_info: BootModeInformation,
}

#[api(
    properties: {
        port: {
            type: Integer,
        },
        ticket: {
            type: String,
        },
        upid: {
            type: String,
        },
        user: {
            type: String,
        },
    },
)]
/// Ticket used for authenticating a VNC websocket upgrade request.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeShellTicket {
    /// port used to bind termproxy to
    pub port: u16,

    /// ticket used to verifiy websocket connection
    pub ticket: String,

    /// UPID for termproxy worker task
    pub upid: String,

    /// user or authid encoded in the ticket
    pub user: Authid,
}
