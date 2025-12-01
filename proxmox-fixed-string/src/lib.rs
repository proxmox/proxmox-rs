use std::borrow::Borrow;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Error type used by constructors of [`FixedString`]
#[derive(Clone, Copy, Debug)]
pub struct TooLongError;

impl Error for TooLongError {}

impl fmt::Display for TooLongError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("string is longer than 31 characters")
    }
}

/// An immutable string type with a maximum size of 31 bytes.
///
/// After construction it is guaranteed that its contents are:
/// * valid utf-8
/// * not longer than 31 characters
///
/// FixedString is immutable, therefore it is sufficient to validate the invariants only at
/// construction time to guarantee that they will always hold during the lifecycle of the
/// struct.
#[derive(Clone, Copy)]
pub struct FixedString {
    buf: [u8; 31],
    len: u8,
}

impl FixedString {
    /// Creates a new FixedString instance from a str reference.
    ///
    /// # Errors
    /// This function will return an error if:
    /// * The passed string is longer than 31 bytes
    pub fn new(value: &str) -> Result<Self, TooLongError> {
        if value.len() > 31 {
            return Err(TooLongError);
        }

        let mut buf = [0; 31];
        buf[..value.len()].copy_from_slice(value.as_bytes());

        Ok(Self {
            buf,
            // SAFETY: self.len is at least 0 and at most 31, which fits into u8
            len: value.len() as u8,
        })
    }

    /// Returns a str reference to the stored data
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: self.buf must be a valid utf-8 string by construction
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// Returns a reference to the set bytes in the stored buffer
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: self.len >= 0 and self.len <= 31 by construction
        unsafe { self.buf.get_unchecked(..self.len as usize) }
    }
}

macro_rules! forward_impl_to_bytes {
    ($($trait:ident {$fn:ident -> $out:ty })+) => {
        $(
            impl $trait for FixedString {
                #[inline]
                fn $fn(&self, other: &Self) -> $out {
                    <[u8] as $trait>::$fn(self.as_bytes(), other.as_bytes())
                }
            }
        )+
    };
}

forward_impl_to_bytes! {
    PartialEq { eq -> bool }
    PartialOrd { partial_cmp -> Option<Ordering> }
    Ord { cmp -> Ordering }
}

macro_rules! forward_impl_to_str_bidir {
    ($($trait:ident {$fn:ident -> $out:ty })+) => {
        $(
            impl $trait<str> for FixedString {
                #[inline]
                fn $fn(&self, other: &str) -> $out {
                    <str as $trait>::$fn(self.as_str(), other)
                }
            }

            impl $trait<FixedString> for &str {
                #[inline]
                fn $fn(&self, other: &FixedString) -> $out {
                    <str as $trait>::$fn(self, other.as_str())
                }
            }
        )+
    };
}

forward_impl_to_str_bidir! {
    PartialEq { eq -> bool }
    PartialOrd { partial_cmp -> Option<Ordering> }
}

impl Eq for FixedString {}

impl fmt::Display for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for FixedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl Deref for FixedString {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for FixedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for FixedString {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Borrow<str> for FixedString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Hash for FixedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl TryFrom<String> for FixedString {
    type Error = TooLongError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        FixedString::new(value.as_str())
    }
}

impl FromStr for FixedString {
    type Err = TooLongError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        FixedString::new(value)
    }
}

impl TryFrom<&str> for FixedString {
    type Error = TooLongError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        FixedString::new(value)
    }
}

impl Serialize for FixedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FixedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FixedStringVisitor;

        impl<'de> serde::de::Visitor<'de> for FixedStringVisitor {
            type Value = FixedString;

            fn expecting(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string that is at most 31 bytes long")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.try_into().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(FixedStringVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_plain;

    #[test]
    fn test_construct() {
        let fixed_string = FixedString::new("").expect("empty string is valid");
        assert_eq!("", fixed_string);

        let fixed_string = FixedString::new("a").expect("valid string");
        assert_eq!("a", fixed_string);

        let fixed_string = FixedString::new("🌏🌏🌏🌏🌏").expect("valid string");
        assert_eq!("🌏🌏🌏🌏🌏", fixed_string);

        let fixed_string =
            FixedString::new("aaaaaaaaaaaaaaaaaaaaaaa").expect("31 characters are allowed");
        assert_eq!("aaaaaaaaaaaaaaaaaaaaaaa", fixed_string);

        FixedString::new(&"🌏".repeat(10)).expect_err("string too long");
        FixedString::new(&"a".repeat(32)).expect_err("string too long");
    }

    #[test]
    fn test_serialize_deserialize() {
        let valid_string = "aaaaaaaaaaaaaaaaaaaaaaa";

        let fixed_string: FixedString =
            serde_plain::from_str("aaaaaaaaaaaaaaaaaaaaaaa").expect("deserialization works");
        assert_eq!(valid_string, fixed_string);

        let serialized_string =
            serde_plain::to_string(&fixed_string).expect("can be serialized into a string");
        assert_eq!(valid_string, serialized_string);

        serde_plain::from_str::<FixedString>(&"a".repeat(32))
            .expect_err("cannot deserialize string that is too long");
    }

    #[test]
    fn test_ord() {
        let fixed_string = FixedString::new("abc").expect("valid string");

        assert!(fixed_string == fixed_string);
        assert!(fixed_string >= fixed_string);
        assert!(fixed_string <= fixed_string);

        assert!("ab" < fixed_string);
        assert!("abc" == fixed_string);
        assert!("abcd" > fixed_string);

        let larger_fixed_string = FixedString::new("abcde").expect("valid string");

        assert!(larger_fixed_string > fixed_string);
        assert!(fixed_string < larger_fixed_string);
    }
}
