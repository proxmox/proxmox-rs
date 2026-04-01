use std::collections::HashMap;

use proxmox_schema::api_types::{
    BLOCKDEVICE_DISK_AND_PARTITION_NAME_REGEX, BLOCKDEVICE_NAME_REGEX,
};

/// Block device name completion helper
pub fn complete_disk_name(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    let dir =
        match proxmox_sys::fs::scan_subdir(libc::AT_FDCWD, "/sys/block", &BLOCKDEVICE_NAME_REGEX) {
            Ok(dir) => dir,
            Err(_) => return vec![],
        };

    dir.flatten()
        .filter_map(|item| item.file_name().to_str().ok().map(String::from))
        .collect()
}

/// Block device partition name completion helper
pub fn complete_partition_name(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    let dir = match proxmox_sys::fs::scan_subdir(
        libc::AT_FDCWD,
        "/sys/class/block",
        &BLOCKDEVICE_DISK_AND_PARTITION_NAME_REGEX,
    ) {
        Ok(dir) => dir,
        Err(_) => return vec![],
    };

    dir.flatten()
        .filter_map(|item| item.file_name().to_str().ok().map(String::from))
        .collect()
}
