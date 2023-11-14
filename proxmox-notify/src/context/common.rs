use std::path::Path;

pub(crate) fn attempt_file_read<P: AsRef<Path>>(path: P) -> Option<String> {
    match proxmox_sys::fs::file_read_optional_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            log::error!("{err}");
            None
        }
    }
}

pub(crate) fn lookup_datacenter_config_key(content: &str, key: &str) -> Option<String> {
    let key_prefix = format!("{key}:");
    normalize_for_return(
        content
            .lines()
            .find_map(|line| line.strip_prefix(&key_prefix)),
    )
}

pub(crate) fn normalize_for_return(s: Option<&str>) -> Option<String> {
    match s?.trim() {
        "" => None,
        s => Some(s.to_string()),
    }
}
