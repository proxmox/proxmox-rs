use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use anyhow::{bail, Error};

use proxmox_schema::api_types::{BLOCKDEVICE_NAME_REGEX, UUID_REGEX};

use crate::{
    get_lvm_devices, zfs_devices, Disk, DiskUsageInfo, DiskUsageType, Disks, LsblkInfo,
    PartitionInfo, PartitionUsageType,
};

static ISCSI_PATH_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"host[^/]*/session[^/]*").unwrap());

/// Use lsblk to read partition type uuids and file system types.
pub(crate) fn get_lsblk_info() -> Result<Vec<LsblkInfo>, Error> {
    let mut command = std::process::Command::new("lsblk");
    command.args(["--json", "-o", "path,parttype,fstype,uuid"]);

    let output = proxmox_sys::command::run_command(command, None)?;

    let mut output: serde_json::Value = output.parse()?;

    Ok(serde_json::from_value(output["blockdevices"].take())?)
}

/// Get set of devices with a file system label.
///
/// The set is indexed by using the unix raw device number (dev_t is u64)
fn get_file_system_devices(lsblk_info: &[LsblkInfo]) -> Result<HashSet<u64>, Error> {
    use std::os::unix::fs::MetadataExt;

    let mut device_set: HashSet<u64> = HashSet::new();

    for info in lsblk_info.iter() {
        if info.file_system_type.is_some() {
            let meta = std::fs::metadata(&info.path)?;
            device_set.insert(meta.rdev());
        }
    }

    Ok(device_set)
}

fn scan_partitions(
    disk_manager: &Arc<Disks>,
    lvm_devices: &HashSet<u64>,
    zfs_devices: &HashSet<u64>,
    device: &str,
) -> Result<DiskUsageType, Error> {
    let mut sys_path = std::path::PathBuf::from("/sys/block");
    sys_path.push(device);

    let mut used = DiskUsageType::Unused;

    let mut found_lvm = false;
    let mut found_zfs = false;
    let mut found_mountpoints = false;
    let mut found_dm = false;
    let mut found_partitions = false;

    for item in proxmox_sys::fs::read_subdir(libc::AT_FDCWD, &sys_path)? {
        let item = item?;
        let name = match item.file_name().to_str() {
            Ok(name) => name,
            Err(_) => continue, // skip non utf8 entries
        };
        if !name.starts_with(device) {
            continue;
        }

        found_partitions = true;

        let mut part_path = sys_path.clone();
        part_path.push(name);

        let data = disk_manager.disk_by_sys_path(&part_path)?;

        let devnum = data.devnum()?;

        if lvm_devices.contains(&devnum) {
            found_lvm = true;
        }

        if data.is_mounted()? {
            found_mountpoints = true;
        }

        if data.has_holders()? {
            found_dm = true;
        }

        if zfs_devices.contains(&devnum) {
            found_zfs = true;
        }
    }

    if found_mountpoints {
        used = DiskUsageType::Mounted;
    } else if found_lvm {
        used = DiskUsageType::LVM;
    } else if found_zfs {
        used = DiskUsageType::ZFS;
    } else if found_dm {
        used = DiskUsageType::DeviceMapper;
    } else if found_partitions {
        used = DiskUsageType::Partitions;
    }

    Ok(used)
}

/// Builder for querying disk usage information.
///
/// This queries sysfs, udev, and lsblk for disk identity and usage data. S.M.A.R.T. health data
/// is intentionally not included - query it separately and merge at the API layer.
pub struct DiskUsageQuery {
    partitions: bool,
}

impl Default for DiskUsageQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskUsageQuery {
    pub const fn new() -> Self {
        Self { partitions: false }
    }

    /// Enable or disable partition information (default: disabled).
    pub fn partitions(mut self, partitions: bool) -> Self {
        self.partitions = partitions;
        self
    }

    /// Query all disks.
    pub fn query(&self) -> Result<HashMap<String, DiskUsageInfo>, Error> {
        get_disks(self, None)
    }

    /// Query a single disk by name.
    pub fn find(&self, disk: &str) -> Result<DiskUsageInfo, Error> {
        let mut map = get_disks(self, Some(vec![disk.to_string()]))?;
        if let Some(info) = map.remove(disk) {
            Ok(info)
        } else {
            bail!("failed to get disk usage info - internal error"); // should not happen
        }
    }

    /// Query multiple disks by name.
    pub fn find_all(&self, disks: Vec<String>) -> Result<HashMap<String, DiskUsageInfo>, Error> {
        get_disks(self, Some(disks))
    }
}

