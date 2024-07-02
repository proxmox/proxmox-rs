#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::os::unix::ffi::OsStrExt;

pub mod boot_mode;
pub mod command;
#[cfg(feature = "crypt")]
pub mod crypt;
pub mod email;
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

mod worker_task_context;
pub use worker_task_context::*;

#[allow(deprecated)]
use fd::Fd;

/// Returns the hosts node name (UTS node name)
pub fn nodename() -> &'static str {
    lazy_static::lazy_static! {
        static ref NODENAME: String = {
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
        };
    }

    &NODENAME
}

/// Safe wrapper for `nix::unistd::pipe2` defaulting to `O_CLOEXEC`
/// and guarding the file descriptors.
#[allow(deprecated)]
pub fn pipe() -> Result<(Fd, Fd), nix::Error> {
    let (pin, pout) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
    Ok((Fd(pin), Fd(pout)))
}
