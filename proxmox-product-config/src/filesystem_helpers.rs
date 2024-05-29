use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;

use anyhow::{bail, format_err, Context, Error};

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::unistd::{Gid, Uid};

use proxmox_sys::fs::CreateOptions;

use super::product_config;

/// Return [CreateOptions] for files owned by `api_user.uid/api_user.gid` with mode `0640`.
pub fn default_create_options() -> CreateOptions {
    let api_user = &product_config().api_user;
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(api_user.uid)
        .group(api_user.gid)
}

/// Return [CreateOptions] for files owned by `root:api-user.gid` with permission `0640`.
///
/// Only the superuser can write those files, but group `api-user.gid` can read them.
pub fn privileged_create_options() -> CreateOptions {
    let api_user = &product_config().api_user;
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(api_user.gid)
}

/// Return [CreateOptions] for files owned by `root:root` with permission `0600`.
///
/// Only the superuser can read and write those files.
pub fn secret_create_options() -> CreateOptions {
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(nix::unistd::Gid::from_raw(0))
}

/// Return [CreateOptions] for files owned by `root:root` with permission `0644`.
///
/// Everyone can read, but only the superuser can write those files. This is usually used
/// for system configuration files inside "/etc/" (i.e. "/etc/resolv.conf").
pub fn system_config_create_options() -> CreateOptions {
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0644);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(nix::unistd::ROOT)
        .group(nix::unistd::Gid::from_raw(0))
}

/// Return [CreateOptions] for lock files, owner `api_user.uid/api_user.gid` and mode `0660`.
pub fn lockfile_create_options() -> CreateOptions {
    let api_user = &product_config().api_user;
    proxmox_sys::fs::CreateOptions::new()
        .perm(nix::sys::stat::Mode::from_bits_truncate(0o660))
        .owner(api_user.uid)
        .group(api_user.gid)
}

/// Check file/directory permissions.
///
/// Make sure that the file or dir is owned by uid/gid and has the correct mode.
pub fn check_permissions<P: AsRef<Path>>(
    dir: P,
    uid: Uid,
    gid: Gid,
    mode: u32,
) -> Result<(), Error> {
    let uid = uid.as_raw();
    let gid = gid.as_raw();
    let dir = dir.as_ref();

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
pub fn mkdir_permissions<P: AsRef<Path>>(
    dir: P,
    uid: Uid,
    gid: Gid,
    mode: u32,
) -> Result<(), Error> {
    let nix_mode = Mode::from_bits(mode).expect("bad mode bits for nix crate");
    let dir = dir.as_ref();

    match nix::unistd::mkdir(dir, nix_mode) {
        Ok(()) => (),
        Err(nix::errno::Errno::EEXIST) => {
            check_permissions(dir, uid, gid, mode)
                .map_err(|err| format_err!("unexpected permissions directory {dir:?}: {err}"))?;
            return Ok(());
        }
        Err(err) => bail!("unable to create directory {dir:?} - {err}",),
    }

    let fd = nix::fcntl::open(dir, OFlag::O_DIRECTORY, Mode::empty())
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .map_err(|err| format_err!("unable to open created directory {dir:?} - {err}"))?;
    // umask defaults to 022 so make sure the mode is fully honowed:
    nix::sys::stat::fchmod(fd.as_raw_fd(), nix_mode)
        .map_err(|err| format_err!("unable to set mode for directory {dir:?} - {err}"))?;
    nix::unistd::fchown(fd.as_raw_fd(), Some(uid), Some(gid))
        .map_err(|err| format_err!("unable to set ownership directory {dir:?} - {err}"))?;

    Ok(())
}

/// Atomically write data to file owned by `root:api-user.gid` with permission `0640`
///
/// Only the superuser can write those files, but group 'api-user' can read them.
pub fn replace_privileged_config<P: AsRef<std::path::Path>>(
    path: P,
    data: &[u8],
) -> Result<(), Error> {
    let options = privileged_create_options();
    proxmox_sys::fs::replace_file(path, data, options, true)?;
    Ok(())
}

/// Atomically write data to file owned by `api-user.uid:api-user.gid` with permission `0660`.
pub fn replace_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let options = default_create_options();
    proxmox_sys::fs::replace_file(path, data, options, true)?;
    Ok(())
}

/// Atomically write data to file owned by `root:root` with permission `0600`.
///
/// Only the superuser can read and write those files.
pub fn replace_secret_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let options = secret_create_options();
    proxmox_sys::fs::replace_file(path, data, options, true)?;
    Ok(())
}

/// Atomically write data to file owned by `root:root` with permission `0644`.
///
/// Everyone can read, but only the superuser can write those files. This is usually used
/// for system configuration files inside "/etc/" (i.e. "/etc/resolv.conf").
pub fn replace_system_config<P: AsRef<std::path::Path>>(path: P, data: &[u8]) -> Result<(), Error> {
    let options = system_config_create_options();
    proxmox_sys::fs::replace_file(path, data, options, true)?;
    Ok(())
}

/// Lock guard used by [open_api_lockfile]
///
/// The lock is released if you drop this guard.
#[allow(dead_code)]
pub struct ApiLockGuard(Option<std::fs::File>);

#[doc(hidden)]
/// Note: do not use for production code, this is only intended for tests
pub unsafe fn create_mocked_lock() -> ApiLockGuard {
    ApiLockGuard(None)
}

/// Open or create a lock file owned by user `api-user` and lock it.
///
/// Owner/Group of the file is set to `api-user.uid/api-user.gid`.
/// File mode is `0660`.
/// Default timeout is 10 seconds.
///
/// The lock is released as soon as you drop the returned lock guard.
///
/// Note: This method needs to be called by user `root` or `api-user`.
pub fn open_api_lockfile<P: AsRef<std::path::Path>>(
    path: P,
    timeout: Option<std::time::Duration>,
    exclusive: bool,
) -> Result<ApiLockGuard, Error> {
    let options = lockfile_create_options();
    let timeout = timeout.unwrap_or(std::time::Duration::new(10, 0));
    let file = proxmox_sys::fs::open_file_locked(&path, timeout, exclusive, options)?;
    Ok(ApiLockGuard(Some(file)))
}
