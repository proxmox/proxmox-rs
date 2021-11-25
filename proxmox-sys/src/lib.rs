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
            nix::sys::utsname::uname()
                .nodename()
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
