use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Error;

use proxmox_product_config::create_secret_dir;

#[derive(Debug)]
struct AcmeApiConfig {
    acme_config_dir: PathBuf,
    acme_account_dir: PathBuf,
}

static ACME_ACME_CONFIG: OnceLock<AcmeApiConfig> = OnceLock::new();

/// Initialize the global product configuration.
pub fn init<P: AsRef<Path>>(acme_config_dir: P, create_subdirs: bool) -> Result<(), Error> {
    let acme_config_dir = acme_config_dir.as_ref().to_owned();

    ACME_ACME_CONFIG
        .set(AcmeApiConfig {
            acme_account_dir: acme_config_dir.join("accounts"),
            acme_config_dir,
        })
        .expect("cannot set acme configuration twice");

    if create_subdirs {
        create_secret_dir(self::acme_config_dir(), false)?;
        create_secret_dir(acme_account_dir(), false)?;
    }

    Ok(())
}

fn acme_api_config() -> &'static AcmeApiConfig {
    ACME_ACME_CONFIG
        .get()
        .expect("AcmeApiConfig is not initialized!")
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
