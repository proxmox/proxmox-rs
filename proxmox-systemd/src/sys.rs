use std::ffi::{c_char, c_int, c_uchar};
use std::io;

#[link(name = "systemd")]
extern "C" {
    pub fn sd_journal_stream_fd(
        identifier: *const c_uchar,
        priority: c_int,
        level_prefix: c_int,
    ) -> c_int;
    pub fn sd_notify(unset_environment: c_int, state: *const c_char) -> c_int;
    pub fn sd_notify_barrier(unset_environment: c_int, timeout: u64) -> c_int;
}

pub fn check_call(ret: c_int) -> Result<c_int, io::Error> {
    if ret < 0 {
        Err(io::Error::from_raw_os_error(-ret))
    } else {
        Ok(ret)
    }
}
