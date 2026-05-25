use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::Path;

use anyhow::{Context as _, Error, bail};

use proxmox_log::info;

use crate::{Disk, FileSystemType};

/// Try to reload the partition table
pub fn reread_partition_table(disk: &Disk) -> Result<(), Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let mut command = std::process::Command::new("blockdev");
    command.arg("--rereadpt");
    command.arg(disk_path);

    proxmox_sys::command::run_command(command, None)?;

    Ok(())
}

/// Initialize disk by writing a GPT partition table
pub fn initialize_gpt_disk(disk: &Disk, uuid: Option<&str>) -> Result<(), Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let uuid = uuid.unwrap_or("R"); // R .. random disk GUID

    let mut command = std::process::Command::new("sgdisk");
    command.arg(disk_path);
    command.args(["-U", uuid]);

    proxmox_sys::command::run_command(command, None)?;

    Ok(())
}

/// Wipes all labels, the first 200 MiB, and the last 4096 bytes of a disk/partition.
/// If called with a partition, also sets the partition type to 0x83 'Linux filesystem'.
pub fn wipe_blockdev(disk: &Disk) -> Result<(), Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let is_partition = disk.is_partition();

    let mut to_wipe: Vec<std::path::PathBuf> = Vec::new();

    let partitions_map = disk.partitions()?;
    for part_disk in partitions_map.values() {
        let part_path = match part_disk.device_path() {
            Some(path) => path,
            None => bail!("disk {:?} has no node in /dev", part_disk.syspath()),
        };
        to_wipe.push(part_path.to_path_buf());
    }

    to_wipe.push(disk_path.to_path_buf());

    info!("Wiping block device {}", disk_path.display());

    let mut wipefs_command = std::process::Command::new("wipefs");
    wipefs_command.arg("--all").args(&to_wipe);

    let wipefs_output = proxmox_sys::command::run_command(wipefs_command, None)?;
    info!("wipefs output: {wipefs_output}");

    zero_disk_start_and_end(disk)?;

    if is_partition {
        // set the partition type to 0x83 'Linux filesystem'
        change_parttype(disk, "8300")?;
    }

    Ok(())
}

pub(crate) fn zero_disk_start_and_end(disk: &Disk) -> Result<(), Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let disk_size = disk.size()?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DSYNC)
        .open(disk_path)
        .with_context(|| format!("failed to open device {disk_path:?} for writing"))?;
    let write_size = disk_size.min(200 * 1024 * 1024);
    let zeroes = proxmox_io::boxed::zeroed(write_size as usize);
    file.write_all_at(&zeroes, 0)
        .with_context(|| format!("failed to wipe start of device {disk_path:?}"))?;
    if disk_size > write_size {
        file.write_all_at(&zeroes[0..4096], disk_size - 4096)
            .with_context(|| format!("failed to wipe end of device {disk_path:?}"))?;
    }
    Ok(())
}

pub(crate) fn change_parttype(part_disk: &Disk, part_type: &str) -> Result<(), Error> {
    let part_path = match part_disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", part_disk.syspath()),
    };
    if let Ok(stat) = nix::sys::stat::stat(part_path) {
        let mut sgdisk_command = std::process::Command::new("sgdisk");
        let major = unsafe { libc::major(stat.st_rdev) };
        let minor = unsafe { libc::minor(stat.st_rdev) };
        let partnum_path = &format!("/sys/dev/block/{major}:{minor}/partition");
        let partnum: u32 = std::fs::read_to_string(partnum_path)?.trim_end().parse()?;
        sgdisk_command.arg(format!("-t{partnum}:{part_type}"));
        let part_disk_parent = match part_disk.parent() {
            Some(disk) => disk,
            None => bail!("disk {:?} has no node in /dev", part_disk.syspath()),
        };
        let part_disk_parent_path = match part_disk_parent.device_path() {
            Some(path) => path,
            None => bail!("disk {:?} has no node in /dev", part_disk.syspath()),
        };
        sgdisk_command.arg(part_disk_parent_path);
        let sgdisk_output = proxmox_sys::command::run_command(sgdisk_command, None)?;
        info!("sgdisk output: {sgdisk_output}");
    }
    Ok(())
}

/// Create a single linux partition using the whole available space
pub fn create_single_linux_partition(disk: &Disk) -> Result<Disk, Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let mut command = std::process::Command::new("sgdisk");
    command.args(["-n1", "-t1:8300"]);
    command.arg(disk_path);

    proxmox_sys::command::run_command(command, None)?;

    let mut partitions = disk.partitions()?;

    match partitions.remove(&1) {
        Some(partition) => Ok(partition),
        None => bail!("unable to lookup device partition"),
    }
}

/// Create a file system on a disk or disk partition
pub fn create_file_system(disk: &Disk, fs_type: FileSystemType) -> Result<(), Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let fs_type = fs_type.to_string();

    let mut command = std::process::Command::new("mkfs");
    command.args(["-t", &fs_type]);
    command.arg(disk_path);

    proxmox_sys::command::run_command(command, None)?;

    Ok(())
}

/// Read the FS UUID (parse blkid output)
///
/// Note: Calling blkid is more reliable than using the udev ID_FS_UUID property.
pub fn get_fs_uuid(disk: &Disk) -> Result<String, Error> {
    let disk_path = match disk.device_path() {
        Some(path) => path,
        None => bail!("disk {:?} has no node in /dev", disk.syspath()),
    };

    let mut command = std::process::Command::new("blkid");
    command.args(["-o", "export"]);
    command.arg(disk_path);

    let output = proxmox_sys::command::run_command(command, None)?;

    for line in output.lines() {
        if let Some(uuid) = line.strip_prefix("UUID=") {
            return Ok(uuid.to_string());
        }
    }

    bail!("get_fs_uuid failed - missing UUID");
}

/// Mount a disk by its UUID and the mount point.
pub fn mount_by_uuid(uuid: &str, mount_point: &Path) -> Result<(), Error> {
    let mut command = std::process::Command::new("mount");
    command.arg(format!("UUID={uuid}"));
    command.arg(mount_point);

    proxmox_sys::command::run_command(command, None)?;
    Ok(())
}

/// Create bind mount.
pub fn bind_mount(path: &Path, target: &Path) -> Result<(), Error> {
    let mut command = std::process::Command::new("mount");
    command.arg("--bind");
    command.arg(path);
    command.arg(target);

    proxmox_sys::command::run_command(command, None)?;
    Ok(())
}

/// Unmount a disk by its mount point.
pub fn unmount_by_mountpoint(path: &Path) -> Result<(), Error> {
    let mut command = std::process::Command::new("umount");
    command.arg(path);

    proxmox_sys::command::run_command(command, None)?;
    Ok(())
}
