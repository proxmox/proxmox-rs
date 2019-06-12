//! Linux specific helpers and syscall wrapper

use failure::*;
use proxmox_tools as tools;

/// Get pseudo random data (/dev/urandom)
pub fn random_data(size: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = tools::vec::undefined(size);
    fill_with_random_data(&mut buffer)?;

    Ok(buffer)
}

/// Fill buffer with pseudo random data (/dev/urandom)
///
/// This code uses the Linux syscall getrandom() - see "man 2 getrandom".
pub fn fill_with_random_data(buffer: &mut [u8]) -> Result<(), Error> {
    let res = unsafe {
        libc::syscall(
            libc::SYS_getrandom,
            buffer.as_mut_ptr(),
            buffer.len() as libc::size_t,
            0 as libc::c_uint,
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
