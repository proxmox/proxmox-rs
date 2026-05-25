use std::io::{BufRead, BufReader};

use anyhow::{Error, bail, format_err};

pub use proxmox_apt_api_types::DebianCodename;

/// Read the `VERSION_CODENAME` from `/etc/os-release`.
pub fn get_current_release_codename() -> Result<DebianCodename, Error> {
    let raw = std::fs::read("/etc/os-release")
        .map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

    let reader = BufReader::new(&*raw);

    for line in reader.lines() {
        let line = line.map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

        if let Some(codename) = line.strip_prefix("VERSION_CODENAME=") {
            let codename = codename.trim_matches(&['"', '\''][..]);
            return Ok(codename.try_into()?);
        }
    }

    bail!("unable to parse codename from '/etc/os-release'");
}
