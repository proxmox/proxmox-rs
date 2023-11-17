//! Linux specific helpers and syscall wrapper

use anyhow::{bail, Error};

use proxmox_io::vec;

pub mod magic;
pub mod pid;
pub mod procfs;
pub mod socket;
#[cfg(feature = "timer")]
pub mod timer;
pub mod tty;

/// Get pseudo random data (/dev/urandom)
pub fn random_data(size: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = vec::undefined(size);
    fill_with_random_data(&mut buffer)?;

    Ok(buffer)
}

/// Fill buffer with pseudo random data (/dev/urandom)
///
/// This code uses the Linux syscall getrandom() - see "man 2 getrandom".
pub fn fill_with_random_data(buffer: &mut [u8]) -> Result<(), Error> {
    let res = unsafe {
        libc::getrandom(
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as libc::size_t,
            0_u32,
        )
    };

    if res == -1 {
        return Err(std::io::Error::last_os_error().into());
    }

    if res as usize != buffer.len() {
        // should not happen
        bail!("short getrandom read");
    }

    Ok(())
}
