use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{bail, format_err, Context, Error};
use const_format::concatcp;
use regex::Regex;

use proxmox_schema::api_types::IPV4RE_STR;
use proxmox_schema::api_types::IPV6RE_STR;
use proxmox_ve_config::guest::vm::MacAddress;

pub static IPV4_REVERSE_MASK: &[&str] = &[
    "0.0.0.0",
    "128.0.0.0",
    "192.0.0.0",
    "224.0.0.0",
    "240.0.0.0",
    "248.0.0.0",
    "252.0.0.0",
    "254.0.0.0",
    "255.0.0.0",
    "255.128.0.0",
    "255.192.0.0",
    "255.224.0.0",
    "255.240.0.0",
    "255.248.0.0",
    "255.252.0.0",
    "255.254.0.0",
    "255.255.0.0",
    "255.255.128.0",
    "255.255.192.0",
    "255.255.224.0",
    "255.255.240.0",
    "255.255.248.0",
    "255.255.252.0",
    "255.255.254.0",
    "255.255.255.0",
    "255.255.255.128",
    "255.255.255.192",
    "255.255.255.224",
    "255.255.255.240",
    "255.255.255.248",
    "255.255.255.252",
    "255.255.255.254",
    "255.255.255.255",
];

pub static IPV4_MASK_HASH_LOCALNET: LazyLock<HashMap<&'static str, u8>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    #[allow(clippy::needless_range_loop)]
    for i in 0..IPV4_REVERSE_MASK.len() {
        map.insert(IPV4_REVERSE_MASK[i], i as u8);
    }
    map
});

pub fn parse_cidr(cidr: &str) -> Result<(String, u8, bool), Error> {
    let (address, mask, is_v6) = parse_address_or_cidr(cidr)?;
    if let Some(mask) = mask {
        Ok((address, mask, is_v6))
    } else {
        bail!("missing netmask in '{}'", cidr);
    }
}

pub(crate) fn check_netmask(mask: u8, is_v6: bool) -> Result<(), Error> {
    let (ver, min, max) = if is_v6 {
        ("IPv6", 1, 128)
    } else {
        ("IPv4", 1, 32)
    };

    if !(mask >= min && mask <= max) {
        bail!(
            "{} mask '{}' is out of range ({}..{}).",
            ver,
            mask,
            min,
            max
        );
    }

    Ok(())
}

// parse ip address with optional cidr mask
pub(crate) fn parse_address_or_cidr(cidr: &str) -> Result<(String, Option<u8>, bool), Error> {
    // NOTE: This is NOT the same regex as in proxmox-schema as this one has capture groups for
    // the addresses vs cidr portions!
    pub static CIDR_V4_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(concatcp!(r"^(", IPV4RE_STR, r")(?:/(\d{1,2}))?$")).unwrap());
    pub static CIDR_V6_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(concatcp!(r"^(", IPV6RE_STR, r")(?:/(\d{1,3}))?$")).unwrap());

    if let Some(caps) = CIDR_V4_REGEX.captures(cidr) {
        let address = &caps[1];
        if let Some(mask) = caps.get(2) {
            let mask: u8 = mask.as_str().parse()?;
            check_netmask(mask, false)?;
            Ok((address.to_string(), Some(mask), false))
        } else {
            Ok((address.to_string(), None, false))
        }
    } else if let Some(caps) = CIDR_V6_REGEX.captures(cidr) {
        let address = &caps[1];
        if let Some(mask) = caps.get(2) {
            let mask: u8 = mask.as_str().parse()?;
            check_netmask(mask, true)?;
            Ok((address.to_string(), Some(mask), true))
        } else {
            Ok((address.to_string(), None, true))
        }
    } else {
        bail!("invalid address/mask '{}'", cidr);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Deserialize)]
pub struct SlaveData {
    perm_hw_addr: Option<MacAddress>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Deserialize)]
pub struct LinkInfo {
    info_slave_data: Option<SlaveData>,
    info_kind: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Deserialize)]
pub struct IpLink {
    ifname: String,
    #[serde(default)]
    altnames: Vec<String>,
    ifindex: i64,
    link_type: String,
    address: MacAddress,
    linkinfo: Option<LinkInfo>,
    operstate: String,
}

impl IpLink {
    pub fn index(&self) -> i64 {
        self.ifindex
    }

    pub fn is_physical(&self) -> bool {
        self.link_type == "ether"
            && (self.linkinfo.is_none() || self.linkinfo.as_ref().unwrap().info_kind.is_none())
    }

    pub fn name(&self) -> &str {
        &self.ifname
    }

    pub fn permanent_mac(&self) -> MacAddress {
        if let Some(link_info) = &self.linkinfo {
            if let Some(info_slave_data) = &link_info.info_slave_data {
                if let Some(perm_hw_addr) = info_slave_data.perm_hw_addr {
                    return perm_hw_addr;
                }
            }
        }

        self.address
    }

    pub fn altnames(&self) -> impl Iterator<Item = &String> {
        self.altnames.iter()
    }

    pub fn active(&self) -> bool {
        self.operstate == "UP"
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AltnameMapping {
    mapping: HashMap<String, String>,
}

impl std::ops::Deref for AltnameMapping {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.mapping
    }
}

impl FromIterator<IpLink> for AltnameMapping {
    fn from_iter<T: IntoIterator<Item = IpLink>>(iter: T) -> Self {
        let mut mapping = HashMap::new();

        for iface in iter.into_iter() {
            for altname in iface.altnames {
                mapping.insert(altname, iface.ifname.clone());
            }
        }

        Self { mapping }
    }
}

pub fn get_network_interfaces() -> Result<HashMap<String, IpLink>, Error> {
    let output = std::process::Command::new("ip")
        .arg("-details")
        .arg("-json")
        .arg("link")
        .arg("show")
        .stdout(std::process::Stdio::piped())
        .output()
        .with_context(|| "could not obtain ip link output")?;

    if !output.status.success() {
        bail!("ip link returned non-zero exit code")
    }

    Ok(serde_json::from_slice::<Vec<IpLink>>(&output.stdout)
        .with_context(|| "could not deserialize ip link output")?
        .into_iter()
        .map(|ip_link| (ip_link.ifname.clone(), ip_link))
        .collect())
}

pub(crate) fn compute_file_diff(filename: &str, shadow: &str) -> Result<String, Error> {
    let output = Command::new("diff")
        .arg("-b")
        .arg("-u")
        .arg(filename)
        .arg(shadow)
        .output()
        .map_err(|err| format_err!("failed to execute diff - {}", err))?;

    let diff = proxmox_sys::command::command_output_as_string(output, Some(|c| c == 0 || c == 1))
        .map_err(|err| format_err!("diff failed: {}", err))?;

    Ok(diff)
}

pub fn assert_ifupdown2_installed() -> Result<(), Error> {
    if !Path::new("/usr/share/ifupdown2").exists() {
        bail!("ifupdown2 is not installed.");
    }

    Ok(())
}

pub fn network_reload() -> Result<(), Error> {
    let output = Command::new("ifreload")
        .arg("-a")
        .output()
        .map_err(|err| format_err!("failed to execute 'ifreload' - {}", err))?;

    proxmox_sys::command::command_output(output, None)
        .map_err(|err| format_err!("ifreload failed: {}", err))?;

    Ok(())
}
