#[cfg(feature = "api")]
mod api;
#[cfg(feature = "api")]
pub use crate::api::{init_node_status_api, API_METHOD_GET_STATUS, API_METHOD_REBOOT_OR_SHUTDOWN};

mod types;
pub use crate::types::{
    BootMode, BootModeInformation, KernelVersionInformation, NodeCpuInformation, NodeInformation,
    NodeMemoryCounters, NodePowerCommand, NodeStatus, NodeSwapCounters, StorageStatus,
};
