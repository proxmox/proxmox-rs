//! ACME API Configuration.

use std::borrow::Cow;
use std::path::Path;

use proxmox_sys::error::SysError;
use proxmox_sys::fs::CreateOptions;

use crate::types::KnownAcmeDirectory;
use crate::acme_config_dir;

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


pub(crate) fn create_secret_subdir<P: AsRef<Path>>(dir: P) -> nix::Result<()> {
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

pub(crate) fn make_acme_dir() -> nix::Result<()> {
    create_secret_subdir(acme_config_dir())
}
