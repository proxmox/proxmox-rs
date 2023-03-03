//! Property string deserialization.

use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
use std::ops::Range;

use serde::de::{self, IntoDeserializer};

use crate::schema::{self, ArraySchema, Schema};

mod cow3;
mod extract;
mod no_schema;

pub mod verify;

pub use extract::ExtractValueDeserializer;

use cow3::{str_slice_to_range, Cow3};

// Used to disable calling `check_constraints` on a `StringSchema` if it is being deserialized
// for a `PropertyString`, which performs its own checking.
thread_local! {
    static IN_PROPERTY_STRING: Cell<bool> = Cell::new(false);
}

pub(crate) struct InPropertyStringGuard;

pub(crate) fn set_in_property_string() -> InPropertyStringGuard {
    IN_PROPERTY_STRING.with(|v| v.set(true));
    InPropertyStringGuard
}

impl Drop for InPropertyStringGuard {
    fn drop(&mut self) {
        IN_PROPERTY_STRING.with(|v| v.set(false));
    }
}

#[derive(Debug)]
pub struct Error(Cow<'static, str>);

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error {
    pub(crate) fn msg<T: Into<Cow<'static, str>>>(msg: T) -> Self {
        Self(msg.into())
    }

    fn invalid<T: fmt::Display>(msg: T) -> Self {
        Self::msg(format!("schema validation failed: {}", msg))
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string().into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string().into())
    }
}

impl From<fmt::Error> for Error {
    fn from(err: fmt::Error) -> Self {
        Self::msg(err.to_string())
    }
}

/// Deserializer for parts a part of a property string given a schema.
pub struct SchemaDeserializer<'de, 'i> {
    input: Cow3<'de, 'i, str>,
    schema: &'static Schema,
}

impl<'de, 'i> SchemaDeserializer<'de, 'i> {
    pub fn new_cow(input: Cow3<'de, 'i, str>, schema: &'static Schema) -> Self {
        Self { input, schema }
    }

    pub fn new<T>(input: T, schema: &'static Schema) -> Self
    where
        T: Into<Cow<'de, str>>,
    {
        Self {
            input: Cow3::from_original(input.into()),
            schema,
        }
    }

    fn deserialize_str<V>(
        self,
        visitor: V,
        schema: &'static schema::StringSchema,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        if !IN_PROPERTY_STRING.with(|v| v.get()) {
            schema
                .check_constraints(&self.input)
                .map_err(|err| Error::invalid(err))?;
        }
        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn deserialize_property_string<V>(
        self,
        visitor: V,
        schema: &'static Schema,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match schema {
            Schema::Object(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            _ => Err(Error::msg(
                "non-object-like schema in ApiStringFormat::PropertyString while deserializing a property string",
            )),
        }
    }

    fn deserialize_array_string<V>(
        self,
        visitor: V,
        schema: &'static Schema,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match schema {
            Schema::Array(schema) => visitor.visit_seq(SeqAccess::new(self.input, schema)),
            _ => Err(Error::msg(
                "non-array schema in ApiStringFormat::PropertyString while deserializing an array",
            )),
        }
    }
}

