use anyhow::Error;

use proxmox_sys::fs::CreateOptions;

use super::{get_api_user, get_priv_user};

/// Return [CreateOptions] for files owned by `api_user.uid/api_user.gid` with mode `0640`.
pub fn default_create_options() -> CreateOptions {
    let api_user = get_api_user();
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(api_user.uid)
        .group(api_user.gid)
}

/// Return [CreateOptions] for files owned by `priv_user.uid:api-user.gid` with permission `0640`.
///
/// Only the superuser can write those files, but group `api-user.gid` can read them.
pub fn privileged_create_options() -> CreateOptions {
    let api_user = get_api_user();
    let priv_user = get_priv_user();
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0640);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(priv_user.uid)
        .group(api_user.gid)
}

/// Return [CreateOptions] for files owned by `priv_user.uid: priv_user.gid` with permission `0600`.
///
/// Only the superuser can read and write those files.
pub fn secret_create_options() -> CreateOptions {
    let priv_user = get_priv_user();
    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
    proxmox_sys::fs::CreateOptions::new()
        .perm(mode)
        .owner(priv_user.uid)
        .group(priv_user.gid)
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
    let api_user = get_api_user();
    proxmox_sys::fs::CreateOptions::new()
        .perm(nix::sys::stat::Mode::from_bits_truncate(0o660))
        .owner(api_user.uid)
        .group(api_user.gid)
}

/// Atomically write data to file owned by `priv_user.uid:api-user.gid` with permission `0640`
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

/// Atomically write data to file owned by `priv_user.uid:priv_user.gid` with permission `0600`.
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
