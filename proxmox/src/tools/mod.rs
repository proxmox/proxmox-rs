//! This is a general utility crate used by all our rust projects.

use failure::*;
use lazy_static::lazy_static;

pub mod as_any;
pub mod borrow;
pub mod common_regex;
pub mod fd;
pub mod fs;
pub mod io;
pub mod parse;
pub mod serde;
pub mod uuid;
pub mod vec;

#[doc(inline)]
pub use uuid::Uuid;

#[doc(inline)]
pub use as_any::AsAny;

/// Evaluates to the offset (in bytes) of a given member within a struct
#[macro_export]
macro_rules! offsetof {
    ($ty:ty, $field:ident) => {
        unsafe { &(*(0 as *const $ty)).$field as *const _ as usize }
    };
}

/// Statically assert the size of a type at compile time.
///
/// This should compile:
/// ```
/// # use proxmox::static_assert_size;
/// #[repr(C)]
/// struct Stuff {
///     value: [u8; 32]
/// }
/// static_assert_size!(Stuff, 32);
/// ```
///
/// This should fail to compile:
/// ```compile_fail
/// # use proxmox::static_assert_size;
/// #[repr(C)]
/// struct Stuff {
///     value: [u8; 32]
/// }
/// static_assert_size!(Stuff, 128);
/// ```
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: fn() -> () = || {
            let _ = ::std::mem::transmute::<[u8; $size], $ty>;
        };
    };
}

/// Macro to write error-handling blocks (like perl eval {})
///
/// #### Example:
/// ```
/// # use proxmox::try_block;
/// # use failure::*;
/// # let some_condition = false;
/// let result = try_block!({
///     if (some_condition) {
///         bail!("some error");
///     }
///     Ok(())
/// })
/// .map_err(|e| format_err!("my try block returned an error - {}", e));
/// ```

#[macro_export]
macro_rules! try_block {
    { $($token:tt)* } => {{ (|| -> Result<_,_> { $($token)* })() }}
}

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

pub fn digest_to_hex(digest: &[u8]) -> String {
    bin_to_hex(digest)
}

/// Convert a byte slice to a string of hexadecimal digits.
///
/// ```
/// # use proxmox::tools::bin_to_hex;
///
/// let text = bin_to_hex(&[1, 2, 0xff]);
/// assert_eq!(text, "0102ff");
/// ```
pub fn bin_to_hex(digest: &[u8]) -> String {
    let mut buf = Vec::<u8>::with_capacity(digest.len() * 2);

    for i in 0..digest.len() {
        buf.push(HEX_CHARS[(digest[i] >> 4) as usize]);
        buf.push(HEX_CHARS[(digest[i] & 0xf) as usize]);
    }

    unsafe { String::from_utf8_unchecked(buf) }
}

/// Convert a string of hexadecimal digits to a byte vector. Any non-digits are treated as an
/// error, so when there is possible whitespace in the string it must be stripped by the caller
/// first. Also, only full bytes are allowed, so the input must consist of an even number of
/// digits.
///
/// ```
/// # use proxmox::tools::hex_to_bin;
///
/// let data = hex_to_bin("aabb0123").unwrap();
/// assert_eq!(&data, &[0xaa, 0xbb, 0x01, 0x23]);
/// ```
pub fn hex_to_bin(hex: &str) -> Result<Vec<u8>, Error> {
    let mut result = vec![];

    let bytes = hex.as_bytes();

    if (bytes.len() % 2) != 0 {
        bail!("hex_to_bin: got wrong input length.");
    }

    let val = |c| {
        if c >= b'0' && c <= b'9' {
            return Ok(c - b'0');
        }
        if c >= b'a' && c <= b'f' {
            return Ok(c - b'a' + 10);
        }
        if c >= b'A' && c <= b'F' {
            return Ok(c - b'A' + 10);
        }
        bail!("found illegal hex character.");
    };

    for pair in bytes.chunks(2) {
        let h = val(pair[0])?;
        let l = val(pair[1])?;
        result.push((h << 4) | l);
    }

    Ok(result)
}

// FIXME: This should be renamed to contain the digest algorithm, so that the array's size makes
// sense.
pub fn hex_to_digest(hex: &str) -> Result<[u8; 32], Error> {
    let mut digest = [0u8; 32];

    let bytes = hex.as_bytes();

    if bytes.len() != 64 {
        bail!("got wrong digest length.");
    }

    let val = |c| {
        if c >= b'0' && c <= b'9' {
            return Ok(c - b'0');
        }
        if c >= b'a' && c <= b'f' {
            return Ok(c - b'a' + 10);
        }
        if c >= b'A' && c <= b'F' {
            return Ok(c - b'A' + 10);
        }
        bail!("found illegal hex character.");
    };

    let mut pos = 0;
    for pair in bytes.chunks(2) {
        if pos >= digest.len() {
            bail!("hex digest too long.");
        }
        let h = val(pair[0])?;
        let l = val(pair[1])?;
        digest[pos] = (h << 4) | l;
        pos += 1;
    }

    if pos != digest.len() {
        bail!("hex digest too short.");
    }

    Ok(digest)
}

/// Returns the hosts node name (UTS node name)
pub fn nodename() -> &'static str {
    lazy_static! {
        static ref NODENAME: String = {
            nix::sys::utsname::uname()
                .nodename()
                .split('.')
                .next()
                .unwrap()
                .to_owned()
        };
    }

    &NODENAME
}
