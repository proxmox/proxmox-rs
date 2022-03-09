//! Property string parsing
//! This may at some point also get a proper direkt `Serializer`/`Deserializer` for property
//! strings.

use std::borrow::Cow;
use std::mem;

use anyhow::{bail, format_err, Error};

/// Iterate over the `key=value` pairs of a property string.
///
/// Note, that the `key` may be optional when the schema defines a "default" key.
/// If the value does not require stripping backslash escapes it will be borrowed, otherwise an
/// owned `String` will be returned.
pub struct PropertyIterator<'a> {
    data: &'a str,
}

impl<'a> PropertyIterator<'a> {
    pub fn new(data: &'a str) -> Self {
        Self { data }
    }
}

impl<'a> Iterator for PropertyIterator<'a> {
    type Item = Result<(Option<&'a str>, Cow<'a, str>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }

        let key = if self.data.starts_with('"') {
            // value without key and quoted
            None
        } else {
            let key = match self.data.find(&[',', '=']) {
                Some(pos) if self.data.as_bytes()[pos] == b',' => None,
                Some(pos) => Some(ascii_split_off(&mut self.data, pos)),
                None => None,
            };

            if !self.data.starts_with('"') {
                let value = match self.data.find(',') {
                    Some(pos) => ascii_split_off(&mut self.data, pos),
                    None => mem::take(&mut self.data),
                };
                return Some(Ok((key, Cow::Borrowed(value))));
            }

            key
        };

        let value = match parse_quoted_string(&mut self.data) {
            Ok(value) => value,
            Err(err) => return Some(Err(err)),
        };

        if !self.data.is_empty() {
            if self.data.starts_with(',') {
                self.data = &self.data[1..];
            } else {
                return Some(Err(format_err!("garbage after quoted string")));
            }
        }

        Some(Ok((key, value)))
    }
}

impl<'a> std::iter::FusedIterator for PropertyIterator<'a> {}

/// Parse a quoted string and move `data` to after the closing quote.
///
/// The string must start with a double quote.
///
/// This allows `\"`, `\\` and `\n` as escape sequences.
///
/// If no escape sequence is found, `Cow::Borrowed` is returned, otherwise an owned `String` is
/// returned.
fn parse_quoted_string<'s>(data: &'_ mut &'s str) -> Result<Cow<'s, str>, Error> {
    let data_out = data;
    let data = data_out.as_bytes();

    if data[0] != b'"' {
        bail!("not a quoted string");
    }

    let mut i = 1;
    while i != data.len() {
        if data[i] == b'"' {
            // unsafe: we're at an ascii boundary and index [0] is an ascii quote
            let value = unsafe { std::str::from_utf8_unchecked(&data[1..i]) };
            *data_out = unsafe { std::str::from_utf8_unchecked(&data[(i + 1)..]) };
            return Ok(Cow::Borrowed(value));
        } else if data[i] == b'\\' {
            // we cannot borrow
            break;
        }
        i += 1;
    }
    if i == data.len() {
        // reached the end before reaching a quote
        bail!("unexpected end of string");
    }

    // we're now at the first backslash, don't include it in the output:
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[1..i]);
    i += 1;
    let mut was_backslash = true;
    loop {
        if i == data.len() {
            bail!("unexpected end of string");
        }

        match (data[i], mem::replace(&mut was_backslash, false)) {
            (b'"', false) => {
                i += 1;
                break;
            }
            (b'"', true) => out.push(b'"'),
            (b'\\', true) => out.push(b'\\'),
            (b'n', true) => out.push(b'\n'),
            (_, true) => bail!("unsupported escape sequence"),
            (b'\\', false) => was_backslash = true,
            (ch, false) => out.push(ch),
        }
        i += 1;
    }

    // unsafe: we know we're at an ascii boundary
    *data_out = unsafe { std::str::from_utf8_unchecked(&data[i..]) };
    Ok(Cow::Owned(unsafe { String::from_utf8_unchecked(out) }))
}

/// Like `str::split_at` but with assumes `mid` points to an ASCII character and the 2nd slice
/// *excludes* `mid`.
fn ascii_split_around(s: &str, mid: usize) -> (&str, &str) {
    (&s[..mid], &s[(mid + 1)..])
}

/// Split "off" the first `mid` bytes of `s`, advancing it to `mid + 1` (assuming `mid` points to
/// an ASCII character!).
fn ascii_split_off<'a, 's>(s: &'a mut &'s str, mid: usize) -> &'s str {
    let (a, b) = ascii_split_around(s, mid);
    *s = b;
    a
}

#[test]
fn iterate_over_property_string() {
    let data = r#""value w/o key",fst=v1,sec="with = in it",third=3,"and \" and '\\'""#;
    let mut iter = PropertyIterator::new(data).map(|entry| entry.unwrap());
    assert_eq!(iter.next().unwrap(), (None, Cow::Borrowed("value w/o key")));
    assert_eq!(iter.next().unwrap(), (Some("fst"), Cow::Borrowed("v1")));
    assert_eq!(
        iter.next().unwrap(),
        (Some("sec"), Cow::Borrowed("with = in it"))
    );
    assert_eq!(iter.next().unwrap(), (Some("third"), Cow::Borrowed("3")));
    assert_eq!(
        iter.next().unwrap(),
        (None, Cow::Borrowed(r#"and " and '\'"#))
    );
    assert!(iter.next().is_none());

    assert!(PropertyIterator::new(r#"key="open \\ value"#).next().unwrap().is_err());
    assert!(PropertyIterator::new(r#"key="open \\ value\""#).next().unwrap().is_err());
}
