use std::ffi::{c_int, CString, OsStr};
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;

use crate::sys;

pub fn stream_fd<I: AsRef<OsStr>>(
    identifier: I,
    priority: c_int,
    level_prefix: bool,
) -> Result<OwnedFd, io::Error> {
    let ident = CString::new(identifier.as_ref().as_bytes())
        .map_err(|_| io::Error::other("invalid identifier for journal stream"))?;
    let fd = unsafe {
        sys::sd_journal_stream_fd(ident.as_bytes().as_ptr(), priority, level_prefix as c_int)
    };
    if fd < 0 {
        Err(std::io::Error::from_raw_os_error(-fd))
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}