impl<'de, 'i> de::Deserializer<'de> for SchemaDeserializer<'de, 'i> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Array(schema) => visitor.visit_seq(SeqAccess::new(self.input, schema)),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::Object(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::Null => Err(Error::msg("null")),
            Schema::Boolean(_) => visitor.visit_bool(
                schema::parse_boolean(&self.input)
                    .map_err(|_| Error::msg(format!("not a boolean: {:?}", self.input)))?,
            ),
            Schema::Integer(schema) => {
                // FIXME: isize vs explicit i64, needs fixing in schema check_constraints api
                let value: isize = self
                    .input
                    .parse()
                    .map_err(|_| Error::msg(format!("not an integer: {:?}", self.input)))?;

                schema
                    .check_constraints(value)
                    .map_err(|err| Error::invalid(err))?;

                let value: i64 = i64::try_from(value)
                    .map_err(|_| Error::invalid("isize did not fit into i64"))?;

                if let Ok(value) = u64::try_from(value) {
                    visitor.visit_u64(value)
                } else {
                    visitor.visit_i64(value)
                }
            }
            Schema::Number(schema) => {
                let value: f64 = self
                    .input
                    .parse()
                    .map_err(|_| Error::msg(format!("not a valid number: {:?}", self.input)))?;

                schema
                    .check_constraints(value)
                    .map_err(|err| Error::invalid(err))?;

                visitor.visit_f64(value)
            }
            Schema::String(schema) => {
                // If not requested differently, strings stay strings, otherwise deserializing to a
                // `Value` will get objects here instead of strings, which we do not expect
                // anywhere.
                self.deserialize_str(visitor, schema)
            }
        }
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

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::String(schema) => self.deserialize_str(visitor, schema),
            _ => Err(Error::msg(
                "tried to deserialize a string with a non-string-schema",
            )),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::String(schema) => self.deserialize_str(visitor, schema),
            _ => Err(Error::msg(
                "tried to deserialize a string with a non-string-schema",
            )),
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

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Object(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.deserialize_property_string(visitor, schema)
                }
                _ => Err(Error::msg(format!(
                    "cannot deserialize struct '{}' with a string schema",
                    name
                ))),
            },
            _ => Err(Error::msg(format!(
                "cannot deserialize struct '{}' with non-object schema",
                name,
            ))),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Object(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::new_cow(self.input, schema)),
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.deserialize_property_string(visitor, schema)
                }
                _ => Err(Error::msg(format!(
                    "cannot deserialize map with a string schema",
                ))),
            },
            _ => Err(Error::msg(format!(
                "cannot deserialize map with non-object schema",
            ))),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Array(schema) => visitor.visit_seq(SeqAccess::new(self.input, schema)),
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.deserialize_array_string(visitor, schema)
                }
                _ => Err(Error::msg("cannot deserialize array with a string schema")),
            },
            _ => Err(Error::msg(
                "cannot deserialize array with non-object schema",
            )),
        }
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::String(_) => visitor.visit_enum(self.input.into_deserializer()),
            _ => Err(Error::msg(format!(
                "cannot deserialize enum '{}' with non-string schema",
                name,
            ))),
        }
    }

    serde::forward_to_deserialize_any! {
            i8 i16 i32 i64
            u8 u16 u32 u64
            f32 f64
            bool
            char
            bytes byte_buf
            unit unit_struct
            tuple tuple_struct
            identifier
            ignored_any
    }
}

fn next_str_entry(input: &str, at: &mut usize, has_null: bool) -> Option<Range<usize>> {
    while *at != input.len() {
        let begin = *at;

        let part = &input[*at..];

        let part_end = if has_null {
            part.find('\0')
        } else {
            part.find(|c: char| c == ',' || c == ';' || char::is_ascii_whitespace(&c))
        };

        let end = match part_end {
            None => {
                *at = input.len();
                input.len()
            }
            Some(rel_end) => {
                *at += rel_end + 1;
                begin + rel_end
            }
        };

        if input[..end].is_empty() {
            continue;
        }

        return Some(begin..end);
    }

    None
}

/// Parse an array with a schema.
///
/// Provides both `SeqAccess` and `Deserializer` implementations.
pub struct SeqAccess<'o, 'i, 's> {
    schema: &'s ArraySchema,
    was_empty: bool,
    input: Cow3<'o, 'i, str>,
    has_null: bool,
    at: usize,
    count: usize,
}

impl<'o, 'i, 's> SeqAccess<'o, 'i, 's> {
    pub fn new(input: Cow3<'o, 'i, str>, schema: &'s ArraySchema) -> Self {
        Self {
            schema,
            was_empty: input.is_empty(),
            has_null: input.contains('\0'),
            input,
            at: 0,
            count: 0,
        }
    }
}

impl<'de, 'i, 's> de::SeqAccess<'de> for SeqAccess<'de, 'i, 's> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.was_empty {
            return Ok(None);
        }

        while let Some(el_range) = next_str_entry(&self.input, &mut self.at, self.has_null) {
            if el_range.is_empty() {
                continue;
            }

            if let Some(max) = self.schema.max_length {
                if self.count == max {
                    return Err(Error::msg("too many elements"));
                }
            }

            self.count += 1;

            return seed
                .deserialize(SchemaDeserializer::new_cow(
                    self.input.slice(el_range),
                    self.schema.items,
                ))
                .map(Some);
        }

        if let Some(min) = self.schema.min_length {
            if self.count < min {
                return Err(Error::msg("not enough elements"));
            }
        }

        Ok(None)
    }
}

impl<'de, 'i, 's> de::Deserializer<'de> for SeqAccess<'de, 'i, 's> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        if self.was_empty {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    serde::forward_to_deserialize_any! {
        i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
        bool char str string
        bytes byte_buf
        unit unit_struct
        newtype_struct
        tuple tuple_struct
        enum map seq
        struct
        identifier ignored_any
    }
}

