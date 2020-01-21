//! Raw file descriptor related structures.

use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::NixPath;

/// Guard a raw file descriptor with a drop handler. This is mostly useful when access to an owned
/// `RawFd` is required without the corresponding handler object (such as when only the file
/// descriptor number is required in a closure which may be dropped instead of being executed).
pub struct Fd(pub RawFd);

impl Drop for Fd {
    fn drop(&mut self) {
        // `>= 0` instead of `!= -1` to also handle things like AT_FDCWD
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl IntoRawFd for Fd {
    fn into_raw_fd(mut self) -> RawFd {
        let fd = self.0;
        self.0 = -1;
        fd
    }
}

impl FromRawFd for Fd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
    }
}

impl Fd {
    pub const fn cwd() -> Self {
        Self(libc::AT_FDCWD)
    }

    pub fn open<P>(path: &P, oflag: OFlag, mode: Mode) -> Result<Self, nix::Error>
    where
        P: ?Sized + NixPath,
    {
        nix::fcntl::open(path, oflag, mode).map(Self)
    }

    pub fn openat<D, P>(dirfd: &D, path: &P, oflag: OFlag, mode: Mode) -> Result<Self, nix::Error>
    where
        D: AsRawFd,
        P: ?Sized + NixPath,
    {
        nix::fcntl::openat(dirfd.as_raw_fd(), path, oflag, mode).map(Self)
    }
}
