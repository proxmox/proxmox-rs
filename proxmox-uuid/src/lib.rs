//! Simple bindings to libuuid's `uuid_generate`.

use std::borrow::{Borrow, BorrowMut};
use std::fmt;

#[link(name = "uuid")]
extern "C" {
    fn uuid_generate(out: *mut [u8; 16]);
    fn uuid_unparse_lower(input: *const [u8; 16], out: *mut u8);
    fn uuid_unparse_upper(input: *const [u8; 16], out: *mut u8);
}

/// An error parsing a uuid from a string.
#[derive(Debug, Clone, Copy)]
pub struct UuidError;

impl fmt::Display for UuidError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bad uuid format")
    }
}

impl std::error::Error for UuidError {}

/// Check for hex digits.
fn hex_digit(b: u8) -> Result<u8, UuidError> {
    Ok(match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 0xA,
        b'A'..=b'F' => b - b'A' + 0xA,
        _ => return Err(UuidError),
    })
}

/// Uuid generated with the system's native libuuid.
///
/// ```
/// use proxmox_uuid::Uuid;
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
    /// use proxmox_uuid::Uuid;
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
    pub fn parse_str(src: &str) -> Result<Self, UuidError> {
        use std::alloc::{alloc, Layout};
        let uuid: *mut [u8; 16] = unsafe { alloc(Layout::new::<[u8; 16]>()) as *mut [u8; 16] };
        if src.len() == 36 {
            // Unfortunately the manpage of `uuid_parse(3)` states that it technically requiers a
            // terminating null byte at the end, which we don't have, so do this manually:
            let uuid: &mut [u8] = unsafe { &mut (*uuid)[..] };
            let src = src.as_bytes();
            if src[8] != b'-' || src[13] != b'-' || src[18] != b'-' || src[23] != b'-' {
                return Err(UuidError);
            }
            for i in 0..4 {
                uuid[i] = hex_digit(src[2 * i])? << 4 | hex_digit(src[2 * i + 1])?;
            }
            for i in 4..6 {
                uuid[i] = hex_digit(src[2 * i + 1])? << 4 | hex_digit(src[2 * i + 2])?;
            }
            for i in 6..8 {
                uuid[i] = hex_digit(src[2 * i + 2])? << 4 | hex_digit(src[2 * i + 3])?;
            }
            for i in 8..10 {
                uuid[i] = hex_digit(src[2 * i + 3])? << 4 | hex_digit(src[2 * i + 4])?;
            }
            for i in 10..16 {
                uuid[i] = hex_digit(src[2 * i + 4])? << 4 | hex_digit(src[2 * i + 5])?;
            }
        } else if src.len() == 32 {
            let uuid: &mut [u8] = unsafe { &mut (*uuid)[..] };
            let src = src.as_bytes();
            for i in 0..16 {
                uuid[i] = hex_digit(src[2 * i])? << 4 | hex_digit(src[2 * i + 1])?;
            }
        } else {
            return Err(UuidError);
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
    type Err = UuidError;

    fn from_str(src: &str) -> Result<Self, UuidError> {
        Self::parse_str(src)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_lower(self.as_bytes(), buf.as_mut_ptr());
        }
        serializer.serialize_str(unsafe { std::str::from_utf8_unchecked(&buf[..36]) })
    }
}

//forward_deserialize_to_from_str!(Uuid);
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Uuid {
    fn deserialize<D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        struct ForwardToStrVisitor;

        impl<'a> serde::de::Visitor<'a> for ForwardToStrVisitor {
            type Value = Uuid;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid uuid as a string")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Uuid, E> {
                v.parse::<Uuid>()
                    .map_err(|err| Error::custom(err.to_string()))
            }
        }

        deserializer.deserialize_str(ForwardToStrVisitor)
    }
}

#[test]
fn test_uuid() {
    let uuid = Uuid::generate();
    let ser: String = uuid.to_string();
    let de: Uuid = ser.parse().expect("failed to parse uuid");
    assert_eq!(uuid, de);
}

#[cfg(feature = "serde")]
#[test]
fn test_uuid_serde() {
    let uuid = Uuid::generate();
    let ser: String = serde_json::to_string(&uuid).expect("failed to serialize uuid");
    let de: Uuid = serde_json::from_str(&ser).expect("failed to deserialize uuid");
    assert_eq!(uuid, de);
}
