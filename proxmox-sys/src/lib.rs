#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::sync::LazyLock;

pub mod boot_mode;
pub mod command;
#[cfg(feature = "crypt")]
pub mod crypt;
pub mod error;
pub mod fd;
pub mod fs;
pub mod linux;
#[cfg(feature = "logrotate")]
pub mod logrotate;
pub mod macros;
pub mod mmap;
pub mod process_locker;
pub mod systemd;

/// Returns the hosts node name (UTS node name)
pub fn nodename() -> &'static str {
    static NODENAME: LazyLock<String> = LazyLock::new(|| {
        std::str::from_utf8(
            nix::sys::utsname::uname()
                .expect("failed to get nodename")
                .nodename()
                .as_bytes(),
        )
        .expect("non utf-8 nodename not supported")
        .split('.')
        .next()
        .unwrap()
        .to_owned()
    });

    &NODENAME
}

/// Safe wrapper for `nix::unistd::pipe2` defaulting to `O_CLOEXEC`
/// and guarding the file descriptors.
pub fn pipe() -> Result<(OwnedFd, OwnedFd), nix::Error> {
    let (pin, pout) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
    Ok(unsafe { (OwnedFd::from_raw_fd(pin), OwnedFd::from_raw_fd(pout)) })
}
