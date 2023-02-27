//! Property string parsing
//! This may at some point also get a proper direkt `Serializer`/`Deserializer` for property
//! strings.

use std::borrow::Cow;
use std::fmt;
use std::mem;

use serde::{Deserialize, Serialize};

use crate::de::Error;
use crate::schema::ApiType;

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
        Some(match next_property(self.data)? {
            Ok((key, value, data)) => {
                self.data = data;
                Ok((key, value))
            }
            Err(err) => Err(err),
        })
    }
}

/// Returns an optional key, its value, and the remainder of `data`.
pub(crate) fn next_property(
    mut data: &str,
) -> Option<Result<(Option<&str>, Cow<str>, &str), Error>> {
    if data.is_empty() {
        return None;
    }

    let key = if data.starts_with('"') {
        // value without key and quoted
        None
    } else {
        let key = match data.find([',', '=']) {
            Some(pos) if data.as_bytes()[pos] == b',' => None,
            Some(pos) => Some(ascii_split_off(&mut data, pos)),
            None => None,
        };

        if !data.starts_with('"') {
            let value = match data.find(',') {
                Some(pos) => ascii_split_off(&mut data, pos),
                None => mem::take(&mut data),
            };
            return Some(Ok((key, Cow::Borrowed(value), data)));
        }

        key
    };

    let value = match parse_quoted_string(&mut data) {
        Ok(value) => value,
        Err(err) => return Some(Err(err)),
    };

    if !data.is_empty() {
        if data.starts_with(',') {
            data = &data[1..];
        } else {
            return Some(Err(Error::msg("garbage after quoted string")));
        }
    }

    Some(Ok((key, value, data)))
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
        return Err(Error::msg("not a quoted string"));
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
        return Err(Error::msg("unexpected end of string"));
    }

    // we're now at the first backslash, don't include it in the output:
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[1..i]);
    i += 1;
    let mut was_backslash = true;
    loop {
        if i == data.len() {
            return Err(Error::msg("unexpected end of string"));
        }

        match (data[i], mem::replace(&mut was_backslash, false)) {
            (b'"', false) => {
                i += 1;
                break;
            }
            (b'"', true) => out.push(b'"'),
            (b'\\', true) => out.push(b'\\'),
            (b'n', true) => out.push(b'\n'),
            (_, true) => return Err(Error::msg("unsupported escape sequence")),
            (b'\\', false) => was_backslash = true,
            (ch, false) => out.push(ch),
        }
        i += 1;
    }

    // unsafe: we know we're at an ascii boundary
    *data_out = unsafe { std::str::from_utf8_unchecked(&data[i..]) };
    Ok(Cow::Owned(unsafe { String::from_utf8_unchecked(out) }))
}

/// Counterpart to `parse_quoted_string`, only supporting the above-supported escape sequences.
/// Returns `true`
pub(crate) fn quote<T: fmt::Write>(s: &str, out: &mut T) -> fmt::Result {
    for b in s.chars() {
        match b {
            '"' => out.write_str(r#"\""#)?,
            '\\' => out.write_str(r#"\\"#)?,
            '\n' => out.write_str(r#"\n"#)?,
            b => out.write_char(b)?,
        }
    }
    Ok(())
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

    assert!(PropertyIterator::new(r#"key="open \\ value"#)
        .next()
        .unwrap()
        .is_err());
    assert!(PropertyIterator::new(r#"key="open \\ value\""#)
        .next()
        .unwrap()
        .is_err());
}

/// A wrapper for a de/serializable type which is stored as a property string.
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct PropertyString<T>(T);

impl<T> PropertyString<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Serialize + ApiType> PropertyString<T> {
    pub fn to_property_string(&self) -> Result<String, Error> {
        print(&self.0)
    }
}

impl<T> From<T> for PropertyString<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> std::ops::Deref for PropertyString<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for PropertyString<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> AsRef<T> for PropertyString<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for PropertyString<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'de, T> Deserialize<'de> for PropertyString<T>
where
    T: Deserialize<'de> + ApiType,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::marker::PhantomData;

        struct V<T>(PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for V<T>
        where
            T: Deserialize<'de> + ApiType,
        {
            type Value = T;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a property string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_string(s.to_string())
            }

            fn visit_string<E>(self, s: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                T::deserialize(crate::de::SchemaDeserializer::new(s, &T::API_SCHEMA))
                    .map_err(|err| E::custom(err.to_string()))
            }

            fn visit_borrowed_str<E>(self, s: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                T::deserialize(crate::de::SchemaDeserializer::new(s, &T::API_SCHEMA))
                    .map_err(|err| E::custom(err.to_string()))
            }
        }

        deserializer.deserialize_string(V(PhantomData)).map(Self)
    }
}

impl<T> std::str::FromStr for PropertyString<T>
where
    T: ApiType + for<'de> Deserialize<'de>,
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        T::deserialize(crate::de::SchemaDeserializer::new(s, &T::API_SCHEMA)).map(Self)
    }
}

impl<T> Serialize for PropertyString<T>
where
    T: Serialize + ApiType,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        serializer.serialize_str(&print(&self.0).map_err(S::Error::custom)?)
    }
}

