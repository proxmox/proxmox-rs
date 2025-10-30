//! Helpers for `io::Error` and `nix::Error`.
//!
//! When dealing with low level functionality, the `nix` crate contains a lot of helpful
//! functionality. Unfortunately it also contains its own error type which doesn't mix all too well
//! with `std::io::Error`. Some of the common cases we want to deal with is checking for `EAGAIN`,
//! `EWOULDBLOCK` or `ENOENT` specifically. To do this easily, we add the `SysError` trait (rather
//! than a type), which is implemented for both these errors and allows checking for the most
//! common errors in a unified way.
//!
//! Another problem with the `nix` error type is that it has no equivalent to `ErrorKind::Other`.
//! Unfortunately this is more difficult to deal with, so for now, we consider `io::Error` to be
//! the more general error (and require an `into_io_error` implementation in `SysError`).
//!
//! See the `SysError` and `SysResult` traits for examples.

use std::io;

use nix::errno::Errno;

/// This trait should be implemented for error types which can represent system errors. Note that
/// it is discouraged to try to map non-system errors to with this trait, since users of this trait
/// assume there to be a relation between the error code and a previous system call. For instance,
/// `error.is_errno(Errno::EAGAIN)` can be used when implementing a reactor to drive async I/O.
///
/// Usage examples:
///
/// ```
/// # use anyhow::{bail, Error};
/// use nix::{dir::Dir, fcntl::OFlag, sys::stat::Mode};
///
/// use proxmox_sys::error::SysError;
///
/// # fn test() -> Result<(), Error> {
///
/// match Dir::open(".", OFlag::O_RDONLY, Mode::empty()) {
///     Ok(_dir) => {
///         // Do something
///     }
///     Err(ref err) if err.not_found() => {
///         // Handle ENOENT specially
///     }
///     Err(err) => bail!("failed to open directory: {}", err),
/// }
///
/// # Ok(())
/// # }
/// ```
pub trait SysError {
    /// Check if this error is a specific error returned from a system call.
    fn is_errno(&self, value: Errno) -> bool;

    /// Convert this error into a `std::io::Error`. This must use the correct `std::io::ErrorKind`,
    /// so that for example `ErrorKind::WouldBlock` means that the previous system call failed with
    /// `EAGAIN`.
    fn into_io_error(self) -> io::Error;

    /// Convenience shortcut to check for `EAGAIN`.
    #[inline]
    fn would_block(&self) -> bool {
        self.is_errno(Errno::EAGAIN)
    }

    /// Convenience shortcut to check for `ENOENT`.
    #[inline]
    fn not_found(&self) -> bool {
        self.is_errno(Errno::ENOENT)
    }

    /// Convenience shortcut to check for `EEXIST`.
    #[inline]
    fn already_exists(&self) -> bool {
        self.is_errno(Errno::EEXIST)
    }
}

impl SysError for io::Error {
    #[inline]
    fn is_errno(&self, value: Errno) -> bool {
        self.raw_os_error() == Some(value as i32)
    }

    #[inline]
    fn into_io_error(self) -> io::Error {
        self
    }
}

impl SysError for nix::Error {
    #[inline]
    fn is_errno(&self, value: Errno) -> bool {
        *self == value
    }

    #[inline]
    fn into_io_error(self) -> io::Error {
        match self {
            Errno::UnknownErrno => io::Error::other("unknown error".to_string()),
            other => io::Error::from_raw_os_error(other as _),
        }
    }
}

/// Convenience helper to map a `Result<_, nix::Error>` to a `Result<_, std::io::Error>` quickly.
///
/// Usage example:
///
/// ```no_run
/// # use std::os::unix::io::RawFd;
/// # use anyhow::{bail, Error};
///
/// use proxmox_sys::error::SysResult;
///
/// struct MyReader(RawFd);
///
/// impl std::io::Read for MyReader {
///     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
///         nix::unistd::read(self.0, buf).into_io_result()
///     }
///
///     // ... rest
/// }
/// ```
pub trait SysResult {
    type Ok;

    fn into_io_result(self) -> io::Result<Self::Ok>;
}

impl<T> SysResult for Result<T, nix::Error> {
    type Ok = T;

    #[inline]
    fn into_io_result(self) -> io::Result<T> {
        self.map_err(|e| e.into_io_error())
    }
}

macro_rules! other_error {
    ($err:ty) => {
        impl<T> SysResult for Result<T, $err> {
            type Ok = T;

            #[inline]
            fn into_io_result(self) -> io::Result<T> {
                self.map_err(::std::io::Error::other)
            }
        }
    };
}

other_error!(std::char::ParseCharError);
other_error!(std::net::AddrParseError);
other_error!(std::num::ParseFloatError);
other_error!(std::num::ParseIntError);
other_error!(std::str::ParseBoolError);
other_error!(std::str::Utf8Error);
other_error!(std::string::FromUtf8Error);
