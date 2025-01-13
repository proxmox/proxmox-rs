//! Verifier functions for the schema.

use anyhow::{bail, format_err, Error};

use proxmox_schema::const_regex;

pub fn verify_pve_volume_id_or_qm_path(s: &str) -> Result<(), Error> {
    if s == "none" || s == "cdrom" || s.starts_with('/') {
        return Ok(());
    }

    verify_volume_id(s)
}

#[rustfmt::skip]
macro_rules! DNS_NAMERE { () => (r##"([a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?)"##) }
#[rustfmt::skip]
macro_rules! DNS_RE { () => (concat!("(", DNS_NAMERE!(), "\\.)*", DNS_NAMERE!(), "$")) }

const_regex! {

pub VOLUME_ID = r##"^(?i)([a-z][a-z0-9\-\_\.]*[a-z0-9]):(.+)$"##;
//pub DNS_NAMERE = concat!("^", DNS_NAMERE!(), "$");
pub DNS_RE = concat!("^", DNS_RE!(), "$");

pub FE80_RE = r##"^(?i)fe80:"##;

pub IFACE_RE = r##"^(?i)[a-z][a-z0-9_]{1,20}([:\.]\d+)?$"##;

pub VLAN_ID_OR_RANGE = r##"^(\d+)(?:-(\d+))?$"##;

}

pub fn verify_volume_id(s: &str) -> Result<(), Error> {
    if VOLUME_ID.is_match(s) {
        Ok(())
    } else {
        bail!("not a valid volume id");
    }
}

pub fn verify_pve_phys_bits(s: &str) -> Result<(), Error> {
    s.parse::<u32>()
        .ok()
        .and_then(|n| (8..=64).contains(&n).then_some(()))
        .ok_or_else(|| format_err!("invalid number of bits"))
}

pub fn verify_ipv4(s: &str) -> Result<(), Error> {
    if proxmox_schema::api_types::IP_V4_REGEX.is_match(s) {
        Ok(())
    } else {
        bail!("not a valid IPv4 address");
    }
}

pub fn verify_ipv6(s: &str) -> Result<(), Error> {
    if proxmox_schema::api_types::IP_V6_REGEX.is_match(s) {
        Ok(())
    } else {
        bail!("not a valid IPv6 address");
    }
}

pub fn verify_ip(s: &str) -> Result<(), Error> {
    if proxmox_schema::api_types::IP_REGEX.is_match(s) {
        Ok(())
    } else {
        bail!("not a valid IP address");
    }
}

pub fn verify_cidr(s: &str) -> Result<(), Error> {
    match s.find('/') {
        None => bail!("not a CIDR notation"),
        Some(pos) => {
            let ip = &s[..pos];
            let prefix = &s[(pos + 1)..];

            let maxbits = if verify_ipv4(ip).is_ok() {
                32
            } else if verify_ipv6(ip).is_ok() {
                128
            } else {
                bail!("not a valid IP address in CIDR");
            };

            match prefix.parse::<u8>() {
                Err(_) => bail!("not a valid CIDR notation"),
                Ok(n) if n > maxbits => bail!("invalid prefix length in CIDR"),
                Ok(_) => Ok(()),
            }
        }
    }
}

pub fn verify_cidrv4(s: &str) -> Result<(), Error> {
    match s.find('/') {
        None => bail!("not a CIDR notation"),
        Some(pos) => {
            verify_ipv4(&s[..pos])?;
            match s[(pos + 1)..].parse::<u8>() {
                Ok(n) if n > 32 => bail!("invalid prefix length in CIDR"),
                Err(_) => bail!("not a valid CIDR notation"),
                Ok(_) => Ok(()),
            }
        }
    }
}

pub fn verify_cidrv6(s: &str) -> Result<(), Error> {
    match s.find('/') {
        None => bail!("not a CIDR notation"),
        Some(pos) => {
            verify_ipv6(&s[..pos])?;
            match s[(pos + 1)..].parse::<u8>() {
                Ok(n) if n > 128 => bail!("invalid prefix length in CIDR"),
                Err(_) => bail!("not a valid CIDR notation"),
                Ok(_) => Ok(()),
            }
        }
    }
}

pub fn verify_ipv4_config(s: &str) -> Result<(), Error> {
    if s == "dhcp" || s == "manual" {
        return Ok(());
    }
    verify_cidrv4(s)
}

pub fn verify_ipv6_config(s: &str) -> Result<(), Error> {
    if s == "dhcp" || s == "manual" || s == "auto" {
        return Ok(());
    }
    verify_cidrv6(s)
}

pub fn verify_ipv4_mask(s: &str) -> Result<(), Error> {
    match s {
        "0.0.0.0" | "128.0.0.0" | "192.0.0.0" | "224.0.0.0" | "240.0.0.0" | "248.0.0.0"
        | "252.0.0.0" | "254.0.0.0" | "255.0.0.0" | "255.128.0.0" | "255.192.0.0"
        | "255.224.0.0" | "255.240.0.0" | "255.248.0.0" | "255.252.0.0" | "255.254.0.0"
        | "255.255.0.0" | "255.255.128.0" | "255.255.192.0" | "255.255.224.0" | "255.255.240.0"
        | "255.255.248.0" | "255.255.252.0" | "255.255.254.0" | "255.255.255.0"
        | "255.255.255.128" | "255.255.255.192" | "255.255.255.224" | "255.255.255.240"
        | "255.255.255.248" | "255.255.255.252" | "255.255.255.254" | "255.255.255.255" => Ok(()),
        _ => bail!("not a valid ipv4 netmask"),
    }
}

pub fn verify_dns_name(s: &str) -> Result<(), Error> {
    if DNS_RE.is_match(s) {
        Ok(())
    } else {
        bail!("not a valid dns name")
    }
}

pub fn verify_address(s: &str) -> Result<(), Error> {
    if DNS_RE.is_match(s) {
        return Ok(());
    }
    verify_ip(s).map_err(|_| format_err!("not a valid address"))
}

pub fn verify_lxc_mp_string(s: &str) -> Result<(), Error> {
    if s.contains("/./") || s.contains("/../") || s.ends_with("/..") || s.starts_with("../") {
        bail!("illegal character sequence for mount point");
    }
    Ok(())
}

pub fn verify_ip_with_ll_iface(s: &str) -> Result<(), Error> {
    if let Some(percent) = s.find('%') {
        if FE80_RE.is_match(s) && IFACE_RE.is_match(&s[(percent + 1)..]) {
            return verify_ipv6(&s[..percent]);
        }
    }
    verify_ip(s)
}

pub fn verify_storage_pair(_s: &str) -> Result<(), Error> {
    // FIXME: Implement this!
    Ok(())
}

pub fn verify_bridge_pair(_s: &str) -> Result<(), Error> {
    // FIXME: Implement this!
    Ok(())
}

pub fn verify_pve_lxc_dev_string(s: &str) -> Result<(), Error> {
    if !s.starts_with("/dev") || s.ends_with("/..") || s.contains("/..") {
        bail!("not a valid device string");
    }
    Ok(())
}

pub fn verify_vlan_id_or_range(s: &str) -> Result<(), Error> {
    let check_vid = |vid: u16| -> Result<(), Error> {
        if vid > 4094 || vid < 2 {
            bail!("invalid VLAN tag '{vid}'");
        } else {
            Ok(())
        }
    };

    let captures = VLAN_ID_OR_RANGE
        .captures(s)
        .ok_or_else(|| format_err!("invalid VLAN configuration '{s}"))?;

    match (captures.get(1), captures.get(2)) {
        (Some(start), Some(end)) => {
            let start = start.as_str().parse()?;
            let end = end.as_str().parse()?;
            check_vid(start)?;
            check_vid(end)?;
            if start >= end {
                bail!("VLAN range must go from lower to higher tag: '{s}");
            }
        }
        (Some(vid), None) => {
            check_vid(vid.as_str().parse()?)?;
        }
        (None, _) => bail!("invalid VLAN configuration '{s}"),
    }

    Ok(())
}
