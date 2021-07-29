use std::convert::{TryFrom, TryInto};
use std::fmt::Display;
use std::io::{BufRead, BufReader};

use anyhow::{bail, format_err, Error};

/// The code names of Debian releases. Does not include `sid`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebianCodename {
    Lenny = 5,
    Squeeze,
    Wheezy,
    Jessie,
    Stretch,
    Buster,
    Bullseye,
    Bookworm,
    Trixie,
}

impl DebianCodename {
    pub fn next(&self) -> Option<Self> {
        match (*self as u8 + 1).try_into() {
            Ok(codename) => Some(codename),
            Err(_) => None,
        }
    }
}

impl TryFrom<&str> for DebianCodename {
    type Error = Error;

    fn try_from(string: &str) -> Result<Self, Error> {
        match string {
            "lenny" => Ok(DebianCodename::Lenny),
            "squeeze" => Ok(DebianCodename::Squeeze),
            "wheezy" => Ok(DebianCodename::Wheezy),
            "jessie" => Ok(DebianCodename::Jessie),
            "stretch" => Ok(DebianCodename::Stretch),
            "buster" => Ok(DebianCodename::Buster),
            "bullseye" => Ok(DebianCodename::Bullseye),
            "bookworm" => Ok(DebianCodename::Bookworm),
            "trixie" => Ok(DebianCodename::Trixie),
            _ => bail!("unknown Debian code name '{}'", string),
        }
    }
}

impl TryFrom<u8> for DebianCodename {
    type Error = Error;

    fn try_from(number: u8) -> Result<Self, Error> {
        match number {
            5 => Ok(DebianCodename::Lenny),
            6 => Ok(DebianCodename::Squeeze),
            7 => Ok(DebianCodename::Wheezy),
            8 => Ok(DebianCodename::Jessie),
            9 => Ok(DebianCodename::Stretch),
            10 => Ok(DebianCodename::Buster),
            11 => Ok(DebianCodename::Bullseye),
            12 => Ok(DebianCodename::Bookworm),
            13 => Ok(DebianCodename::Trixie),
            _ => bail!("unknown Debian release number '{}'", number),
        }
    }
}

impl Display for DebianCodename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebianCodename::Lenny => write!(f, "lenny"),
            DebianCodename::Squeeze => write!(f, "squeeze"),
            DebianCodename::Wheezy => write!(f, "wheezy"),
            DebianCodename::Jessie => write!(f, "jessie"),
            DebianCodename::Stretch => write!(f, "stretch"),
            DebianCodename::Buster => write!(f, "buster"),
            DebianCodename::Bullseye => write!(f, "bullseye"),
            DebianCodename::Bookworm => write!(f, "bookworm"),
            DebianCodename::Trixie => write!(f, "trixie"),
        }
    }
}

/// Read the `VERSION_CODENAME` from `/etc/os-release`.
pub fn get_current_release_codename() -> Result<DebianCodename, Error> {
    let raw = std::fs::read("/etc/os-release")
        .map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

    let reader = BufReader::new(&*raw);

    for line in reader.lines() {
        let line = line.map_err(|err| format_err!("unable to read '/etc/os-release' - {}", err))?;

        if let Some(codename) = line.strip_prefix("VERSION_CODENAME=") {
            let codename = codename.trim_matches(&['"', '\''][..]);
            return codename.try_into();
        }
    }

    bail!("unable to parse codename from '/etc/os-release'");
}