/// Serialize a value as a property string.
pub fn print<T: Serialize + ApiType>(value: &T) -> Result<String, Error> {
    value.serialize(crate::ser::PropertyStringSerializer::new(
        String::new(),
        &T::API_SCHEMA,
    ))
}

/// Deserialize a value from a property string.
pub fn parse<T: ApiType>(value: &str) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de>,
{
    parse_with_schema(value, &T::API_SCHEMA)
}

/// Deserialize a value from a property string.
pub fn parse_with_schema<T>(value: &str, schema: &'static crate::Schema) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de>,
{
    T::deserialize(crate::de::SchemaDeserializer::new(value, schema))
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::schema::*;

    impl ApiType for Object {
        const API_SCHEMA: Schema = ObjectSchema::new(
            "An object",
            &[
                // MUST BE SORTED
                ("count", false, &IntegerSchema::new("name").schema()),
                ("name", false, &StringSchema::new("name").schema()),
                ("nested", true, &Nested::API_SCHEMA),
                (
                    "optional",
                    true,
                    &BooleanSchema::new("an optional boolean").schema(),
                ),
            ],
        )
        .schema();
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub struct Object {
        name: String,
        count: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        optional: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        nested: Option<Nested>,
    }

    impl ApiType for Nested {
        const API_SCHEMA: Schema = ObjectSchema::new(
            "An object",
            &[
                // MUST BE SORTED
                (
                    "count",
                    true,
                    &ArraySchema::new("count", &IntegerSchema::new("a value").schema()).schema(),
                ),
                ("name", false, &StringSchema::new("name").schema()),
                ("third", true, &Third::API_SCHEMA),
            ],
        )
        .schema();
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub struct Nested {
        name: String,

        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        count: Vec<u32>,

        #[serde(skip_serializing_if = "Option::is_none")]
        third: Option<Third>,
    }

    impl ApiType for Third {
        const API_SCHEMA: Schema = ObjectSchema::new(
            "An object",
            &[
                // MUST BE SORTED
                ("count", false, &IntegerSchema::new("name").schema()),
                ("name", false, &StringSchema::new("name").schema()),
            ],
        )
        .schema();
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub struct Third {
        name: String,
        count: u32,
    }

    #[test]
    fn test() -> Result<(), super::Error> {
        let obj = Object {
            name: "One \"Mo\\re\" Name".to_string(),
            count: 12,
            optional: Some(true),
            nested: Some(Nested {
                name: "a \"bobby\"".to_string(),
                count: vec![22, 23, 24],
                third: Some(Third {
                    name: "oh\\backslash".to_string(),
                    count: 37,
                }),
            }),
        };

        let s = super::print(&obj)?;

        let deserialized: Object = super::parse(&s).expect("failed to parse property string");

        assert_eq!(obj, deserialized, "deserialized does not equal original");

        Ok(())
    }
}
