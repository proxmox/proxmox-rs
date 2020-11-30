//! Raw file descriptor related structures.

use std::borrow::Borrow;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::NixPath;

/// Guard a raw file descriptor with a drop handler. This is mostly useful when access to an owned
/// `RawFd` is required without the corresponding handler object (such as when only the file
/// descriptor number is required in a closure which may be dropped instead of being executed).
#[derive(Debug)]
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

    /// Borrow this file descriptor as an `&FdRef`.
    pub fn as_fd_ref(&self) -> &FdRef {
        unsafe { &*(&self.0 as *const RawFd as *const FdRef) }
    }
}

impl FromRawFd for Fd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
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

impl AsRef<FdRef> for Fd {
    fn as_ref(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

impl Borrow<FdRef> for Fd {
    fn borrow(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

impl std::ops::Deref for Fd {
    type Target = FdRef;

    fn deref(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

/// Raw file descriptor by number. Thin wrapper to provide `AsRawFd` which a simple `RawFd` does
/// not since it's just an `i32`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RawFdNum(RawFd);

impl RawFdNum {
    /// Borrow this file descriptor as an `&FdRef`.
    pub fn as_fd_ref(&self) -> &FdRef {
        unsafe { &*(&self.0 as *const RawFd as *const FdRef) }
    }
}

impl AsRawFd for RawFdNum {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl FromRawFd for RawFdNum {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
    }
}

impl IntoRawFd for RawFdNum {
    fn into_raw_fd(self) -> RawFd {
        self.0
    }
}

impl AsRef<FdRef> for RawFdNum {
    fn as_ref(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

impl Borrow<FdRef> for RawFdNum {
    fn borrow(&self) -> &FdRef {
        self.as_fd_ref()
    }
}

impl std::ops::Deref for RawFdNum {
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

/// A borrowed file raw descriptor. (A `RawFd` with an attached lifetime).
///
/// For when using `&FdRef` is not an option.
///
/// This specifically does not implement `IntoRawFd` or `FromRawFd`, since those would drop life
/// times.
#[derive(Debug, Eq, PartialEq)]
pub struct BorrowedFd<'a> {
    fd: RawFd,
    _borrow: PhantomData<&'a RawFd>,
}

impl<'a> BorrowedFd<'a> {
    #[inline]
    pub fn new<T: ?Sized + AsRawFd>(fd: &T) -> Self {
        Self {
            fd: fd.as_raw_fd(),
            _borrow: PhantomData,
        }
    }
}

impl AsRawFd for BorrowedFd<'_> {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl<'a, T: ?Sized + AsRawFd> From<&'a T> for BorrowedFd<'a> {
    #[inline]
    fn from(fd: &'a T) -> Self {
        Self::new(fd)
    }
}
