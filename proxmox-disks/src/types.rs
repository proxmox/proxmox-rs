use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::api;

/// S.M.A.R.T. health status.
#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SmartStatus {
    /// Smart tests passed - everything is OK.
    Passed,
    /// Smart tests failed - disk has problems.
    Failed,
    /// Unknown status.
    Unknown,
}

/// Information for a device as returned by lsblk.
#[cfg(feature = "discovery")]
#[derive(Deserialize)]
pub(crate) struct LsblkInfo {
    /// Path to the device.
    pub(crate) path: String,
    /// Partition type GUID.
    #[serde(rename = "parttype")]
    pub(crate) partition_type: Option<String>,
    /// File system label.
    #[serde(rename = "fstype")]
    pub(crate) file_system_type: Option<String>,
    /// File system UUID.
    pub(crate) uuid: Option<String>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
/// This is just a rough estimate for a "type" of disk.
pub enum DiskType {
    /// We know nothing.
    Unknown,

    /// May also be a USB-HDD.
    Hdd,

    /// May also be a USB-SSD.
    Ssd,

    /// Some kind of USB disk, but we don't know more than that.
    Usb,
}

/// Represents the contents of the `/sys/block/<dev>/stat` file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlockDevStat {
    pub read_ios: u64,
    pub read_sectors: u64,
    pub write_ios: u64,
    pub write_sectors: u64,
    pub io_ticks: u64, // milliseconds
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
/// What a block device partition is used for.
pub enum PartitionUsageType {
    /// Partition is not used (as far we can tell)
    Unused,
    /// Partition is used by LVM
    LVM,
    /// Partition is used by ZFS
    ZFS,
    /// Partition is ZFS reserved
    ZfsReserved,
    /// Partition is an EFI partition
    EFI,
    /// Partition is a BIOS partition
    BIOS,
    /// Partition contains a file system label
    FileSystem,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
/// What a block device (disk) is used for.
pub enum DiskUsageType {
    /// Disk is not used (as far we can tell)
    Unused,
    /// Disk is mounted
    Mounted,
    /// Disk is used by LVM
    LVM,
    /// Disk is used by ZFS
    ZFS,
    /// Disk is used by device-mapper
    DeviceMapper,
    /// Disk has partitions
    Partitions,
    /// Disk contains a file system label
    FileSystem,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
/// Basic information about a partition
pub struct PartitionInfo {
    /// The partition name
    pub name: String,
    /// What the partition is used for
    pub used: PartitionUsageType,
    /// Is the partition mounted
    pub mounted: bool,
    /// The filesystem of the partition
    pub filesystem: Option<String>,
    /// The partition devpath
    pub devpath: Option<String>,
    /// Size in bytes
    pub size: Option<u64>,
    /// GPT partition
    pub gpt: bool,
    /// UUID
    pub uuid: Option<String>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        used: {
            type: DiskUsageType,
        },
        "disk-type": {
            type: DiskType,
        },
        partitions: {
            optional: true,
            items: {
                type: PartitionInfo
            }
        }
    }
))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
/// Disk identity and usage information from sysfs/udev/lsblk.
///
/// This type intentionally does not include S.M.A.R.T. health data - use
/// `get_smart_data()` for on-demand queries and combine at the API layer.
pub struct DiskUsageInfo {
    /// Disk name (`/sys/block/<name>`)
    pub name: String,
    pub used: DiskUsageType,
    pub disk_type: DiskType,
    /// Vendor
    pub vendor: Option<String>,
    /// Model
    pub model: Option<String>,
    /// WWN
    pub wwn: Option<String>,
    /// Disk size
    pub size: u64,
    /// Serial number
    pub serial: Option<String>,
    /// Partitions on the device
    pub partitions: Option<Vec<PartitionInfo>>,
    /// Linux device path (/dev/xxx)
    pub devpath: Option<String>,
    /// Set if disk contains a GPT partition table
    pub gpt: bool,
    /// RPM
    pub rpm: Option<u64>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
/// A file system type supported by our tooling.
pub enum FileSystemType {
    /// Linux Ext4
    Ext4,
    /// XFS
    Xfs,
}

impl std::fmt::Display for FileSystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            FileSystemType::Ext4 => "ext4",
            FileSystemType::Xfs => "xfs",
        };
        write!(f, "{text}")
    }
}

impl std::str::FromStr for FileSystemType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ext4" => Ok(Self::Ext4),
            "xfs" => Ok(Self::Xfs),
            other => anyhow::bail!("unknown filesystem type: {other:?}"),
        }
    }
}
