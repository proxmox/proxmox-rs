//! Crate for Debian versions, with the main use case being to compare two versions as per
//! [deb-version], while trying to use a minimal dependencies, not panicking and being robust while
//! staying ergonomic enough.
//!
//! The implementation should be efficient enough for most use cases, but [Version] does use the
//! (allocated) String type for storing the upstream and revision parts of the version. This is done
//! as a trade-off between more convenience for a bit less efficiency, but version strings are quite
//! small and so this won't really matter.
//!
//! Very lightly inspired by [debversion-rs], but we have a much narrower focus and this is no
//! reimplementation. Some specific test cases may have been taken over 1:1, but our implementation
//! does not have the exact same behavior in all edge cases (like no-epoch and explicit zero epoch
//! is the same for us, as deb-version declares it that way), and we currently also do not support
//! changing the version, like incrementing it.
//!
//! [deb-version]: https://manpages.debian.org/stable/dpkg-dev/deb-version.7.en.html
//! [debversion-rs]: https://github.com/jelmer/debversion-rs

use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Errors for parsing a Debian version string.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("version string cannot be empty")]
    Empty,
    #[error("upstream version cannot be empty")]
    MissingUpstream,
    #[error("invalid epoch: {0}")]
    InvalidEpoch(#[source] std::num::ParseIntError),
}

/// A parsed Debian package version number.
///
/// Contains an optional epoch (defaults to 0), an upstream version, and an optional Debian
/// revision. If the latter is none, it means that this is a native version.
///
/// # Examples
///
/// ```
/// use proxmox_deb_version::Version;
/// use std::str::FromStr;
///
/// let v1 = Version::from_str("1:2.0.3-1").unwrap();
/// let v2 = Version::from_str("1:2.0.3-2").unwrap();
///
/// assert!(v1 < v2);
/// assert_eq!(v1.epoch(), 1);
/// assert_eq!(v1.upstream(), "2.0.3");
/// assert_eq!(v1.revision(), Some("1"));
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct Version {
    epoch: u32,
    upstream: String,
    revision: Option<String>,
}

impl Version {
    /// Creates a new Version with epoch 0 (the most common case for us).
    pub fn new(upstream: &str, revision: Option<&str>) -> Self {
        Self {
            epoch: 0,
            upstream: upstream.to_string(),
            revision: revision.map(String::from),
        }
    }

    /// Creates a new Version instance including an epoch.
    pub fn new_with_epoch(epoch: u32, upstream: &str, revision: Option<&str>) -> Self {
        Self {
            epoch,
            upstream: upstream.to_string(),
            revision: revision.map(String::from),
        }
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    pub fn upstream(&self) -> &str {
        &self.upstream
    }

    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
}

impl std::str::FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseError::Empty);
        }

        // split epoch, if present
        let (epoch, rest) = match s.split_once(':') {
            Some((e, r)) => (e.parse().map_err(ParseError::InvalidEpoch)?, r),
            None => (0, s),
        };

        // split upstream and revision at the *last* hyphen
        let (upstream, revision) = match rest.rfind('-') {
            Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
            None => (rest, None),
        };

        if upstream.is_empty() {
            return Err(ParseError::MissingUpstream); // always required
        }

        Ok(Version::new_with_epoch(epoch, upstream, revision))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.epoch > 0 {
            write!(f, "{}:", self.epoch)?;
        }
        write!(f, "{}", self.upstream)?;
        if let Some(rev) = &self.revision {
            write!(f, "-{rev}")?;
        }
        Ok(())
    }
}

impl From<Version> for String {
    fn from(v: Version) -> Self {
        v.to_string()
    }
}

