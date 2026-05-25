use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Error, format_err};
use libc::dev_t;
use once_cell::sync::OnceCell;

use proxmox_lang::io_format_err;

use crate::{BlockDevStat, DiskType, Disks};

/// Queries (and caches) various information about a specific disk.
///
/// This belongs to a `Disks` and provides information for a single disk.
pub struct Disk {
    manager: Arc<Disks>,
    device: udev::Device,
    info: DiskInfo,
}

/// Lazily cached disk properties, populated on first access from sysfs/udev.
#[derive(Default)]
struct DiskInfo {
    size: OnceCell<u64>,
    vendor: OnceCell<Option<OsString>>,
    model: OnceCell<Option<OsString>>,
    rotational: OnceCell<Option<bool>>,
    ata_rotation_rate_rpm: OnceCell<Option<u64>>,
    device_path: OnceCell<Option<PathBuf>>,
    wwn: OnceCell<Option<OsString>>,
    serial: OnceCell<Option<OsString>>,
    partition_table_type: OnceCell<Option<OsString>>,
    partition_entry_scheme: OnceCell<Option<OsString>>,
    partition_entry_uuid: OnceCell<Option<OsString>>,
    partition_entry_type: OnceCell<Option<OsString>>,
    gpt: OnceCell<bool>,
    bus: OnceCell<Option<OsString>>,
    fs_type: OnceCell<Option<OsString>>,
    has_holders: OnceCell<bool>,
    is_mounted: OnceCell<bool>,
}

impl Disk {
    /// Create a new `Disk` from a udev device and its managing context.
    pub(crate) fn new(manager: Arc<Disks>, device: udev::Device) -> Self {
        Self {
            manager,
            device,
            info: Default::default(),
        }
    }

    /// Try to get the device number for this disk.
    ///
    /// (In udev this can fail...)
    pub fn devnum(&self) -> Result<dev_t, Error> {
        // not sure when this can fail...
        self.device
            .devnum()
            .ok_or_else(|| format_err!("failed to get device number"))
    }

    /// Get the sys-name of this device. (The final component in the `/sys` path).
    pub fn sysname(&self) -> &OsStr {
        self.device.sysname()
    }

    /// Get the this disk's `/sys` path.
    pub fn syspath(&self) -> &Path {
        self.device.syspath()
    }

    /// Get the device node in `/dev`, if any.
    pub fn device_path(&self) -> Option<&Path> {
        //self.device.devnode()
        self.info
            .device_path
            .get_or_init(|| self.device.devnode().map(Path::to_owned))
            .as_ref()
            .map(PathBuf::as_path)
    }

    /// Get the parent device.
    pub fn parent(&self) -> Option<Self> {
        self.device.parent().map(|parent| Self {
            manager: self.manager.clone(),
            device: parent,
            info: Default::default(),
        })
    }

