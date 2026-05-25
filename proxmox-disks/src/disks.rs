use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, Error, format_err};
use libc::dev_t;
use once_cell::sync::OnceCell;

use proxmox_sys::linux::procfs::{MountInfo, mountinfo::Device};

use crate::{BlockDevStat, Disk};

/// Disk management context.
///
/// This provides access to disk information with some caching for faster querying of multiple
/// devices.
///
/// Several methods on [`Disk`](crate::Disk) (such as `disk_by_node`, `disk_by_sys_path`, and
/// `disk_by_name`) require `self: &Arc<Self>`, so callers that need them should wrap the
/// `Disks` in an `Arc` via [`into_arc`](Self::into_arc).
#[derive(Default)]
pub struct Disks {
    mount_info: OnceCell<MountInfo>,
    mounted_devices: OnceCell<HashSet<dev_t>>,
}

impl Disks {
    /// Create a new disk management context.
    ///
    /// Wrap in an [`Arc`] via [`into_arc`](Self::into_arc) if you need the `disk_by_*` methods.
    pub fn new() -> Self {
        Self {
            mount_info: OnceCell::new(),
            mounted_devices: OnceCell::new(),
        }
    }

    /// Wrap this context in an `Arc` for use with `disk_by_*` methods.
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Get the current mount info. This simply caches the result of `MountInfo::read` from the
    /// `proxmox::sys` module.
    pub fn mount_info(&self) -> Result<&MountInfo, Error> {
        self.mount_info.get_or_try_init(MountInfo::read)
    }

    /// Get a `Disk` from a device node (eg. `/dev/sda`).
    pub fn disk_by_node<P: AsRef<Path>>(self: &Arc<Self>, devnode: P) -> io::Result<Disk> {
        let devnode = devnode.as_ref();

        let meta = std::fs::metadata(devnode)?;
        if (meta.mode() & libc::S_IFBLK) == libc::S_IFBLK {
            self.disk_by_dev_num(meta.rdev())
        } else {
            proxmox_lang::io_bail!("not a block device: {:?}", devnode);
        }
    }

    /// Get a `Disk` for a specific device number.
    pub fn disk_by_dev_num(self: &Arc<Self>, devnum: dev_t) -> io::Result<Disk> {
        self.disk_by_sys_path(format!(
            "/sys/dev/block/{}:{}",
            unsafe { libc::major(devnum) },
            unsafe { libc::minor(devnum) },
        ))
    }

    /// Get a `Disk` for a path in `/sys`.
    pub fn disk_by_sys_path<P: AsRef<Path>>(self: &Arc<Self>, path: P) -> io::Result<Disk> {
        let device = udev::Device::from_syspath(path.as_ref())?;
        Ok(Disk::new(self.clone(), device))
    }

    /// Get a `Disk` for a name in `/sys/block/<name>`.
    pub fn disk_by_name(self: &Arc<Self>, name: &str) -> io::Result<Disk> {
        let syspath = format!("/sys/block/{name}");
        self.disk_by_sys_path(syspath)
    }

    /// Get a `Disk` for a name in `/sys/class/block/<name>`.
    pub fn partition_by_name(self: &Arc<Self>, name: &str) -> io::Result<Disk> {
        let syspath = format!("/sys/class/block/{name}");
        self.disk_by_sys_path(syspath)
    }

    /// Gather information about mounted disks:
    fn mounted_devices(&self) -> Result<&HashSet<dev_t>, Error> {
        self.mounted_devices
            .get_or_try_init(|| -> Result<_, Error> {
                let mut mounted = HashSet::new();

                for (_id, mp) in self.mount_info()? {
                    let source = match mp.mount_source.as_deref() {
                        Some(s) => s,
                        None => continue,
                    };

                    let path = Path::new(source);
                    if !path.is_absolute() {
                        continue;
                    }

                    let meta = match std::fs::metadata(path) {
                        Ok(meta) => meta,
                        Err(ref err) if err.kind() == io::ErrorKind::NotFound => continue,
                        Err(other) => return Err(Error::from(other)),
                    };

                    if (meta.mode() & libc::S_IFBLK) != libc::S_IFBLK {
                        // not a block device
                        continue;
                    }

                    mounted.insert(meta.rdev());
                }

                Ok(mounted)
            })
    }

    /// Information about file system type and used device for a path
    ///
    /// Returns tuple (fs_type, device, mount_source)
    pub fn find_mounted_device(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<(String, Device, Option<OsString>)>, Error> {
        let meta = std::fs::metadata(path)?;
        let device = Device::from_dev_t(meta.dev());

        let root_path = std::path::Path::new("/");

        for (_id, entry) in self.mount_info()? {
            if entry.root == root_path && entry.device == device {
                return Ok(Some((
                    entry.fs_type.clone(),
                    entry.device,
                    entry.mount_source.clone(),
                )));
            }
        }

        Ok(None)
    }

    /// Check whether a specific device node is mounted.
    ///
    /// Note that this tries to `stat` the sources of all mount points without caching the result
    /// of doing so, so this is always somewhat expensive.
    pub fn is_devnum_mounted(&self, dev: dev_t) -> Result<bool, Error> {
        self.mounted_devices().map(|mounted| mounted.contains(&dev))
    }

    /// Query [`BlockDevStat`] for the block device underlying a given path.
    ///
    /// This handles both regular block devices (via sysfs) and ZFS datasets (via kstat). The path
    /// is resolved to its mount point to determine the backing device or dataset.
    pub fn blockdev_stat_for_path<P: AsRef<Path>>(&self, path: P) -> Result<BlockDevStat, Error> {
        let (_fs_type, device, _mount_source) = self
            .find_mounted_device(path.as_ref())
            .context("find_mounted_device failed")?
            .ok_or_else(|| {
                format_err!(
                    "could not determine mounted device for path {}",
                    path.as_ref().display()
                )
            })?;

        #[cfg(feature = "discovery")]
        if let Some(source) = _mount_source
            && _fs_type == "zfs"
        {
            let dataset = source.into_string().map_err(|s| {
                format_err!("could not convert {s:?} to string - invalid characters")
            })?;

            return crate::zfs_dataset_stats(&dataset);
        }

        let dev = device.into_dev_t();
        let sys_path = format!(
            "/sys/dev/block/{}:{}",
            unsafe { libc::major(dev) },
            unsafe { libc::minor(dev) }
        );

        crate::disk::read_stat_from_sysfs(Path::new(&sys_path))
            .with_context(|| format!("could not read stats for {}", path.as_ref().display()))?
            .ok_or_else(|| format_err!("could not read disk stats for {}", path.as_ref().display()))
    }
}
