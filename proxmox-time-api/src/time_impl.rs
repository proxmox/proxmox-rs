use anyhow::{bail, format_err, Error};

use proxmox_product_config::replace_system_config;
use proxmox_sys::fs::file_read_firstline;

use super::ServerTimeInfo;

pub fn read_etc_localtime() -> Result<String, Error> {
    // use /etc/timezone
    if let Ok(line) = file_read_firstline("/etc/timezone") {
        return Ok(line.trim().to_owned());
    }

    // otherwise guess from the /etc/localtime symlink
    let link = std::fs::read_link("/etc/localtime")
        .map_err(|err| format_err!("failed to guess timezone - {}", err))?;

    let link = link.to_string_lossy();
    match link.rfind("/zoneinfo/") {
        Some(pos) => Ok(link[(pos + 10)..].to_string()),
        None => Ok(link.to_string()),
    }
}

pub fn set_timezone(timezone: String) -> Result<(), Error> {
    let path = std::path::PathBuf::from(format!("/usr/share/zoneinfo/{}", timezone));

    if !path.exists() {
        bail!("No such timezone.");
    }

    replace_system_config("/etc/timezone", timezone.as_bytes())?;

    let _ = std::fs::remove_file("/etc/localtime");

    use std::os::unix::fs::symlink;
    symlink(path, "/etc/localtime")?;

    Ok(())
}

/// Read server time and time zone settings.
pub fn get_server_time_info() -> Result<ServerTimeInfo, Error> {
    let time = proxmox_time::epoch_i64();
    let tm = proxmox_time::localtime(time)?;
    let offset = tm.tm_gmtoff;

    let localtime = time + offset;

    Ok(ServerTimeInfo {
        timezone: read_etc_localtime()?,
        time,
        localtime,
    })
}
