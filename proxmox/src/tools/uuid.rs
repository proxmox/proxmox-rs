//! Simple bindings to libuuid's `uuid_generate`.

use std::borrow::{Borrow, BorrowMut};
use std::fmt;

use failure::{bail, Error};

use crate::tools::parse::hex_nibble;

#[link(name = "uuid")]
extern "C" {
    fn uuid_generate(out: *mut [u8; 16]);
    fn uuid_unparse_lower(input: *const [u8; 16], out: *mut u8);
    fn uuid_unparse_upper(input: *const [u8; 16], out: *mut u8);
}

/// Uuid generated with the system's native libuuid.
///
/// ```
/// use proxmox::tools::uuid::Uuid;
///
/// let uuid = Uuid::generate();
/// println!("Generated uuid: {}", uuid);
/// // prints somethign like:
/// //    Generated uuid: 65b85639-78d7-4330-85c6-39502b2f9b01
///
/// let bytes: &[u8] = uuid.as_ref();
/// println!("raw byte string: {:?}", bytes);
/// //    raw byte string: [101, 184, 86, 57, 120, 215, 67, 48, 133, 198, 57, 80, 43, 47, 155, 1]
///
/// let text = format!("{}", uuid);
/// let parsed: Uuid = text.parse().unwrap();
/// assert_eq!(uuid, parsed);
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Uuid(Box<[u8; 16]>);

impl Uuid {
    /// Generate a uuid with `uuid_generate(3)`.
    pub fn generate() -> Self {
        use std::alloc::{alloc, Layout};
        let uuid = unsafe { alloc(Layout::new::<[u8; 16]>()) as *mut [u8; 16] };
        unsafe { uuid_generate(uuid) };
        Self(unsafe { Box::from_raw(uuid) })
    }

    /// Get a reference to the internal 16 byte array.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &*self.0
    }

    /// Take out the inner boxed 16 byte array.
    pub fn into_inner(self) -> Box<[u8; 16]> {
        self.0
    }

    /// Parse a uuid in optionally-hyphenated format.
    ///
    /// ```
    /// use proxmox::tools::uuid::Uuid;
    ///
    /// let gen = Uuid::generate();
    /// let text = format!("{}", gen);
    /// let parsed: Uuid = text.parse().unwrap();
    /// assert_eq!(gen, parsed);
    ///
    /// let uuid1: Uuid = "65b8563978d7433085c639502b2f9b01".parse().unwrap();
    /// let uuid2: Uuid = "65b85639-78d7-4330-85c6-39502b2f9b01".parse().unwrap();
    /// assert_eq!(uuid1, uuid2);
    /// ```
    pub fn parse_str(src: &str) -> Result<Self, Error> {
        use std::alloc::{alloc, Layout};
        let uuid: *mut [u8; 16] = unsafe { alloc(Layout::new::<[u8; 16]>()) as *mut [u8; 16] };
        if src.len() == 36 {
            // Unfortunately the manpage of `uuid_parse(3)` states that it technically requiers a
            // terminating null byte at the end, which we don't have, so do this manually:
            let uuid: &mut [u8] = unsafe { &mut (*uuid)[..] };
            let src = src.as_bytes();
            if src[8] != b'-' || src[13] != b'-' || src[18] != b'-' || src[23] != b'-' {
                bail!("failed to parse uuid");
            }
            for i in 0..4 {
                uuid[i] = hex_nibble(src[2 * i])? << 4 | hex_nibble(src[2 * i + 1])?;
            }
            for i in 4..6 {
                uuid[i] = hex_nibble(src[2 * i + 1])? << 4 | hex_nibble(src[2 * i + 2])?;
            }
            for i in 6..8 {
                uuid[i] = hex_nibble(src[2 * i + 2])? << 4 | hex_nibble(src[2 * i + 3])?;
            }
            for i in 8..10 {
                uuid[i] = hex_nibble(src[2 * i + 3])? << 4 | hex_nibble(src[2 * i + 4])?;
            }
            for i in 10..16 {
                uuid[i] = hex_nibble(src[2 * i + 4])? << 4 | hex_nibble(src[2 * i + 5])?;
            }
        } else if src.len() == 32 {
            let uuid: &mut [u8] = unsafe { &mut (*uuid)[..] };
            let src = src.as_bytes();
            for i in 0..16 {
                uuid[i] = hex_nibble(src[2 * i])? << 4 | hex_nibble(src[2 * i + 1])?;
            }
        } else {
            bail!("unrecognized uuid format");
        }
        Ok(Self(unsafe { Box::from_raw(uuid) }))
    }
}

impl AsRef<[u8]> for Uuid {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsMut<[u8]> for Uuid {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut (self.0)[..]
    }
}

impl Borrow<[u8]> for Uuid {
    #[inline]
    fn borrow(&self) -> &[u8] {
        &(self.0)[..]
    }
}

impl BorrowMut<[u8]> for Uuid {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [u8] {
        &mut (self.0)[..]
    }
}

impl From<[u8; 16]> for Uuid {
    fn from(data: [u8; 16]) -> Self {
        Self(Box::new(data))
    }
}

impl From<Box<[u8; 16]>> for Uuid {
    fn from(data: Box<[u8; 16]>) -> Self {
        Self(data)
    }
}

impl Into<[u8; 16]> for Uuid {
    fn into(self) -> [u8; 16] {
        *self.0
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(self, f)
    }
}

impl fmt::LowerHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_lower(self.as_bytes(), buf.as_mut_ptr());
        }
        write!(f, "{}", unsafe {
            std::str::from_utf8_unchecked(&buf[..36])
        })
    }
}

impl fmt::UpperHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_upper(self.as_bytes(), buf.as_mut_ptr());
        }
        write!(f, "{}", unsafe {
            std::str::from_utf8_unchecked(&buf[..36])
        })
    }
}

impl std::str::FromStr for Uuid {
    type Err = Error;

    fn from_str(src: &str) -> Result<Self, Error> {
        Self::parse_str(src)
    }
}
