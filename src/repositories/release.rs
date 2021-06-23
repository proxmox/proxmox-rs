use anyhow::{bail, format_err, Error};

use std::io::{BufRead, BufReader};

/// The suites of Debian releases, ordered chronologically, with variable releases
/// like 'oldstable' and 'testing' ordered at the extremes. Does not include 'stable'.
pub const DEBIAN_SUITES: [&str; 15] = [
    "oldoldstable",
    "oldstable",
    "lenny",
    "squeeze",
    "wheezy",
    "jessie",
    "stretch",
    "buster",
    "bullseye",
    "bookworm",
    "trixie",
    "sid",
    "testing",
    "unstable",
    "experimental",
];

/// Read the `VERSION_CODENAME` from `/etc/os-release`.
pub fn get_current_release_codename() -> Result<String, Error> {
    let raw = std::fs::read("/etc/os-release")
        .map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

    let reader = BufReader::new(&*raw);

    for line in reader.lines() {
        let line = line.map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

        if let Some(codename) = line.strip_prefix("VERSION_CODENAME=") {
            let codename = codename.trim_matches(&['"', '\''][..]);
            return Ok(codename.to_string());
        }
    }

    bail!("unable to parse codename from '/etc/os-release'");
}
