use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Error;
use const_format::concatcp;
use lazy_static::lazy_static;
use proxmox_product_config::ConfigDigest;
use regex::Regex;

use proxmox_sys::fs::file_get_contents;
use proxmox_sys::fs::replace_file;
use proxmox_sys::fs::CreateOptions;

use proxmox_schema::api_types::IPRE_STR;

mod api_types;
pub use api_types::{DeletableResolvConfProperty, ResolvConf, ResolvConfWithDigest};

static RESOLV_CONF_FN: &str = "/etc/resolv.conf";

/// Read DNS configuration from '/etc/resolv.conf'.
pub fn read_etc_resolv_conf(
    expected_digest: Option<&[u8; 32]>,
) -> Result<ResolvConfWithDigest, Error> {
    let mut config = ResolvConf::default();

    let mut nscount = 0;

    let raw = file_get_contents(RESOLV_CONF_FN)?;
    let digest = ConfigDigest::from_slice(&raw);

    proxmox_product_config::detect_modified_configuration_file(expected_digest, &digest)?;

    let data = String::from_utf8(raw)?;

    lazy_static! {
        static ref DOMAIN_REGEX: Regex = Regex::new(r"^\s*(?:search|domain)\s+(\S+)\s*").unwrap();
        static ref SERVER_REGEX: Regex =
            Regex::new(concatcp!(r"^\s*nameserver\s+(", IPRE_STR, r")\s*")).unwrap();
    }

    let mut options = String::new();

    for line in data.lines() {
        if let Some(caps) = DOMAIN_REGEX.captures(line) {
            config.search = Some(caps[1].to_owned());
        } else if let Some(caps) = SERVER_REGEX.captures(line) {
            nscount += 1;
            if nscount > 3 {
                continue;
            };
            let nameserver = Some(caps[1].to_owned());
            match nscount {
                1 => config.dns1 = nameserver,
                2 => config.dns2 = nameserver,
                3 => config.dns3 = nameserver,
                _ => continue,
            }
        } else {
            if !options.is_empty() {
                options.push('\n');
            }
            options.push_str(line);
        }
    }

    if !options.is_empty() {
        config.options = Some(options);
    }

    Ok(ResolvConfWithDigest { config, digest })
}

/// Update DNS configuration, write result back to '/etc/resolv.conf'.
pub fn update_dns(
    update: ResolvConf,
    delete: Option<Vec<DeletableResolvConfProperty>>,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    lazy_static! {
        static ref MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    }

    let _guard = MUTEX.lock();

    let ResolvConfWithDigest { mut config, .. } = read_etc_resolv_conf(digest.as_deref())?;

    if let Some(delete) = delete {
        for delete_prop in delete {
            match delete_prop {
                DeletableResolvConfProperty::Dns1 => {
                    config.dns1 = None;
                }
                DeletableResolvConfProperty::Dns2 => {
                    config.dns2 = None;
                }
                DeletableResolvConfProperty::Dns3 => {
                    config.dns3 = None;
                }
            }
        }
    }

    if update.search.is_some() {
        config.search = update.search;
    }
    if update.dns1.is_some() {
        config.dns1 = update.dns1;
    }
    if update.dns2.is_some() {
        config.dns2 = update.dns2;
    }
    if update.dns3.is_some() {
        config.dns3 = update.dns3;
    }

    let mut data = String::new();

    use std::fmt::Write as _;
    if let Some(search) = config.search {
        let _ = writeln!(data, "search {}", search);
    }

    if let Some(dns1) = config.dns1 {
        let _ = writeln!(data, "nameserver {}", dns1);
    }

    if let Some(dns2) = config.dns2 {
        let _ = writeln!(data, "nameserver {}", dns2);
    }

    if let Some(dns3) = config.dns3 {
        let _ = writeln!(data, "nameserver {}", dns3);
    }

    if let Some(options) = config.options {
        data.push_str(&options);
    }

    replace_file(RESOLV_CONF_FN, data.as_bytes(), CreateOptions::new(), true)?;

    Ok(())
}
