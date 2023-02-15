//! When we have no schema we allow simple values and arrays.

use std::borrow::Cow;

use serde::de;

use super::cow3::Cow3;
use super::Error;

/// This can only deserialize strings and lists of strings and has no schema.
pub struct NoSchemaDeserializer<'de, 'i> {
    input: Cow3<'de, 'i, str>,
}

impl<'de, 'i> NoSchemaDeserializer<'de, 'i> {
    pub fn new<T>(input: T) -> Self
    where
        T: Into<Cow<'de, str>>,
    {
        Self {
            input: Cow3::from_original(input),
        }
    }
}

macro_rules! deserialize_num {
    ($( $name:ident : $visit:ident : $ty:ty : $error:literal, )*) => {$(
        fn $name<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            let value: $ty = self
                .input
                .parse()
                .map_err(|_| Error::msg(format!($error, self.input)))?;
            visitor.$visit(value)
        }
    )*}
}

impl<'de, 'i> de::Deserializer<'de> for NoSchemaDeserializer<'de, 'i> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        if self.input.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SimpleSeqAccess::new(self.input))
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SimpleSeqAccess::new(self.input))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SimpleSeqAccess::new(self.input))
    }

    deserialize_num! {
        deserialize_i8   : visit_i8   : i8   : "not an integer: {:?}",
        deserialize_u8   : visit_u8   : u8   : "not an integer: {:?}",
        deserialize_i16  : visit_i16  : i16  : "not an integer: {:?}",
        deserialize_u16  : visit_u16  : u16  : "not an integer: {:?}",
        deserialize_i32  : visit_i32  : i32  : "not an integer: {:?}",
        deserialize_u32  : visit_u32  : u32  : "not an integer: {:?}",
        deserialize_i64  : visit_i64  : i64  : "not an integer: {:?}",
        deserialize_u64  : visit_u64  : u64  : "not an integer: {:?}",
        deserialize_f32  : visit_f32  : f32  : "not a number: {:?}",
        deserialize_f64  : visit_f64  : f64  : "not a number: {:?}",
        deserialize_bool : visit_bool : bool : "not a boolean: {:?}",
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        let mut chars = self.input.chars();
        let ch = chars
            .next()
            .ok_or_else(|| Error::msg(format!("not a single character: {:?}", self.input)))?;
        if chars.next().is_some() {
            return Err(Error::msg(format!(
                "not a single character: {:?}",
                self.input
            )));
        }
        visitor.visit_char(ch)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_bytes(input.as_bytes()),
            Cow3::Intermediate(input) => visitor.visit_bytes(input.as_bytes()),
            Cow3::Owned(input) => visitor.visit_byte_buf(input.into_bytes()),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_bytes(input.as_bytes()),
            Cow3::Intermediate(input) => visitor.visit_bytes(input.as_bytes()),
            Cow3::Owned(input) => visitor.visit_byte_buf(input.into_bytes()),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        if self.input.is_empty() {
            visitor.visit_unit()
        } else {
            self.deserialize_string(visitor)
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        if self.input.is_empty() {
            visitor.visit_unit()
        } else {
            self.deserialize_string(visitor)
        }
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        use serde::de::IntoDeserializer;
        visitor.visit_enum(self.input.into_deserializer())
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }
}

/// Parse an array without a schema.
///
/// It may only contain simple values.
struct SimpleSeqAccess<'de, 'i> {
    input: Cow3<'de, 'i, str>,
    has_null: bool,
    at: usize,
}

impl<'de, 'i> SimpleSeqAccess<'de, 'i> {
    fn new(input: Cow3<'de, 'i, str>) -> Self {
        Self {
            has_null: input.contains('\0'),
            input,
            at: 0,
        }
    }
}

impl<'de, 'i> de::SeqAccess<'de> for SimpleSeqAccess<'de, 'i> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        while self.at != self.input.len() {
            let begin = self.at;

            let input = &self.input[self.at..];

            let end = if self.has_null {
                input.find('\0')
            } else {
                input.find(|c: char| c == ',' || c == ';' || char::is_ascii_whitespace(&c))
            };

            let end = match end {
                None => {
                    self.at = self.input.len();
                    input.len()
                }
                Some(pos) => {
                    self.at += pos + 1;
                    pos
                }
            };

            if input[..end].is_empty() {
                continue;
            }

            return seed
                .deserialize(NoSchemaDeserializer::new(match &self.input {
                    Cow3::Original(input) => Cow::Borrowed(&input[begin..end]),
                    Cow3::Intermediate(input) => Cow::Owned(input[begin..end].to_string()),
                    Cow3::Owned(input) => Cow::Owned(input[begin..end].to_string()),
                }))
                .map(Some);
        }

        Ok(None)
    }
}
