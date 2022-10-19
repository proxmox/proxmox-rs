use std::borrow::Borrow;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::NixPath;

/// Guard a raw file descriptor with a drop handler. This is mostly useful when access to an owned
/// `RawFd` is required without the corresponding handler object (such as when only the file
/// descriptor number is required in a closure which may be dropped instead of being executed).
#[derive(Debug)]
#[deprecated(note = "use std::os::unix::io::OwnedFd instead")]
pub struct Fd(pub RawFd);

#[allow(deprecated)]
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

#[allow(deprecated)]
impl Fd {
    #[deprecated(note = "use proxmox_sys::fd::cwd instead")]
    pub const fn cwd() -> Self {
        Self(libc::AT_FDCWD)
    }

    #[deprecated(note = "use proxmox_sys::fd::open instead")]
    pub fn open<P>(path: &P, oflag: OFlag, mode: Mode) -> Result<Self, nix::Error>
    where
        P: ?Sized + NixPath,
    {
        nix::fcntl::open(path, oflag, mode).map(Self)
    }

    #[deprecated(note = "use proxmox_sys::fd::openat instead")]
    pub fn openat<D, P>(dirfd: &D, path: &P, oflag: OFlag, mode: Mode) -> Result<Self, nix::Error>
    where
        D: AsRawFd,
        P: ?Sized + NixPath,
    {
        nix::fcntl::openat(dirfd.as_raw_fd(), path, oflag, mode).map(Self)
    }

    /// Borrow this file descriptor as an `&FdRef`.
    pub fn as_fd_ref(&self) -> &FdRef {
        unsafe { &*(&self.0 as *const RawFd as *const FdRef) }
    }
}

#[allow(deprecated)]
impl FromRawFd for Fd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
    }
}

#[allow(deprecated)]
impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

#[allow(deprecated)]
impl IntoRawFd for Fd {
    fn into_raw_fd(mut self) -> RawFd {
        let fd = self.0;
        self.0 = -1;
        fd
    }
}

#[allow(deprecated)]
impl AsRef<FdRef> for Fd {
    fn as_ref(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

#[allow(deprecated)]
impl Borrow<FdRef> for Fd {
    fn borrow(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

#[allow(deprecated)]
impl std::ops::Deref for Fd {
    type Target = FdRef;

    fn deref(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

/// A reference to a raw file descriptor. (Strongly typed `&RawFd` which is not equivalent to an
/// `&i32`.
///
/// `RawFd` should only be used as parameter type for functions, but never as a return type, since
/// it is not clear whether the returned integer is borrowed or owned. Instead, functions should
/// return `Fd` or `&FdRef`.
///
/// This specifically does not implement `IntoRawFd` or `FromRawFd`, since those would drop life
/// times.
#[derive(Debug, Eq, PartialEq)]
pub enum FdRef {}

impl AsRawFd for FdRef {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        unsafe { *(self as *const Self as *const RawFd) }
    }
}
