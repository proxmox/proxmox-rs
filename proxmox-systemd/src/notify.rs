use std::ffi::CString;
use std::io;

use crate::sys;

/// Systemd service startup states (see: ``man sd_notify``)
#[derive(Clone, Debug)]
pub enum SystemdNotify {
    Ready,
    Reloading,
    Stopping,
    Status(String),
    MainPid(libc::pid_t),
}

impl SystemdNotify {
    /// Tells systemd the startup state of the service (see: ``man sd_notify``)
    ///
    /// If a `SystemdNotify::Status` message cannot be converted to a C-String this returns an
    /// `io::ErrorKind::InvalidInput`.
    pub fn notify(self) -> Result<(), io::Error> {
        let cs;
        let message = match self {
            SystemdNotify::Ready => c"READY=1",
            SystemdNotify::Reloading => c"RELOADING=1",
            SystemdNotify::Stopping => c"STOPPING=1",
            SystemdNotify::Status(msg) => {
                cs = CString::new(msg)?;
                &cs
            }
            SystemdNotify::MainPid(pid) => {
                cs = CString::new(format!("MAINPID={}", pid))?;
                &cs
            }
        };
        sys::check_call(unsafe { sys::sd_notify(0, message.as_ptr()) }).map(drop)
    }
}

/// Waits until all previously sent messages with sd_notify are processed
pub fn barrier(timeout: u64) -> Result<(), io::Error> {
    sys::check_call(unsafe { sys::sd_notify_barrier(0, timeout) }).map(drop)
}
