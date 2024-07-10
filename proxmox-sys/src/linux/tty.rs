use std::io::{self, IsTerminal, Read, Write};
use std::mem::MaybeUninit;
use std::os::unix::io::{AsRawFd, OwnedFd};

use anyhow::{bail, format_err, Error};
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;

use proxmox_lang::try_block;

use crate::c_try;

/// Get the current size of the terminal (for stdout).
/// # Safety
///
/// uses unsafe call to tty_ioctl, see man tty_ioctl(2).
pub fn stdout_terminal_size() -> (usize, usize) {
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) };
    (winsize.ws_row as usize, winsize.ws_col as usize)
}

pub enum TtyOutput {
    Stdout(std::io::Stdout),
    DevTty(OwnedFd),
}

impl Write for TtyOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            TtyOutput::Stdout(out) => out.write(buf),
            TtyOutput::DevTty(out) => {
                let written = c_try!(unsafe {
                    libc::write(
                        out.as_raw_fd(),
                        buf.as_ptr() as *const libc::c_void,
                        buf.len(),
                    )
                });
                Ok(written as usize)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            TtyOutput::Stdout(out) => out.flush(),
            TtyOutput::DevTty(_) => Ok(()),
        }
    }
}

impl TtyOutput {
    /// Get an output file descriptor for the current terminal.
    pub fn open() -> io::Result<Option<Self>> {
        let stdout = std::io::stdout();
        if stdout.is_terminal() {
            Ok(Some(TtyOutput::Stdout(stdout)))
        } else {
            match crate::fd::open(
                "/dev/tty",
                OFlag::O_WRONLY | OFlag::O_CLOEXEC | OFlag::O_NOCTTY,
                Mode::empty(),
            ) {
                Ok(fd) => Ok(Some(TtyOutput::DevTty(fd))),
                Err(nix::errno::Errno::ENXIO) => Ok(None),
                Err(err) => Err(err.into()),
            }
        }
    }
}

/// Read a password from stdin.
///
/// Masking the echoed output with asterisks and writing a query
/// first.
pub fn read_password(query: &str) -> Result<Vec<u8>, Error> {
    let input = std::io::stdin();
    if !input.is_terminal() {
        let mut out = String::new();
        input.read_line(&mut out)?;
        return Ok(out.into_bytes());
    }

    let mut out = TtyOutput::open()?
        .ok_or_else(|| format_err!("cannot read password without a controlling tty"))?;

    let _ignore_error = out.write_all(query.as_bytes());
    let _ignore_error = out.flush();

    let infd = input.as_raw_fd();
    let mut termios = MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(infd, &mut *termios.as_mut_ptr()) } != 0 {
        bail!("tcgetattr() failed");
    }
    let mut termios = unsafe { termios.assume_init() };
    let old_termios = termios; // termios is a 'Copy' type
    unsafe {
        libc::cfmakeraw(&mut termios);
    }
    if unsafe { libc::tcsetattr(infd, libc::TCSANOW, &termios) } != 0 {
        bail!("tcsetattr() failed");
    }

    let mut password = Vec::<u8>::new();
    let mut asterisks = true;

    let ok: Result<(), Error> = try_block!({
        for byte in input.bytes() {
            let byte = byte?;
            match byte {
                3 => bail!("cancelled"), // ^C
                4 => break,              // ^D / EOF
                9 => asterisks = false,  // tab disables echo
                0xA | 0xD => {
                    // newline, we're done
                    let _ignore_error = out.write_all(b"\r\n");
                    let _ignore_error = out.flush();
                    break;
                }
                0x7F => {
                    // backspace
                    if !password.is_empty() {
                        password.pop();
                        if asterisks {
                            let _ignore_error = out.write_all(b"\x08 \x08");
                            let _ignore_error = out.flush();
                        }
                    }
                }
                other => {
                    password.push(other);
                    if asterisks {
                        let _ignore_error = out.write_all(b"*");
                        let _ignore_error = out.flush();
                    }
                }
            }
        }
        Ok(())
    });
    if unsafe { libc::tcsetattr(infd, libc::TCSANOW, &old_termios) } != 0 {
        // not fatal...
        eprintln!("failed to reset terminal attributes!");
    }
    match ok {
        Ok(_) => Ok(password),
        Err(e) => Err(e),
    }
}

/// Read a password from stdin, then read again to verify it.
pub fn read_and_verify_password(prompt: &str) -> Result<Vec<u8>, Error> {
    let password = String::from_utf8(read_password(prompt)?)?;
    let verify_password = String::from_utf8(read_password("Verify Password: ")?)?;

    if password != verify_password {
        bail!("Passwords do not match!");
    }

    if password.len() < 5 {
        bail!("Password too short!");
    }

    Ok(password.into_bytes())
}