/// Provides serde's `MapAccess` for parsing a property string.
pub struct MapAccess<'de, 'i> {
    // The property string iterator and quoted string handler.
    input: Cow3<'de, 'i, str>,
    input_at: usize, // for when using `Cow3::Owned`.

    /// As a `Deserializer` we want to be able to handle `deserialize_option` and need to know
    /// whether this was an empty string.
    was_empty: bool,

    /// The schema used to verify the contents and distinguish between structs and property
    /// strings.
    schema: &'static dyn schema::ObjectSchemaType,

    /// The current next value's key, value and schema (if available).
    value: Option<(Cow<'de, str>, Cow<'de, str>, Option<&'static Schema>)>,
}

impl<'de, 'i> MapAccess<'de, 'i> {
    pub fn new<S: schema::ObjectSchemaType>(input: &'de str, schema: &'static S) -> Self {
        Self {
            was_empty: input.is_empty(),
            input: Cow3::Original(input),
            schema,
            input_at: 0,
            value: None,
        }
    }

    pub fn new_cow<S: schema::ObjectSchemaType>(
        input: Cow3<'de, 'i, str>,
        schema: &'static S,
    ) -> Self {
        Self {
            was_empty: input.is_empty(),
            input,
            schema,
            input_at: 0,
            value: None,
        }
    }

    pub fn new_intermediate<S: schema::ObjectSchemaType>(
        input: &'i str,
        schema: &'static S,
    ) -> Self {
        Self {
            was_empty: input.is_empty(),
            input: Cow3::Intermediate(input),
            schema,
            input_at: 0,
            value: None,
        }
    }
}

impl<'de, 'i> de::MapAccess<'de> for MapAccess<'de, 'i> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        use crate::property_string::next_property;

        if self.was_empty {
            // shortcut
            return Ok(None);
        }

        let (key, value, rem) = match next_property(&self.input[self.input_at..]) {
            None => return Ok(None),
            Some(entry) => entry?,
        };

        if rem.is_empty() {
            self.input_at = self.input.len();
        } else {
            let ofs = unsafe { rem.as_ptr().offset_from(self.input.as_ptr()) };
            if ofs < 0 || (ofs as usize) > self.input.len() {
                // 'rem' is either an empty string (rem.is_empty() is true), or a valid offset into
                // the input string...
                panic!("unexpected remainder in next_property");
            }
            self.input_at = ofs as usize;
        }

        let value = match value {
            Cow::Owned(value) => Cow::Owned(value),
            Cow::Borrowed(value) => match str_slice_to_range(&self.input, value) {
                None => Cow::Owned(value.to_string()),
                Some(range) => match &self.input {
                    Cow3::Original(orig) => Cow::Borrowed(&orig[range]),
                    _ => Cow::Owned(value.to_string()),
                },
            },
        };

        let (key, schema) = match key {
            Some(key) => {
                let schema = self.schema.lookup(&key);
                let key = match str_slice_to_range(&self.input, key) {
                    None => Cow::Owned(key.to_string()),
                    Some(range) => match &self.input {
                        Cow3::Original(orig) => Cow::Borrowed(&orig[range]),
                        _ => Cow::Owned(key.to_string()),
                    },
                };
                (key, schema)
            }
            None => match self.schema.default_key() {
                Some(key) => {
                    let schema = self
                        .schema
                        .lookup(key)
                        .ok_or(Error::msg("bad default key"))?;
                    (Cow::Borrowed(key), Some(schema))
                }
                None => return Err(Error::msg("missing key")),
            },
        };
        let schema = schema.map(|(_optional, schema)| schema);

        let out = match &key {
            Cow::Borrowed(key) => {
                seed.deserialize(de::value::BorrowedStrDeserializer::<'de, Error>::new(key))?
            }
            Cow::Owned(key) => {
                seed.deserialize(IntoDeserializer::<Error>::into_deserializer(key.as_str()))?
            }
        };

        self.value = Some((key, value, schema));

        Ok(Some(out))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let (key, input, schema) = self.value.take().ok_or(Error::msg("bad map access"))?;

        if let Some(schema) = schema {
            seed.deserialize(SchemaDeserializer::new(input, schema))
        } else {
            if !verify::is_verifying() && !self.schema.additional_properties() {
                return Err(Error::msg(format!("unknown key {:?}", key.as_ref())));
            }

            // additional properties are treated as strings...
            let deserializer = no_schema::NoSchemaDeserializer::new(input);
            seed.deserialize(deserializer)
        }
    }
}
