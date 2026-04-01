//! Disk query/management utilities.
//!
//! The crate is organized into feature-gated layers:
//!
//! - **Core** (always available): [`Disks`], [`Disk`], [`BlockDevStat`] - sysfs/udev queries
//!   with no subprocess calls.
//! - **`smart`**: S.M.A.R.T. health queries via `smartctl` with configurable timeout.
//! - **`discovery`**: Full disk enumeration via [`DiskUsageQuery`], including LVM/ZFS detection.
//! - **`operations`**: Mutating disk operations (wipe, partition, format, mount).
//! - **`api-types`**: `proxmox-schema` API type derives.

mod disk;
pub use disk::Disk;
mod disks;
pub use disks::Disks;
mod types;
pub use types::*;

#[cfg(feature = "smart")]
mod smart;
#[cfg(feature = "smart")]
pub use smart::*;
#[cfg(feature = "discovery")]
mod completion;
#[cfg(feature = "discovery")]
pub use completion::*;
#[cfg(feature = "discovery")]
mod lvm;
#[cfg(feature = "discovery")]
pub(crate) use lvm::*;
#[cfg(feature = "discovery")]
mod scan;
#[cfg(feature = "discovery")]
pub use scan::*;
#[cfg(feature = "discovery")]
mod zfs;
#[cfg(feature = "discovery")]
pub use zfs::*;
#[cfg(feature = "discovery")]
mod zpool_list;
#[cfg(feature = "discovery")]
pub use zpool_list::*;
#[cfg(feature = "discovery")]
mod zpool_status;
#[cfg(feature = "discovery")]
pub use zpool_status::*;
#[cfg(feature = "discovery")]
mod parse_helpers;

#[cfg(feature = "operations")]
mod operations;
#[cfg(feature = "operations")]
pub use operations::*;
