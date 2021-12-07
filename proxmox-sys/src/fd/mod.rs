//! Raw file descriptor related structures.

mod fd_impl;
pub use fd_impl::*;

mod raw_fd_num;
pub use raw_fd_num::*;

mod borrowed_fd;
pub use borrowed_fd::*;

use std::os::unix::io::RawFd;

use nix::fcntl::{fcntl, FdFlag, F_GETFD, F_SETFD};

/// Change the `O_CLOEXEC` flag of an existing file descriptor.
pub fn fd_change_cloexec(fd: RawFd, on: bool) -> Result<(), anyhow::Error> {
    let mut flags = unsafe { FdFlag::from_bits_unchecked(fcntl(fd, F_GETFD)?) };
    flags.set(FdFlag::FD_CLOEXEC, on);
    fcntl(fd, F_SETFD(flags))?;
    Ok(())
}
