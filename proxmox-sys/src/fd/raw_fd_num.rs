use std::borrow::Borrow;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use super::FdRef;

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

