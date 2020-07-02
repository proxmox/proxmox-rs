//! Helper for creating a pseudo-terminal
//!
//! normally used like this:
//! ```norun
//! let (mut pty, secondary) = PTY::new()?;
//!
//! // fork somehow, e.g. std::process::Command
//! if child {
//!     make_controlling_terminal(secondary)?;
//!     // exec or exit
//! }
//!
//! // parent can read/write/set_size
//! pty.read(...);
//! pty.write(...);
//! pty.set_size(100,20);
//! ```

use std::os::unix::io::{AsRawFd, RawFd};

use nix::pty::{posix_openpt, grantpt, unlockpt, ptsname_r, PtyMaster};
use nix::fcntl::{OFlag};
use nix::sys::stat::Mode;
use nix::{ioctl_write_int_bad, ioctl_write_ptr_bad, Result};
use nix::unistd::{dup2, setsid};
use nix::errno::Errno::EINVAL;

use crate::tools::fd::Fd;

ioctl_write_int_bad!(set_controlling_tty, libc::TIOCSCTTY);
ioctl_write_ptr_bad!(set_size, libc::TIOCSWINSZ, nix::pty::Winsize);

pub struct PTY {
    primary: PtyMaster,
}

pub fn make_controlling_terminal(terminal: &str) -> Result<()> {
    setsid()?; // make new process group
    let mode = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH; // 0666
    let secondary_fd = Fd::open(terminal, OFlag::O_RDWR | OFlag::O_NOCTTY, mode)?;
    let s_raw_fd = secondary_fd.as_raw_fd();
    unsafe { set_controlling_tty(s_raw_fd, 0) }?;
    dup2(s_raw_fd, 0)?;
    dup2(s_raw_fd, 1)?;
    dup2(s_raw_fd, 2)?;

    if s_raw_fd <= 2 {
        std::mem::forget(secondary_fd); // don't call drop handler
    }

    Ok(())
}

impl PTY {
    pub fn new() -> Result<(Self, String)> {
        let primary = posix_openpt(OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)?;
        grantpt(&primary)?;
        unlockpt(&primary)?;
        let secondary = ptsname_r(&primary)?; // linux specific
        Ok((Self{
            primary,
        }, secondary))
    }

    pub fn set_size(&mut self, col: u16, row: u16) -> Result<()> {
        let size = nix::pty::Winsize{
            ws_row: row,
            ws_col: col,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe { set_size(self.primary.as_raw_fd(), &size) }?;

        Ok(())
    }
}

impl std::io::Read for PTY {
    fn read(&mut self, buf: &mut[u8]) -> std::io::Result<usize> {
        match nix::unistd::read(self.primary.as_raw_fd(), buf) {
            Ok(val) => Ok(val),
            Err(err) => Err(err.as_errno().unwrap_or(EINVAL).into()),
        }
    }
}

impl std::io::Write for PTY {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match nix::unistd::write(self.primary.as_raw_fd(), buf) {
            Ok(size) => Ok(size),
            Err(err) => Err(err.as_errno().unwrap_or(EINVAL).into()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for PTY {
    fn as_raw_fd(&self) -> RawFd{
        self.primary.as_raw_fd()
    }
}


