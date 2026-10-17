//! Types for Linux bonding interfaces.

use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::api;

#[cfg_attr(feature = "api-types", api())]
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

#[cfg_attr(feature = "api-types", api())]
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
