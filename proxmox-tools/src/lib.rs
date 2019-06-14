//! This is a general utility crate used by all our rust projects.

use failure::*;

pub mod io;
pub mod vec;

/// Evaluates to the offset (in bytes) of a given member within a struct
#[macro_export]
macro_rules! offsetof {
    ($ty:ty, $field:ident) => {
        unsafe { &(*(0 as *const $ty)).$field as *const _ as usize }
    }
}

const HEX_CHARS: &'static [u8; 16] = b"0123456789abcdef";

pub fn digest_to_hex(digest: &[u8]) -> String {
    let mut buf = Vec::<u8>::with_capacity(digest.len()*2);

    for i in 0..digest.len() {
        buf.push(HEX_CHARS[(digest[i] >> 4) as usize]);
        buf.push(HEX_CHARS[(digest[i] & 0xf) as usize]);
    }

    unsafe { String::from_utf8_unchecked(buf) }
}

pub fn hex_to_digest(hex: &str) -> Result<[u8; 32], Error> {
    let mut digest = [0u8; 32];

    let bytes = hex.as_bytes();

    if bytes.len() != 64 { bail!("got wrong digest length."); }

    let val = |c| {
        if c >= b'0' && c <= b'9' { return Ok(c - b'0'); }
        if c >= b'a' && c <= b'f' { return Ok(c - b'a' + 10); }
        if c >= b'A' && c <= b'F' { return Ok(c - b'A' + 10); }
        bail!("found illegal hex character.");
    };

    let mut pos = 0;
    for pair in bytes.chunks(2) {
        if pos >= digest.len() { bail!("hex digest too long."); }
        let h = val(pair[0])?;
        let l = val(pair[1])?;
        digest[pos] = (h<<4)|l;
        pos +=1;
    }

    if pos != digest.len() {  bail!("hex digest too short."); }

    Ok(digest)
}
