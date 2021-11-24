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