    /// Read from a file in this device's sys path.
    ///
    /// Note: path must be a relative path!
    pub fn read_sys(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        if !path.is_relative() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must be relative",
            ));
        }

        std::fs::read(self.syspath().join(path))
            .map(Some)
            .or_else(|err| {
                if err.kind() == io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(err)
                }
            })
    }

    /// Convenience wrapper for reading a `/sys` file which contains just a simple `OsString`.
    pub fn read_sys_os_str<P: AsRef<Path>>(&self, path: P) -> io::Result<Option<OsString>> {
        Ok(self.read_sys(path.as_ref())?.map(|mut v| {
            if Some(&b'\n') == v.last() {
                v.pop();
            }
            OsString::from_vec(v)
        }))
    }

    /// Convenience wrapper for reading a `/sys` file which contains just a simple utf-8 string.
    pub fn read_sys_str<P: AsRef<Path>>(&self, path: P) -> io::Result<Option<String>> {
        Ok(match self.read_sys(path.as_ref())? {
            Some(data) => Some(String::from_utf8(data).map_err(io::Error::other)?),
            None => None,
        })
    }

    /// Convenience wrapper for unsigned integer `/sys` values up to 64 bit.
    pub fn read_sys_u64<P: AsRef<Path>>(&self, path: P) -> io::Result<Option<u64>> {
        Ok(match self.read_sys_str(path)? {
            Some(data) => Some(data.trim().parse().map_err(io::Error::other)?),
            None => None,
        })
    }

    /// Get the disk's size in bytes.
    pub fn size(&self) -> io::Result<u64> {
        Ok(*self.info.size.get_or_try_init(|| {
            self.read_sys_u64("size")?.map(|s| s * 512).ok_or_else(|| {
                io_format_err!(
                    "failed to get disk size from {:?}",
                    self.syspath().join("size"),
                )
            })
        })?)
    }

    /// Get the device vendor (`/sys/.../device/vendor`) entry if available.
    pub fn vendor(&self) -> io::Result<Option<&OsStr>> {
        Ok(self
            .info
            .vendor
            .get_or_try_init(|| self.read_sys_os_str("device/vendor"))?
            .as_ref()
            .map(OsString::as_os_str))
    }

    /// Get the device model (`/sys/.../device/model`) entry if available.
    pub fn model(&self) -> Option<&OsStr> {
        self.info
            .model
            .get_or_init(|| self.device.property_value("ID_MODEL").map(OsStr::to_owned))
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Check whether this is a rotational disk.
    ///
    /// Returns `None` if there's no `queue/rotational` file, in which case no information is
    /// known. `Some(false)` if `queue/rotational` is zero, `Some(true)` if it has a non-zero
    /// value.
    pub fn rotational(&self) -> io::Result<Option<bool>> {
        Ok(*self
            .info
            .rotational
            .get_or_try_init(|| -> io::Result<Option<bool>> {
                Ok(self.read_sys_u64("queue/rotational")?.map(|n| n != 0))
            })?)
    }

    /// Get the WWN if available.
    pub fn wwn(&self) -> Option<&OsStr> {
        self.info
            .wwn
            .get_or_init(|| self.device.property_value("ID_WWN").map(|v| v.to_owned()))
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Get the device serial if available.
    pub fn serial(&self) -> Option<&OsStr> {
        self.info
            .serial
            .get_or_init(|| {
                self.device
                    .property_value("ID_SERIAL_SHORT")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Get the ATA rotation rate value from udev. This is not necessarily the same as sysfs'
    /// `rotational` value.
    pub fn ata_rotation_rate_rpm(&self) -> Option<u64> {
        *self.info.ata_rotation_rate_rpm.get_or_init(|| {
            std::str::from_utf8(
                self.device
                    .property_value("ID_ATA_ROTATION_RATE_RPM")?
                    .as_bytes(),
            )
            .ok()?
            .parse()
            .ok()
        })
    }

    /// Get the partition table type, if any.
    pub fn partition_table_type(&self) -> Option<&OsStr> {
        self.info
            .partition_table_type
            .get_or_init(|| {
                self.device
                    .property_value("ID_PART_TABLE_TYPE")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Check if this contains a GPT partition table.
    pub fn has_gpt(&self) -> bool {
        *self.info.gpt.get_or_init(|| {
            self.partition_table_type()
                .map(|s| s == "gpt")
                .unwrap_or(false)
        })
    }

    /// Get the partitioning scheme of which this device is a partition.
    pub fn partition_entry_scheme(&self) -> Option<&OsStr> {
        self.info
            .partition_entry_scheme
            .get_or_init(|| {
                self.device
                    .property_value("ID_PART_ENTRY_SCHEME")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Check if this is a partition.
    pub fn is_partition(&self) -> bool {
        self.partition_entry_scheme().is_some()
    }

    /// Get the type of partition entry (ie. type UUID from the entry in the GPT partition table).
    pub fn partition_entry_type(&self) -> Option<&OsStr> {
        self.info
            .partition_entry_type
            .get_or_init(|| {
                self.device
                    .property_value("ID_PART_ENTRY_TYPE")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Get the partition entry UUID (ie. the UUID from the entry in the GPT partition table).
    pub fn partition_entry_uuid(&self) -> Option<&OsStr> {
        self.info
            .partition_entry_uuid
            .get_or_init(|| {
                self.device
                    .property_value("ID_PART_ENTRY_UUID")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Get the bus type used for this disk.
    pub fn bus(&self) -> Option<&OsStr> {
        self.info
            .bus
            .get_or_init(|| self.device.property_value("ID_BUS").map(|v| v.to_owned()))
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Attempt to guess the disk type.
    pub fn guess_disk_type(&self) -> io::Result<DiskType> {
        Ok(match self.rotational()? {
            Some(false) => DiskType::Ssd,
            Some(true) => DiskType::Hdd,
            None => match self.ata_rotation_rate_rpm() {
                Some(_) => DiskType::Hdd,
                None => match self.bus() {
                    Some(bus) if bus == "usb" => DiskType::Usb,
                    _ => DiskType::Unknown,
                },
            },
        })
    }

    /// Get the file system type found on the disk, if any.
    ///
    /// Note that `None` may also just mean "unknown".
    pub fn fs_type(&self) -> Option<&OsStr> {
        self.info
            .fs_type
            .get_or_init(|| {
                self.device
                    .property_value("ID_FS_TYPE")
                    .map(|v| v.to_owned())
            })
            .as_ref()
            .map(OsString::as_os_str)
    }

    /// Check if there are any "holders" in `/sys`. This usually means the device is in use by
    /// another kernel driver like the device mapper.
    pub fn has_holders(&self) -> io::Result<bool> {
        Ok(*self
            .info
            .has_holders
            .get_or_try_init(|| -> io::Result<bool> {
                let mut subdir = self.syspath().to_owned();
                subdir.push("holders");
                for entry in std::fs::read_dir(subdir)? {
                    match entry?.file_name().as_bytes() {
                        b"." | b".." => (),
                        _ => return Ok(true),
                    }
                }
                Ok(false)
            })?)
    }

    /// Check if this disk is mounted.
    pub fn is_mounted(&self) -> Result<bool, Error> {
        Ok(*self
            .info
            .is_mounted
            .get_or_try_init(|| self.manager.is_devnum_mounted(self.devnum()?))?)
    }

    /// Read block device stats
    ///
    /// see <https://www.kernel.org/doc/Documentation/block/stat.txt>
    pub fn read_stat(&self) -> std::io::Result<Option<BlockDevStat>> {
        read_stat_from_sysfs(self.syspath())
    }

    /// List device partitions
    pub fn partitions(&self) -> Result<HashMap<u64, Disk>, Error> {
        let sys_path = self.syspath();
        let device = self.sysname().to_string_lossy().to_string();

        let mut map = HashMap::new();

        for item in proxmox_sys::fs::read_subdir(libc::AT_FDCWD, sys_path)? {
            let item = item?;
            let name = match item.file_name().to_str() {
                Ok(name) => name,
                Err(_) => continue, // skip non utf8 entries
            };

            if !name.starts_with(&device) {
                continue;
            }

            let mut part_path = sys_path.to_owned();
            part_path.push(name);

            let disk_part = self.manager.disk_by_sys_path(&part_path)?;

            if let Some(partition) = disk_part.read_sys_u64("partition")? {
                map.insert(partition, disk_part);
            }
        }

        Ok(map)
    }
}

/// Read block device I/O statistics from sysfs.
///
/// The `sys_path` must point to a block device directory in sysfs (for example,
/// `/sys/dev/block/8:0` or `/sys/block/sda`). This reads the `stat` file and parses the kernel's
/// block device statistics.
///
/// See <https://www.kernel.org/doc/Documentation/block/stat.txt>
pub(crate) fn read_stat_from_sysfs(sys_path: &Path) -> std::io::Result<Option<BlockDevStat>> {
    let stat_path = sys_path.join("stat");
    let stat = match std::fs::read(&stat_path) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let stat = std::str::from_utf8(&stat).map_err(std::io::Error::other)?;
    let stat: Vec<u64> = stat
        .split_ascii_whitespace()
        .map(|s| s.parse().unwrap_or_default())
        .collect();

    if stat.len() < 15 {
        return Ok(None);
    }

    Ok(Some(BlockDevStat {
        read_ios: stat[0],
        read_sectors: stat[2],
        write_ios: stat[4] + stat[11],     // write + discard
        write_sectors: stat[6] + stat[13], // write + discard
        io_ticks: stat[10],
    }))
}
