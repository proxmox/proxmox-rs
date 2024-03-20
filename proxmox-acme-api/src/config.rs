//! ACME API Configuration.

use std::borrow::Cow;

use proxmox_sys::error::SysError;
use proxmox_sys::fs::CreateOptions;

use crate::types::KnownAcmeDirectory;

/// ACME API Configuration.
///
/// This struct provides access to the server side configuration, like the
/// configuration directory. All ACME API functions are implemented as member
/// fuction, so they all have access to this configuration.
///

pub struct AcmeApiConfig {
    /// Path to the ACME configuration  directory.
    pub config_dir: &'static str,
    /// Configuration file owner.
    pub file_owner: fn() -> nix::unistd::User,
}

/// List of known ACME directorties.
pub const KNOWN_ACME_DIRECTORIES: &[KnownAcmeDirectory] = &[
    KnownAcmeDirectory {
        name: Cow::Borrowed("Let's Encrypt V2"),
        url: Cow::Borrowed("https://acme-v02.api.letsencrypt.org/directory"),
    },
    KnownAcmeDirectory {
        name: Cow::Borrowed("Let's Encrypt V2 Staging"),
        url: Cow::Borrowed("https://acme-staging-v02.api.letsencrypt.org/directory"),
    },
];

/// Default ACME directorties.
pub const DEFAULT_ACME_DIRECTORY_ENTRY: &KnownAcmeDirectory = &KNOWN_ACME_DIRECTORIES[0];

// local helpers to read/write acme configuration
impl AcmeApiConfig {
    pub(crate) fn acme_config_dir(&self) -> &'static str {
        self.config_dir
    }

    pub(crate) fn plugin_cfg_filename(&self) -> String {
        format!("{}/plugins.cfg", self.acme_config_dir())
    }
    pub(crate) fn plugin_cfg_lockfile(&self) -> String {
        format!("{}/.plugins.lck", self.acme_config_dir())
    }

    pub(crate) fn create_acme_subdir(dir: &str) -> nix::Result<()> {
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

    pub(crate) fn make_acme_dir(&self) -> nix::Result<()> {
        Self::create_acme_subdir(&self.acme_config_dir())
    }
}
