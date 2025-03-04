//! PID file descriptor handling.

use std::fs::File;
use std::io;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use nix::fcntl::OFlag;
use nix::sys::signal::Signal;
use nix::sys::signalfd::siginfo;
use nix::sys::stat::Mode;
use nix::unistd::Pid;
use nix::NixPath;

use crate::error::SysResult;
use crate::linux::procfs::{MountInfo, PidStat};
use crate::{c_result, c_try};

/// asm-generic pidfd_open syscall number
#[allow(non_upper_case_globals)]
pub const SYS_pidfd_open: libc::c_long = 434;

/// asm-generic pidfd_send_signal syscall number
#[allow(non_upper_case_globals)]
pub const SYS_pidfd_send_signal: libc::c_long = 424;

unsafe fn pidfd_open(pid: libc::pid_t, flags: libc::c_uint) -> libc::c_long {
    unsafe { libc::syscall(SYS_pidfd_open, pid, flags) }
}

unsafe fn pidfd_send_signal(
    pidfd: RawFd,
    sig: libc::c_int,
    info: *mut libc::siginfo_t,
    flags: libc::c_uint,
) -> libc::c_long {
    unsafe { libc::syscall(SYS_pidfd_send_signal, pidfd, sig, info, flags) }
}

/// File descriptor reference to a process.
pub struct PidFd {
    fd: OwnedFd,
    pid: Pid,
}

impl PidFd {
    /// Get a pidfd to the current process.
    pub fn current() -> io::Result<Self> {
        Self::open(Pid::this())
    }

    /// Open a pidfd for the given process id.
    pub fn open(pid: Pid) -> io::Result<Self> {
        let fd = unsafe { OwnedFd::from_raw_fd(c_try!(pidfd_open(pid.as_raw(), 0)) as RawFd) };
        Ok(Self { fd, pid })
    }

    /// Send a signal to the process.
    pub fn send_signal<S: Into<Option<Signal>>>(
        &self,
        sig: S,
        info: Option<&mut siginfo>,
    ) -> io::Result<()> {
        let sig = match sig.into() {
            Some(sig) => sig as libc::c_int,
            None => 0,
        };
        let info = match info {
            Some(info) => info as *mut siginfo as *mut libc::siginfo_t,
            None => std::ptr::null_mut(),
        };
        c_result!(unsafe { pidfd_send_signal(self.fd.as_raw_fd(), sig, info, 0) }).map(drop)
    }

    /// Get the original PID number used to open this pidfd. Note that this may not be the correct
    /// pid if the PID namespace was changed (which is currently only possible by forking, which is
    /// why this is usually safe under normal circumstances.)
    #[inline]
    pub const fn pid(&self) -> Pid {
        self.pid
    }

    /// Open a procfs file from. This is equivalent to opening `/proc/<pid>/<file>` using this
    /// process actual pid. This also works if the file descriptor has been sent over
    pub fn open_file<P: ?Sized + NixPath>(&self, path: &P) -> io::Result<File> {
        crate::fd::openat(
            self,
            path,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map(|fd| unsafe { File::from_raw_fd(fd.into_raw_fd()) })
        .into_io_result()
    }

    /// Convenience helper to read a procfs file into memory.
    ///
    /// This calls `self.open_file()` reads it to the end.
    pub fn read_file<P: ?Sized + NixPath>(&self, path: &P) -> io::Result<Vec<u8>> {
        use io::Read;

        let mut reader = self.open_file(path)?;
        let mut out = Vec::new();
        reader.read_to_end(&mut out)?;
        Ok(out)
    }

    /// Get the `PidStat` structure for this process. (`/proc/PID/stat`)
    pub fn get_stat(&self) -> io::Result<PidStat> {
        let data = self.read_file(c"stat")?;
        let data = String::from_utf8(data).map_err(io::Error::other)?;
        PidStat::parse(&data).map_err(io::Error::other)
    }

    /// Read this process' `/proc/PID/mountinfo` file.
    pub fn get_mount_info(&self) -> io::Result<MountInfo> {
        MountInfo::parse(&self.read_file(c"mountinfo")?).map_err(io::Error::other)
    }

    /// Attempt to get a `PidFd` from a raw file descriptor.
    ///
    /// This will attempt to read the pid number via the file descriptor.
    pub fn try_from_raw_fd(fd: RawFd) -> io::Result<Self> {
        let mut this = Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
            pid: Pid::from_raw(1),
        };
        // Simple check first: is it a valid pid file descriptor:
        if let Err(err) = this.send_signal(None, None) {
            if err.kind() == io::ErrorKind::PermissionDenied {
                // valid pidfd, but we probably can't do much with it, proceed anyway...
            } else {
                // make sure we don't try to close the file descriptor:
                let _ = this.fd.into_raw_fd();
                // if err.raw_os_error() == Some(libc::EBADF)
                //     => not a valid pid fd, pass the error through
                // if err.raw_os_error() == Some(libc::ENOSYS)
                //     => kernel too old, things most likely won't work anyway
                return Err(err);
            }
        }

        match this.get_stat() {
            Ok(stat) => {
                this.pid = stat.pid;
                Ok(this)
            }
            Err(err) => {
                // make sure we don't close the raw file descriptor:
                let _ = this.fd.into_raw_fd();
                Err(err)
            }
        }
    }
}

impl AsFd for PidFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl AsRawFd for PidFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl IntoRawFd for PidFd {
    fn into_raw_fd(self) -> RawFd {
        self.fd.into_raw_fd()
    }
}

impl FromRawFd for PidFd {
    /// Panics if the file descriptor is not an actual pid fd (or at least a reference to its proc
    /// directory).
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::try_from_raw_fd(fd).unwrap()
    }
}
