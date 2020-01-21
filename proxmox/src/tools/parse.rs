//! Some parsing utilities.

use failure::{bail, Error};

/// Parse a hexadecimal digit into a byte.
#[inline]
pub fn hex_nibble(c: u8) -> Result<u8, Error> {
    Ok(match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 0xa,
        b'A'..=b'F' => c - b'A' + 0xa,
        _ => bail!("not a hex digit: {}", c as char),
    })
}
