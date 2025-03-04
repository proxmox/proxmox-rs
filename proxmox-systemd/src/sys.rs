use std::ffi::{c_char, c_int, c_uchar, c_uint};
use std::io;
use std::os::fd::RawFd;

pub const LISTEN_FDS_START: RawFd = 3;

#[link(name = "systemd")]
unsafe extern "C" {
    pub fn sd_journal_stream_fd(
        identifier: *const c_uchar,
        priority: c_int,
        level_prefix: c_int,
    ) -> c_int;
    pub fn sd_notify(unset_environment: c_int, state: *const c_char) -> c_int;
    pub fn sd_notify_barrier(unset_environment: c_int, timeout: u64) -> c_int;
    pub fn sd_pid_notify_with_fds(
        pid: libc::pid_t,
        unset_environment: c_int,
        state: *const c_char,
        fds: *const c_int,
        n_fds: c_uint,
    ) -> c_int;
    pub fn sd_listen_fds_with_names(
        unset_environment: c_int,
        names: *mut *mut *mut c_char,
    ) -> c_int;
}

pub fn check_call(ret: c_int) -> Result<c_int, io::Error> {
    if ret < 0 {
        Err(io::Error::from_raw_os_error(-ret))
    } else {
        Ok(ret)
    }
}