impl TryFrom<String> for Version {
    type Error = ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl<'a> TryFrom<&'a str> for Version {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. compare epoch
        self.epoch
            .cmp(&other.epoch)
            // 2. compare upstream
            .then_with(|| debian_cmp_str(&self.upstream, &other.upstream))
            // 3. Compare revision
            .then_with(|| match (&self.revision, &other.revision) {
                (Some(a), Some(b)) => debian_cmp_str(a, b),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Implements the Debian version sorting algorithm as per the deb-version manpage.
fn debian_cmp_str(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    while a_chars.peek().is_some() || b_chars.peek().is_some() {
        // lexical comparison of non-digit chunk
        let diff = compare_non_digits(&mut a_chars, &mut b_chars);
        if diff != Ordering::Equal {
            return diff;
        }

        // numeric comparison of digit chunk
        let diff = compare_digits(&mut a_chars, &mut b_chars);
        if diff != Ordering::Equal {
            return diff;
        }
    }
    Ordering::Equal
}

fn compare_digits(a: &mut Peekable<Chars>, b: &mut Peekable<Chars>) -> Ordering {
    parse_leading_digits(a).cmp(&parse_leading_digits(b))
}

fn parse_leading_digits(chars: &mut Peekable<Chars>) -> u64 {
    let mut num: u64 = 0;
    while let Some(&c) = chars.peek() {
        if let Some(d) = c.to_digit(10) {
            chars.next();
            // saturating add to cope with extremely long version numbers gracefully
            num = num.saturating_mul(10).saturating_add(d as u64);
        } else {
            break;
        }
    }
    num
}

fn compare_non_digits(a: &mut Peekable<Chars>, b: &mut Peekable<Chars>) -> Ordering {
    loop {
        let ca = a.next_if(|c| !c.is_ascii_digit());
        let cb = b.next_if(|c| !c.is_ascii_digit());

        match (ca, cb) {
            (None, None) => return Ordering::Equal,
            (Some(char_a), Some(char_b)) => {
                let ord = order_char(char_a).cmp(&order_char(char_b));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            // end of non-digit run behaves like '0' in ordering, except '~' which is -1.
            (Some(char_a), None) => return order_char(char_a).cmp(&0),
            (None, Some(char_b)) => return 0.cmp(&order_char(char_b)),
        }
    }
}

/// Custom character ordering: ~ < letters < end < everything else
fn order_char(c: char) -> i32 {
    if c == '~' {
        -1
    } else if c.is_ascii_alphabetic() {
        c as i32
    } else {
        c as i32 + 256
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        assert!(matches!("".parse::<Version>(), Err(ParseError::Empty)));
        assert!(matches!(
            "1:".parse::<Version>(),
            Err(ParseError::MissingUpstream)
        ));
        assert!(matches!(
            "a:1.0".parse::<Version>(),
            Err(ParseError::InvalidEpoch(_))
        ));
    }

    #[test]
    fn test_valid_parsing() {
        let v: Version = "2:1.2.3-1".parse().unwrap();
        assert_eq!(v.epoch, 2);
        assert_eq!(v.upstream, "1.2.3");
        assert_eq!(v.revision.as_deref(), Some("1"));

        let v: Version = "1.0".parse().unwrap();
        assert_eq!(v.epoch, 0);
        assert_eq!(v.revision, None);
    }

    #[test]
    fn test_comparisons() {
        let pairs = vec![
            ("1.0", "1.0", Ordering::Equal),
            ("1.0", "1.0.0", Ordering::Less), // 1.0 < 1.0.0 as longer version wins
            ("1.0", "1.1", Ordering::Less),
            ("1.0-1", "1.0-2", Ordering::Less),
            ("1.0", "1.0-1", Ordering::Less), // no-revsion < has-revision
            ("1:1.0", "1.0", Ordering::Greater),
            ("1.0~rc1", "1.0", Ordering::Less),
            ("1.0~~", "1.0~", Ordering::Less),
            ("1.0a", "1.0+", Ordering::Less), // letters < non-letters
        ];

        for (v1, v2, expected) in pairs {
            let ver1: Version = v1.parse().unwrap();
            let ver2: Version = v2.parse().unwrap();
            assert_eq!(ver1.cmp(&ver2), expected, "{v1} vs {v2}");
        }
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_roundtrip() {
        let v: Version = "1:2.3-4".parse().unwrap();

        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#""1:2.3-4""#);

        let v2: Version = serde_json::from_str(&json).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_invalid() {
        // Should fail gracefully with parse error
        let result: Result<Version, _> = serde_json::from_str(r#""""#);
        assert!(result.is_err());
    }
}
