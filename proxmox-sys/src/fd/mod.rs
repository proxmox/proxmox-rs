//! Raw file descriptor related structures.

use std::os::unix::io::AsRawFd;

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::NixPath;

mod fd_impl;
pub use fd_impl::*;

mod raw_fd_num;
pub use raw_fd_num::*;

mod borrowed_fd;
pub use borrowed_fd::*;

use std::os::unix::io::{FromRawFd, OwnedFd, RawFd};

use nix::fcntl::{fcntl, FdFlag, F_GETFD, F_SETFD};

/// Change the `O_CLOEXEC` flag of an existing file descriptor.
pub fn fd_change_cloexec(fd: RawFd, on: bool) -> Result<(), anyhow::Error> {
    change_cloexec(fd, on)
}

/// Change the `O_CLOEXEC` flag of an existing file descriptor.
pub fn change_cloexec(fd: RawFd, on: bool) -> Result<(), anyhow::Error> {
    let mut flags = unsafe { FdFlag::from_bits_unchecked(fcntl(fd, F_GETFD)?) };
    flags.set(FdFlag::FD_CLOEXEC, on);
    fcntl(fd, F_SETFD(flags))?;
    Ok(())
}

pub fn cwd() -> OwnedFd {
    unsafe { OwnedFd::from_raw_fd(libc::AT_FDCWD) }
}

pub fn open<P>(path: &P, oflag: OFlag, mode: Mode) -> Result<OwnedFd, nix::Error>
where
    P: ?Sized + NixPath,
{
    nix::fcntl::open(path, oflag, mode).map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
}

pub fn openat<D, P>(dirfd: &D, path: &P, oflag: OFlag, mode: Mode) -> Result<OwnedFd, nix::Error>
where
    D: AsRawFd,
    P: ?Sized + NixPath,
{
    nix::fcntl::openat(dirfd.as_raw_fd(), path, oflag, mode)
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
}