fn get_partitions_info(
    partitions: HashMap<u64, Disk>,
    lvm_devices: &HashSet<u64>,
    zfs_devices: &HashSet<u64>,
    file_system_devices: &HashSet<u64>,
    lsblk_infos: &[LsblkInfo],
) -> Vec<PartitionInfo> {
    partitions
        .values()
        .map(|disk| {
            let devpath = disk
                .device_path()
                .map(|p| p.to_owned())
                .map(|p| p.to_string_lossy().to_string());

            let mut used = PartitionUsageType::Unused;

            if let Ok(devnum) = disk.devnum() {
                if lvm_devices.contains(&devnum) {
                    used = PartitionUsageType::LVM;
                } else if zfs_devices.contains(&devnum) {
                    used = PartitionUsageType::ZFS;
                } else if file_system_devices.contains(&devnum) {
                    used = PartitionUsageType::FileSystem;
                }
            }

            let mounted = disk.is_mounted().unwrap_or(false);
            let mut filesystem = None;
            let mut uuid = None;
            if let Some(devpath) = devpath.as_ref() {
                for info in lsblk_infos.iter().filter(|i| i.path.eq(devpath)) {
                    uuid = info.uuid.clone().filter(|uuid| UUID_REGEX.is_match(uuid));
                    used = match info.partition_type.as_deref() {
                        Some("21686148-6449-6e6f-744e-656564454649") => PartitionUsageType::BIOS,
                        Some("c12a7328-f81f-11d2-ba4b-00a0c93ec93b") => PartitionUsageType::EFI,
                        Some("6a945a3b-1dd2-11b2-99a6-080020736631") => {
                            PartitionUsageType::ZfsReserved
                        }
                        _ => used,
                    };
                    if used == PartitionUsageType::FileSystem {
                        filesystem.clone_from(&info.file_system_type);
                    }
                }
            }

            PartitionInfo {
                name: disk.sysname().to_str().unwrap_or("?").to_string(),
                devpath,
                used,
                mounted,
                filesystem,
                size: disk.size().ok(),
                gpt: disk.has_gpt(),
                uuid,
            }
        })
        .collect()
}

/// Get disk usage information for multiple disks.
fn get_disks(
    opts: &DiskUsageQuery,
    disks: Option<Vec<String>>,
) -> Result<HashMap<String, DiskUsageInfo>, Error> {
    let disk_manager = Arc::new(Disks::new());

    let lsblk_info = get_lsblk_info()?;

    let zfs_devices =
        zfs_devices(&lsblk_info, None).or_else(|err| -> Result<HashSet<u64>, Error> {
            proxmox_log::error!("error getting zfs devices: {err}");
            Ok(HashSet::new())
        })?;

    let lvm_devices = get_lvm_devices(&lsblk_info)?;

    let file_system_devices = get_file_system_devices(&lsblk_info)?;

    // fixme: ceph journals/volumes

    let mut result = HashMap::new();

    for item in proxmox_sys::fs::scan_subdir(libc::AT_FDCWD, "/sys/block", &BLOCKDEVICE_NAME_REGEX)?
    {
        let item = item?;

        let name = match item.file_name().to_str() {
            Ok(name) => name.to_string(),
            Err(_) => continue, // skip non-UTF-8 names
        };

        if let Some(ref disks) = disks
            && !disks.contains(&name)
        {
            continue;
        }

        let sys_path = format!("/sys/block/{name}");

        if let Ok(target) = std::fs::read_link(&sys_path)
            && let Some(target) = target.to_str()
            && ISCSI_PATH_REGEX.is_match(target)
        {
            continue;
        } // skip iSCSI devices

        let disk = disk_manager.disk_by_sys_path(&sys_path)?;

        let devnum = disk.devnum()?;

        let size = match disk.size() {
            Ok(size) => size,
            Err(_) => continue, // skip devices with unreadable size
        };

        let disk_type = match disk.guess_disk_type() {
            Ok(disk_type) => disk_type,
            Err(_) => continue, // skip devices with undetectable type
        };

        let mut usage = DiskUsageType::Unused;

        if lvm_devices.contains(&devnum) {
            usage = DiskUsageType::LVM;
        }

        match disk.is_mounted() {
            Ok(true) => usage = DiskUsageType::Mounted,
            Ok(false) => {}
            Err(_) => continue, // skip devices with undetectable mount status
        }

        if zfs_devices.contains(&devnum) {
            usage = DiskUsageType::ZFS;
        }

        let vendor = disk
            .vendor()
            .unwrap_or(None)
            .map(|s| s.to_string_lossy().trim().to_string());

        let model = disk.model().map(|s| s.to_string_lossy().into_owned());

        let serial = disk.serial().map(|s| s.to_string_lossy().into_owned());

        let devpath = disk
            .device_path()
            .map(|p| p.to_owned())
            .map(|p| p.to_string_lossy().to_string());

        let wwn = disk.wwn().map(|s| s.to_string_lossy().into_owned());

        let partitions: Option<Vec<PartitionInfo>> = if opts.partitions {
            disk.partitions().map_or(None, |parts| {
                Some(get_partitions_info(
                    parts,
                    &lvm_devices,
                    &zfs_devices,
                    &file_system_devices,
                    &lsblk_info,
                ))
            })
        } else {
            None
        };

        if usage != DiskUsageType::Mounted {
            match scan_partitions(&disk_manager, &lvm_devices, &zfs_devices, &name) {
                Ok(part_usage) => {
                    if part_usage != DiskUsageType::Unused {
                        usage = part_usage;
                    }
                }
                Err(_) => continue, // skip devices if scan_partitions fail
            };
        }

        if usage == DiskUsageType::Unused && file_system_devices.contains(&devnum) {
            usage = DiskUsageType::FileSystem;
        }

        if usage == DiskUsageType::Unused && disk.has_holders()? {
            usage = DiskUsageType::DeviceMapper;
        }

        let info = DiskUsageInfo {
            name: name.clone(),
            vendor,
            model,
            partitions,
            serial,
            devpath,
            size,
            wwn,
            disk_type,
            used: usage,
            gpt: disk.has_gpt(),
            rpm: disk.ata_rotation_rate_rpm(),
        };

        result.insert(name, info);
    }

    Ok(result)
}
