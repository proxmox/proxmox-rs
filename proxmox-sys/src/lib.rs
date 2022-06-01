use std::os::unix::ffi::OsStrExt;

pub mod command;
pub mod crypt;
pub mod email;
pub mod error;
pub mod fd;
pub mod fs;
pub mod linux;
pub mod logrotate;
pub mod macros;
pub mod mmap;
pub mod process_locker;
pub mod systemd;

mod worker_task_context;
pub use worker_task_context::*;

#[deprecated(
    since = "0.2.2",
    note = "the sortable macro does not require this anymore, it will be removed"
)]
/// An identity (nop) macro. Used by the `#[sortable]` proc macro.
#[cfg(feature = "sortable-macro")]
#[macro_export]
macro_rules! identity {
    ($($any:tt)*) => ($($any)*)
}

#[cfg(feature = "sortable-macro")]
pub use proxmox_sortable_macro::sortable;

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
pub fn pipe() -> Result<(Fd, Fd), nix::Error> {
    let (pin, pout) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)?;
    Ok((Fd(pin), Fd(pout)))
}
