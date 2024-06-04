use std::path::{Path, PathBuf};

use anyhow::Error;

use proxmox_sys::error::SysError;
use proxmox_sys::fs::CreateOptions;

struct AcmeApiConfig {
    acme_config_dir: PathBuf,
    acme_account_dir: PathBuf,
}

static mut ACME_ACME_CONFIG: Option<AcmeApiConfig> = None;

/// Initialize the global product configuration.
pub fn init<P: AsRef<Path>>(acme_config_dir: P, create_subdirs: bool) -> Result<(), Error> {
    let acme_config_dir = acme_config_dir.as_ref().to_owned();

    unsafe {
        ACME_ACME_CONFIG = Some(AcmeApiConfig {
            acme_account_dir: acme_config_dir.join("accounts"),
            acme_config_dir,
        });
    }

    if create_subdirs {
        create_secret_subdir(self::acme_config_dir())?;
        create_secret_subdir(acme_account_dir())?;
    }

    Ok(())
}

fn acme_api_config() -> &'static AcmeApiConfig {
    unsafe {
        ACME_ACME_CONFIG
            .as_ref()
            .expect("ProxmoxProductConfig is not initialized!")
    }
}

fn acme_config_dir() -> &'static Path {
    acme_api_config().acme_config_dir.as_path()
}

pub(crate) fn acme_account_dir() -> &'static Path {
    acme_api_config().acme_account_dir.as_path()
}

pub(crate) fn plugin_cfg_filename() -> PathBuf {
    acme_config_dir().join("plugins.cfg")
}

pub(crate) fn plugin_cfg_lockfile() -> PathBuf {
    acme_config_dir().join("plugins.lck")
}

fn create_secret_subdir<P: AsRef<Path>>(dir: P) -> nix::Result<()> {
    let root_only = CreateOptions::new()
        .owner(nix::unistd::ROOT)
        .group(nix::unistd::Gid::from_raw(0))
        .perm(nix::sys::stat::Mode::from_bits_truncate(0o700));

    match proxmox_sys::fs::create_dir(dir, root_only) {
        Ok(()) => Ok(()),
        Err(err) if err.already_exists() => Ok(()),
        Err(err) => Err(err),
    }
}
