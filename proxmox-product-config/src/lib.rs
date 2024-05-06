use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;

use anyhow::{bail, format_err, Context, Error};

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::unistd::{Gid, Uid};

mod digest;
pub use digest::{ConfigDigest, PROXMOX_CONFIG_DIGEST_FORMAT, PROXMOX_CONFIG_DIGEST_SCHEMA};

static mut PRODUCT_CONFIG: Option<ProxmoxProductConfig> = None;

/// Initialize the global product configuration.
pub fn init_product_config(config_dir: &'static str, api_user: nix::unistd::User) {
    unsafe {
        PRODUCT_CONFIG = Some(ProxmoxProductConfig {
            config_dir,
            api_user,
        });
    }
}

/// Returns the global product configuration (see [init_product_config])
pub fn product_config() -> &'static ProxmoxProductConfig {
    unsafe {
        PRODUCT_CONFIG
            .as_ref()
            .expect("ProxmoxProductConfig is not initialized!")
    }
}

pub struct ProxmoxProductConfig {
    /// Path to the main product configuration directory.
    config_dir: &'static str,

    /// Configuration file owner.
    api_user: nix::unistd::User,
}

impl ProxmoxProductConfig {
    /// Returns the absolute path (prefix with 'config_dir')
    pub fn absolute_path(&self, rel_path: &str) -> PathBuf {
        let mut path = PathBuf::from(self.config_dir);
        path.push(rel_path);
        path
    }
}

// Check file/directory permissions
//
// For security reasons, we want to make sure they are set correctly:
// * owned by uid/gid
// * nobody else can read (mode 0700)
pub fn check_permissions(dir: &str, uid: Uid, gid: Gid, mode: u32) -> Result<(), Error> {
    let uid = uid.as_raw();
    let gid = gid.as_raw();

    let nix::sys::stat::FileStat {
        st_uid,
        st_gid,
        st_mode,
        ..
    } = nix::sys::stat::stat(dir).with_context(|| format!("failed to stat {dir:?}"))?;

    if st_uid != uid {
        log::error!("bad owner on {dir:?} ({st_uid} != {uid})");
    }
    if st_gid != gid {
        log::error!("bad group on {dir:?} ({st_gid} != {gid})");
    }
    let perms = st_mode & !nix::sys::stat::SFlag::S_IFMT.bits();
    if perms != mode {
        log::error!("bad permissions on {dir:?} (0o{perms:o} != 0o{mode:o})");
    }

    Ok(())
}

/// Create a new directory with uid/gid and mode.
///
/// Returns Ok if the directory already exists with correct access permissions.
pub fn mkdir_permissions(dir: &str, uid: Uid, gid: Gid, mode: u32) -> Result<(), Error> {
    let nix_mode = Mode::from_bits(mode).expect("bad mode bits for nix crate");
    match nix::unistd::mkdir(dir, nix_mode) {
        Ok(()) => (),
        Err(nix::errno::Errno::EEXIST) => {
            check_permissions(dir, uid, gid, mode)
                .map_err(|err| format_err!("unexpected permissions directory '{dir}': {err}"))?;
            return Ok(());
        }
        Err(err) => bail!("unable to create directory '{dir}' - {err}",),
    }

    let fd = nix::fcntl::open(dir, OFlag::O_DIRECTORY, Mode::empty())
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .map_err(|err| format_err!("unable to open created directory '{dir}' - {err}"))?;
    // umask defaults to 022 so make sure the mode is fully honowed:
    nix::sys::stat::fchmod(fd.as_raw_fd(), nix_mode)
        .map_err(|err| format_err!("unable to set mode for directory '{dir}' - {err}"))?;
    nix::unistd::fchown(fd.as_raw_fd(), Some(uid), Some(gid))
        .map_err(|err| format_err!("unable to set ownership directory '{dir}' - {err}"))?;

    Ok(())
}

/// Atomically write data to file owned by `root:api-user` with permission `0640`
///
/// Only the superuser can write those files, but group 'api-user' can read them.
pub fn replace_privileged_config<P: AsRef<std::path::Path>>(
    path: P,
    data: &[u8],
) -> Result<(), Error> {
    let api_user = &product_config().api_user;
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    // set the correct owner/group/permissions while saving file
    // owner(rw) = root, group(r)= api-user
    let options = proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(api_user.gid);

    proxmox_sys::fs::replace_file(path, data, options, true)?;

    Ok(())
}

/// Atomically write data to file owned by `api-user:api-user` with permission `0660`.
pub fn replace_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let api_user = &product_config().api_user;
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    // set the correct owner/group/permissions while saving file
    // owner(rw) = root, group(r)= api-user
    let options = proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(api_user.uid)
        .group(api_user.gid);

    proxmox_sys::fs::replace_file(path, data, options, true)?;

    Ok(())
}

/// Atomically write data to file owned by "root:root" with permission "0600"
///
/// Only the superuser can read and write those files.
pub fn replace_secret_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
    // set the correct owner/group/permissions while saving file
    // owner(rw) = root, group(r)= root
    let options = proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(nix::unistd::Gid::from_raw(0));

    proxmox_sys::fs::replace_file(path, data, options, true)?;

    Ok(())
}

/// Atomically write data to file owned by "root:root" with permission "0644"
///
/// Everyone can read, but only the superuser can write those files. This is usually used
/// for system configuration files inside "/etc/" (i.e. "/etc/resolv.conf").
pub fn replace_system_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0644);
    // set the correct owner/group/permissions while saving file
    // owner(rw) = root, group(r)= root
    let options = proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(nix::unistd::Gid::from_raw(0));

    proxmox_sys::fs::replace_file(path, data, options, true)?;

    Ok(())
}

#[allow(dead_code)]
pub struct ApiLockGuard(Option<std::fs::File>);

#[doc(hidden)]
/// Note: do not use for production code, this is only intended for tests
pub unsafe fn create_mocked_lock() -> ApiLockGuard {
    ApiLockGuard(None)
}

/// Open or create a lock file owned by user "api-user" and lock it.
///
/// Owner/Group of the file is set to api-user/api-group.
/// File mode is 0660.
/// Default timeout is 10 seconds.
///
/// Note: This method needs to be called by user "root" or "api-user".
pub fn open_api_lockfile<P: AsRef<std::path::Path>>(
    path: P,
    timeout: Option<std::time::Duration>,
    exclusive: bool,
) -> Result<ApiLockGuard, Error> {
    let api_user = &product_config().api_user;
    let options = proxmox_sys::fs::CreateOptions::new()
        .perm(nix::sys::stat::Mode::from_bits_truncate(0o660))
        .owner(api_user.uid)
        .group(api_user.gid);

    let timeout = timeout.unwrap_or(std::time::Duration::new(10, 0));

    let file = proxmox_sys::fs::open_file_locked(&path, timeout, exclusive, options)?;
    Ok(ApiLockGuard(Some(file)))
}
