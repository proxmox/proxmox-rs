use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};

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
