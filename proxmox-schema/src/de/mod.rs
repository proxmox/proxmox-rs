//! Property string deserialization.

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use anyhow::format_err;
use serde::de::{self, IntoDeserializer};

use crate::schema::{self, ArraySchema, ParameterError, Schema};

mod cow3;
mod extract;
mod no_schema;

pub mod verify;

pub use extract::ExtractValueDeserializer;

use cow3::{Cow3, str_slice_to_range};

pub use no_schema::{SplitList, split_list};

// Used to disable calling `check_constraints` on a `StringSchema` if it is being deserialized
// for a `PropertyString`, which performs its own checking.
thread_local! {
    static IN_PROPERTY_STRING: Cell<bool> = const { Cell::new(false) };
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

/// Verify a property string with its object or array schema.
pub fn verify(schema: &'static Schema, value: &str) -> Result<(), anyhow::Error> {
    let mut errors = ParameterError::new();
    match SchemaDeserializer::new(value, schema)
        .collect_errors(&mut errors)
        .de_any(verify::Visitor)
    {
        Ok(_) => (),
        Err(err) => {
            if !err.is_recoverable() || errors.is_empty() {
                errors.push(String::new(), format_err!("{err}"));
            }
        }
    };

    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 && errors.errors()[0].0.is_empty() {
        // This is a parameter with exactly 1 error at an empty path - let's unpack it:
        // Unwrap: We just checked for `len == 1`:
        Err(errors.into_inner().pop().unwrap().1)
    } else {
        Err(errors.into())
    }
}

pub struct Error {
    inner: ErrorImpl,
}

enum ErrorImpl {
    /// Generic and fatal error.
    Anyhow(anyhow::Error),

    /// Generic error message without anyhow's allocation.
    Msg(Cow<'static, str>),

    /// In validation mode multiple errors are collected into a [`ParameterError`].
    /// Non-fatal errors are *moved* there and this is returned instead to tell the visitor of a
    /// `SeqAccess` or `MapAccess` that it should continue iterating over the next keys.
    Recoverable,
}

impl std::error::Error for Error {}

impl fmt::Display for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Anyhow(err) => fmt::Display::fmt(err, f),
            Self::Msg(err) => fmt::Display::fmt(err, f),

            // This error type should only be used in conjunction with the verify module, so this
            // message should be generic enough but also recognizable as something which should not
            // happen, as no code path should end up printing this message to an end user.
            Self::Recoverable => f.write_str("schema validation failed [internal]"),
        }
    }
}

impl fmt::Debug for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Anyhow(err) => fmt::Debug::fmt(err, f),
            Self::Msg(err) => write!(f, "ErrorImpl::Msg({err:?})"),
            Self::Recoverable => f.write_str("ErrorImpl::Recoverable"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl Error {
    pub(crate) fn msg<T: Into<Cow<'static, str>>>(msg: T) -> Self {
        Self {
            inner: ErrorImpl::Msg(msg.into()),
        }
    }

    fn anyhow(err: anyhow::Error) -> Self {
        Self {
            inner: ErrorImpl::Anyhow(err),
        }
    }

    fn recoverable() -> Self {
        Self {
            inner: ErrorImpl::Recoverable,
        }
    }

    fn is_recoverable(&self) -> bool {
        matches!(self.inner, ErrorImpl::Recoverable)
    }

    /// If this is the recoverable, turn it into an `Ok`.
    fn recoverable_ok(self) -> Result<(), Self> {
        if self.is_recoverable() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::msg(msg.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::msg(error.to_string())
    }
}

impl From<fmt::Error> for Error {
    fn from(err: fmt::Error) -> Self {
        Self::msg(err.to_string())
    }
}

/// Helper trait for the validation mode to move errors into a [`ParameterError`] while returning
/// the `Recoverable` marker error and making these methods accessible for both the [`Error`] and
/// `Result` types.
trait MergeErrors: Sized {
    fn merge_to_with_key<K>(self, list: &mut ParameterError, key: K) -> Self
    where
        K: FnOnce() -> String;

    fn maybe_merge_to_with_key<K>(self, list: &mut Option<&mut ParameterError>, key: K) -> Self
    where
        K: FnOnce() -> String,
    {
        if let Some(list) = list {
            self.merge_to_with_key(list, key)
        } else {
            self
        }
    }

    fn maybe_merge_to(self, list: &mut Option<&mut ParameterError>) -> Self {
        self.maybe_merge_to_with_key(list, String::new)
    }
}

impl MergeErrors for Error {
    fn merge_to_with_key<K>(self, list: &mut ParameterError, key: K) -> Self
    where
        K: FnOnce() -> String,
    {
        if !self.is_recoverable() {
            list.push(key(), self.into());
            Self::recoverable()
        } else {
            self
        }
    }
}

impl<T> MergeErrors for Result<T, Error> {
    fn merge_to_with_key<K>(self, list: &mut ParameterError, key: K) -> Self
    where
        K: FnOnce() -> String,
    {
        self.map_err(move |err| err.merge_to_with_key(list, key))
    }
}

/// Deserializer for parts a part of a property string given a schema.
pub struct SchemaDeserializer<'de, 'i, 'err> {
    input: Cow3<'de, 'i, str>,
    schema: &'static Schema,
    errors: Option<&'err mut ParameterError>,
}

impl<'de, 'i> SchemaDeserializer<'de, 'i, 'static> {
    pub fn new_cow(input: Cow3<'de, 'i, str>, schema: &'static Schema) -> Self {
        Self {
            input,
            schema,
            errors: None,
        }
    }

    pub fn new<T>(input: T, schema: &'static Schema) -> Self
    where
        T: Into<Cow<'de, str>>,
    {
        Self {
            input: Cow3::from_original(input.into()),
            schema,
            errors: None,
        }
    }
}

impl<'de, 'i> SchemaDeserializer<'de, 'i, '_> {
    /// Collect errors into a `ParameterError` and return `Error::Recoverable` for recoverable
    /// errors. Non-recoverable cases still return regular errors.
    fn collect_errors<'err>(
        self,
        errors: &'err mut ParameterError,
    ) -> SchemaDeserializer<'de, 'i, 'err> {
        SchemaDeserializer {
            input: self.input,
            schema: self.schema,
            errors: Some(errors),
        }
    }

    fn de_str<V>(self, visitor: V, schema: &'static schema::StringSchema) -> Result<V::Value, Error>
    where
        V: VisitorOrVerify<'de>,
    {
        if !IN_PROPERTY_STRING.with(|v| v.get()) {
            if let Err(err) = schema.check_constraints(&self.input) {
                if let Some(errors) = self.errors
                    && err.is::<ParameterError>()
                {
                    errors.add_errors("", err);
                    return Err(Error::recoverable());
                }
                return Err(Error::anyhow(err));
            }
        }

        match self.input {
            Cow3::Original(input) => visitor.visit_borrowed_str(input),
            Cow3::Intermediate(input) => visitor.visit_str(input),
            Cow3::Owned(input) => visitor.visit_string(input),
        }
    }

    fn de_property_string<V>(self, visitor: V, schema: &'static Schema) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        let schema = schema.any_object()
            .ok_or_else(|| Error::msg(
                "non-object-like schema in ApiStringFormat::PropertyString while deserializing a property string",
            ))?;

        self.do_de_map(visitor, schema)
    }

    fn de_array_string<V>(self, visitor: V, schema: &'static Schema) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        let schema = schema.array().ok_or_else(|| {
            Error::msg(
                "non-array schema in ApiStringFormat::PropertyString while deserializing an array",
            )
        })?;
        visitor.visit_seq(SeqAccess::new(self.input, schema, self.errors))
    }

    fn do_de_map<V>(
        mut self,
        visitor: V,
        schema: &'static dyn schema::ObjectSchemaType,
    ) -> Result<V::Value, Error>
    where
        V: VisitorOrVerify<'de>,
    {
        let map = MapAccess::new_cow(self.input, schema, self.errors.as_deref_mut())?;
        visitor.visit_map(map).maybe_merge_to(&mut self.errors)
    }
}

/// We "specialize" the verifier's visitor so it can access the deserializer's Error type contents.
trait VisitorOrVerify<'de> {
    type Value;

    fn visit_bool(self, value: bool) -> Result<Self::Value, Error>;
    fn visit_i64(self, value: i64) -> Result<Self::Value, Error>;
    fn visit_u64(self, value: u64) -> Result<Self::Value, Error>;
    fn visit_f64(self, value: f64) -> Result<Self::Value, Error>;
    fn visit_str(self, value: &str) -> Result<Self::Value, Error>;
    fn visit_borrowed_str(self, value: &'de str) -> Result<Self::Value, Error>;
    fn visit_string(self, value: String) -> Result<Self::Value, Error>;
    fn visit_seq(self, access: SeqAccess<'de, '_, '_, '_>) -> Result<Self::Value, Error>;
    fn visit_map(self, access: MapAccess<'de, '_>) -> Result<Self::Value, Error>;
}

impl<'de, T> VisitorOrVerify<'de> for T
where
    T: de::Visitor<'de>,
{
    type Value = <T as de::Visitor<'de>>::Value;

    fn visit_bool(self, value: bool) -> Result<Self::Value, Error> {
        de::Visitor::visit_bool(self, value)
    }

    fn visit_i64(self, value: i64) -> Result<Self::Value, Error> {
        de::Visitor::visit_i64(self, value)
    }

    fn visit_u64(self, value: u64) -> Result<Self::Value, Error> {
        de::Visitor::visit_u64(self, value)
    }

    fn visit_f64(self, value: f64) -> Result<Self::Value, Error> {
        de::Visitor::visit_f64(self, value)
    }

    fn visit_str(self, value: &str) -> Result<Self::Value, Error> {
        de::Visitor::visit_str(self, value)
    }

    fn visit_borrowed_str(self, value: &'de str) -> Result<Self::Value, Error> {
        de::Visitor::visit_borrowed_str(self, value)
    }

    fn visit_string(self, value: String) -> Result<Self::Value, Error> {
        de::Visitor::visit_string(self, value)
    }

    fn visit_seq(self, access: SeqAccess<'de, '_, '_, '_>) -> Result<Self::Value, Error> {
        de::Visitor::visit_seq(self, access)
    }

    fn visit_map(self, access: MapAccess<'de, '_>) -> Result<Self::Value, Error> {
        de::Visitor::visit_map(self, access)
    }
}

trait DeserializeSeedOrVerify<'de> {
    type Value;

    fn deserialize(
        self,
        deserializer: SchemaDeserializer<'de, '_, '_>,
    ) -> Result<Self::Value, Error>;

    fn deserialize_no_schema(
        self,
        deserializer: no_schema::NoSchemaDeserializer<'de, '_>,
    ) -> Result<Self::Value, Error>;
}

impl<'de, T> DeserializeSeedOrVerify<'de> for T
where
    T: de::DeserializeSeed<'de>,
{
    type Value = <Self as de::DeserializeSeed<'de>>::Value;

    fn deserialize(
        self,
        deserializer: SchemaDeserializer<'de, '_, '_>,
    ) -> Result<Self::Value, Error> {
        de::DeserializeSeed::deserialize(self, deserializer)
    }

    fn deserialize_no_schema(
        self,
        deserializer: no_schema::NoSchemaDeserializer<'de, '_>,
    ) -> Result<Self::Value, Error> {
        de::DeserializeSeed::deserialize(self, deserializer)
    }
}

impl<'de, 'i> SchemaDeserializer<'de, 'i, '_> {
    fn de_any<V>(mut self, visitor: V) -> Result<V::Value, Error>
    where
        V: VisitorOrVerify<'de>,
    {
        match self.schema {
            Schema::Array(schema) => {
                visitor.visit_seq(SeqAccess::new(self.input, schema, self.errors))
            }
            Schema::Object(schema) => self.do_de_map(visitor, schema),
            Schema::AllOf(schema) => self.do_de_map(visitor, schema),
            Schema::OneOf(schema) => self.do_de_map(visitor, schema),
            Schema::Null => Err(Error::msg("null schema")),
            Schema::Boolean(_) => {
                let value = schema::parse_boolean(&self.input)
                    .map_err(|_| Error::msg(format!("not a boolean: {:?}", self.input)))
                    .maybe_merge_to(&mut self.errors)?;

                visitor.visit_bool(value).maybe_merge_to(&mut self.errors)
            }
            Schema::Integer(schema) => {
                let value: i64 = self
                    .input
                    .parse()
                    .map_err(|_| Error::msg(format!("not an integer: {:?}", self.input)))
                    .maybe_merge_to(&mut self.errors)?;

                schema
                    .check_constraints(value)
                    .map_err(Error::anyhow)
                    .maybe_merge_to(&mut self.errors)?;

                if let Ok(value) = u64::try_from(value) {
                    visitor.visit_u64(value)
                } else {
                    visitor.visit_i64(value)
                }
                .maybe_merge_to(&mut self.errors)
            }
            Schema::Number(schema) => {
                let value: f64 = self
                    .input
                    .parse()
                    .map_err(|_| Error::msg(format!("not a valid number: {:?}", self.input)))
                    .maybe_merge_to(&mut self.errors)?;

                schema
                    .check_constraints(value)
                    .map_err(Error::anyhow)
                    .maybe_merge_to(&mut self.errors)?;

                visitor.visit_f64(value).maybe_merge_to(&mut self.errors)
            }
            Schema::String(schema) => {
                // If not requested differently, strings stay strings, otherwise deserializing to a
                // `Value` will get objects here instead of strings, which we do not expect
                // anywhere.
                self.de_str(visitor, schema)
            }
        }
    }
}

impl<'de> de::Deserializer<'de> for SchemaDeserializer<'de, '_, '_> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        self.de_any(visitor)
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
            Schema::String(schema) => self.de_str(visitor, schema),
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
            Schema::String(schema) => self.de_str(visitor, schema),
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
            Schema::Object(schema) => self.do_de_map(visitor, schema),
            Schema::AllOf(schema) => self.do_de_map(visitor, schema),
            Schema::OneOf(schema) => self.do_de_map(visitor, schema),
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.de_property_string(visitor, schema)
                }
                _ => Err(Error::msg(format!(
                    "cannot deserialize struct '{name}' with a string schema"
                ))),
            },
            _ => Err(Error::msg(format!(
                "cannot deserialize struct '{name}' with non-object schema",
            ))),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Object(schema) => self.do_de_map(visitor, schema),
            Schema::AllOf(schema) => self.do_de_map(visitor, schema),
            Schema::OneOf(schema) => self.do_de_map(visitor, schema),
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.de_property_string(visitor, schema)
                }
                _ => Err(Error::msg("cannot deserialize map with a string schema")),
            },
            _ => Err(Error::msg("cannot deserialize map with non-object schema")),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        match self.schema {
            Schema::Array(schema) => {
                visitor.visit_seq(SeqAccess::new(self.input, schema, self.errors))
            }
            Schema::String(schema) => match schema.format {
                Some(schema::ApiStringFormat::PropertyString(schema)) => {
                    self.de_array_string(visitor, schema)
                }
                _ => Err(Error::msg("cannot deserialize array with a string schema")),
            },
            _ => Err(Error::msg("cannot deserialize array with non-array schema")),
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
                "cannot deserialize enum '{name}' with non-string schema",
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

pub(crate) fn next_str_entry(input: &str, at: &mut usize, has_null: bool) -> Option<Range<usize>> {
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

        if begin == end || input[..end].is_empty() {
            continue;
        }

        return Some(begin..end);
    }

    None
}

/// Parse an array with a schema.
///
/// Provides both `SeqAccess` and `Deserializer` implementations.
struct SeqAccess<'o, 'i, 's, 'err> {
    schema: &'s ArraySchema,
    was_empty: bool,
    input: Cow3<'o, 'i, str>,
    has_null: bool,
    at: usize,
    count: usize,
    errors: Option<&'err mut ParameterError>,

    // If `errors.is_some()` we check min and max only once.
    // Since we allow accumulating multiple errors, including errors from keys which don't fit
    // anymore.
    min_triggered: bool,
    max_triggered: bool,
}

impl<'o, 'i, 's, 'err> SeqAccess<'o, 'i, 's, 'err> {
    fn new(
        input: Cow3<'o, 'i, str>,
        schema: &'s ArraySchema,
        errors: Option<&'err mut ParameterError>,
    ) -> Self {
        Self {
            schema,
            was_empty: input.is_empty(),
            has_null: input.contains('\0'),
            input,
            at: 0,
            count: 0,
            errors,
            min_triggered: false,
            max_triggered: false,
        }
    }

    fn check_min(&mut self) -> Result<(), Error> {
        if self.min_triggered {
            return Ok(());
        }

        if let Some(min) = self.schema.min_length {
            if self.count < min {
                self.min_triggered = self.errors.is_some();
                return Err(
                    Error::msg(format!("array must contain at least {min} elements"))
                        .maybe_merge_to(&mut self.errors),
                );
            }
        }
        Ok(())
    }

    fn increment_and_check_max(&mut self) -> Result<(), Error> {
        self.count += 1;

        if self.max_triggered {
            return Ok(());
        }

        // strictly greater, since we increment before the check
        if let Some(max) = self.schema.max_length
            && self.count > max
        {
            self.max_triggered = self.errors.is_some();

            Error::msg(format!("array must contain at most {max} elements"))
                .maybe_merge_to(&mut self.errors)
                .recoverable_ok()
        } else {
            Ok(())
        }
    }
}

impl<'de> SeqAccess<'de, '_, '_, '_> {
    fn next<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: DeserializeSeedOrVerify<'de>,
    {
        if self.was_empty {
            self.check_min()?;
            return Ok(None);
        }

        if let Some(el_range) = next_str_entry(&self.input, &mut self.at, self.has_null) {
            self.increment_and_check_max()?;

            let deserializer =
                SchemaDeserializer::new_cow(self.input.slice(el_range), self.schema.items);
            if let Some(errors) = &mut self.errors {
                let mut inner_errors = ParameterError::new();
                let out = seed
                    .deserialize(deserializer.collect_errors(&mut inner_errors))
                    .map(Some);
                let index = self.count - 1;
                if !inner_errors.is_empty() {
                    errors.add_errors(&format!("[{index}]"), inner_errors.into());
                }
                return out.merge_to_with_key(errors, || format!("[{index}]"));
            } else {
                return seed.deserialize(deserializer).map(Some);
            }
        }

        self.check_min()?;
        Ok(None)
    }
}

impl<'de> de::SeqAccess<'de> for SeqAccess<'de, '_, '_, '_> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.next(seed)
    }
}

impl<'de> de::Deserializer<'de> for SeqAccess<'de, '_, '_, '_> {
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

/// This contains a pre-split property string (borrowing from the original data).
#[derive(Default)]
struct ParsedPropertyString<'de> {
    entries: HashMap<Cow<'de, str>, Cow<'de, str>>,
    entry_order: std::collections::VecDeque<Option<Cow<'de, str>>>,
    value_without_key: Option<Cow<'de, str>>,
}

impl crate::schema::WithTypeField for ParsedPropertyString<'_> {
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        func(self.entries.get(name).map(|v| &**v))
    }
}

/// If a string was borrowed from a `Cow3::Original`, map it to a `Cow::Borrowed`.
/// Otherwise turn it into a `Cow::Owned`.
fn propagate_cow<'de>(input: &Cow3<'de, '_, str>, substr: Cow<'_, str>) -> Cow<'de, str> {
    match substr {
        Cow::Owned(s) => Cow::Owned(s),
        Cow::Borrowed(substr) => match str_slice_to_range(input, substr) {
            None => Cow::Owned(substr.to_string()),
            Some(range) => match &input {
                Cow3::Original(orig) => Cow::Borrowed(&orig[range]),
                _ => Cow::Owned(substr.to_string()),
            },
        },
    }
}

impl<'de> ParsedPropertyString<'de> {
    fn split_cow<'i>(
        input: Cow3<'de, 'i, str>,
        mut collect_errors: Option<&mut ParameterError>,
    ) -> Result<Self, Error> {
        let mut this = Self::default();

        let mut remainder = &input[..];
        while let Some(next) = crate::property_string::next_property(remainder) {
            let (key, value, new_remainder) = next?;
            remainder = new_remainder;

            let value = propagate_cow(&input, value);
            match key {
                Some(key_ref) => {
                    let key = propagate_cow(&input, Cow::Borrowed(key_ref));
                    if this.entries.insert(key.clone(), value).is_some() {
                        if let Some(collect_errors) = &mut collect_errors {
                            // avoid duplicating the key name:
                            collect_errors.push(key_ref.to_string(), format_err!("duplicate key"));
                        } else {
                            return Err(Error::msg(format!("duplicate key {key_ref:?}")));
                        }
                    } else {
                        this.entry_order.push_back(Some(key));
                    }
                }
                None => {
                    if this.value_without_key.is_some() {
                        Error::msg("multiple values without a key in property string")
                            .maybe_merge_to(&mut collect_errors)
                            .recoverable_ok()?;
                    } else {
                        this.entry_order.push_back(None);
                    }
                    this.value_without_key = Some(value);
                }
            }
        }

        Ok(this)
    }

    /// Returns the key (or `None` for a value without a key), and the value.
    fn next_entry(&mut self) -> Option<(Option<Cow<'de, str>>, Cow<'de, str>)> {
        loop {
            match self.entry_order.pop_front()? {
                None => {
                    if let Some(value) = self.value_without_key.take() {
                        return Some((None, value));
                    }
                }
                Some(key) => {
                    let value = self.entries.remove(&key)?;
                    return Some((Some(key), value));
                }
            }
        }
    }
}

/// This is the state while walking through the key/value pairs of a property string.
/// When a key is used it gets moved into the "used elements" hash, so it can stick around
/// as a lookup for `oneOf` types.
struct ObjectData<'de> {
    properties: ParsedPropertyString<'de>,
    used_elements: HashMap<Cow<'de, str>, Cow<'de, str>>,
}

impl crate::schema::WithTypeField for ObjectData<'_> {
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        func(self.lookup(name))
    }
}

impl<'de> ObjectData<'de> {
    fn split_cow<'i>(
        input: Cow3<'de, 'i, str>,
        collect_errors: Option<&mut ParameterError>,
    ) -> Result<Self, Error> {
        Ok(Self {
            properties: ParsedPropertyString::split_cow(input, collect_errors)?,
            used_elements: HashMap::new(),
        })
    }

    fn lookup(&self, key: &str) -> Option<&str> {
        if let Some(value) = self.properties.entries.get(key) {
            Some(value)
        } else {
            self.used_elements.get(key).map(|v| &**v)
        }
    }

    fn next_entry(&mut self) -> Option<(Option<Cow<'de, str>>, Cow<'de, str>)> {
        let (key, value) = self.properties.next_entry()?;
        if let Some(key) = &key {
            self.used_elements.insert(key.clone(), value.clone());
        }
        Some((key, value))
    }
}

/// Provides serde's `MapAccess` for parsing a property string.
struct MapAccess<'de, 'err> {
    data: ObjectData<'de>,

    /// The schema used to verify the contents and distinguish between structs and property
    /// strings.
    schema: &'static dyn schema::ObjectSchemaType,

    /// The current next value's key, value and schema (if available).
    value: Option<(Cow<'de, str>, Cow<'de, str>, Option<&'static Schema>)>,

    /// We just returned the key-value pair of a keyAlias:
    was_alias: Option<(&'static str, Cow<'de, str>)>,

    verify_state: Option<VerifyState<'err>>,
}

struct VerifyState<'err> {
    required_keys: BTreeSet<&'static str>,
    keys_seen: BTreeSet<String>,
    errors: &'err mut ParameterError,
}

impl Drop for MapAccess<'_, '_> {
    fn drop(&mut self) {
        if let Some(verify_state) = &mut self.verify_state {
            for key in &verify_state.required_keys {
                verify_state.errors.push(
                    key.to_string(),
                    format_err!("property is missing and it is not optional"),
                );
            }
            if let Err(errors) = self.schema.verify_type_fields(&self.data) {
                verify_state.errors.extend_unreported(errors);
            }
        }
    }
}

impl<'de, 'err> MapAccess<'de, 'err> {
    fn new_cow<'i>(
        input: Cow3<'de, 'i, str>,
        schema: &'static dyn schema::ObjectSchemaType,
        mut errors: Option<&'err mut ParameterError>,
    ) -> Result<Self, Error> {
        let data = ObjectData::split_cow(input, errors.as_deref_mut())?;

        let mut this = Self {
            data,
            schema,
            value: None,
            was_alias: None,
            verify_state: None,
        };

        if let Some(errors) = errors {
            this.prepare_verify(errors);
        }

        Ok(this)
    }

    fn prepare_verify(&mut self, errors: &'err mut ParameterError) {
        let required_keys = self
            .schema
            .all_properties()
            .with_types_from(&self.data.properties)
            .filter_map(|entry| (!entry.optional()).then_some(entry.name()))
            .collect();

        self.verify_state = Some(VerifyState {
            required_keys,
            keys_seen: BTreeSet::new(),
            errors,
        });
    }

    fn errors(&mut self) -> Option<&mut ParameterError> {
        self.verify_state.as_mut().map(|state| &mut *state.errors)
    }

    fn key_seen(&mut self, key: &str) {
        if let Some(verify_state) = &mut self.verify_state {
            // Track canonical names, an alias can map to a different property per oneOf variant.
            let canonical = match self.schema.lookup_alias(key) {
                None => key,
                Some(_) => self
                    .schema
                    .lookup_all(key)
                    .find(|entry| {
                        entry
                            .conditions()
                            .iter()
                            .all(|(name, value)| self.data.lookup(name) == Some(*value))
                    })
                    .map_or(key, |entry| entry.name()),
            };
            verify_state.required_keys.remove(canonical);

            if !verify_state.keys_seen.insert(canonical.to_string()) {
                verify_state
                    .errors
                    .push(key.to_string(), format_err!("duplicate key"));
            }
        }
    }

    /// Takes the key in a `<model>=<macaddr>` assignment.
    /// There may exist a `KeyAliasInfo { key_key, values, aliased_key }` such that the expression
    /// is turned into `<key_key>=<model>,<aliased_key>=<macaddr>`.
    ///
    /// Returns (key_key, value, aliased_key).
    ///
    /// `value` is equal to the passed `key` parameter, but replaced with its entry from the static
    /// `KeyAliasInfo` to drop its lifetime.
    fn try_key_alias_info(&self, key: &str) -> Option<(&'static str, &'static str, &'static str)> {
        let info = self.schema.key_alias_info()?;

        let index = info.values.binary_search(&key).ok()?;

        Some((info.key_alias, info.values[index], info.alias))
    }

    fn return_key_alias_info<K>(
        &mut self,
        seed: K,
        (key_key, info_value, alias_key): (&'static str, &'static str, &'static str),
        next_value: Cow<'de, str>,
    ) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        // If the object schema has a `KeyAliasInfo`, take a detour through it: if
        // our "key" is defined in it, we first return the key as a value for its declared
        // `keyAlias`.

        self.was_alias = Some((alias_key, next_value));

        let (_optional, schema) =
            self.schema
                .lookup_schema(key_key, &self.data)
                .ok_or(Error::msg(
                    "key alias info pointed to a key without a schema",
                ))?;

        self.key_seen(key_key);
        let out = seed.deserialize(<de::value::BorrowedStrDeserializer<'de, Error>>::new(
            key_key,
        ))?;

        self.value = Some((
            Cow::Borrowed(key_key),
            Cow::Borrowed(info_value),
            Some(schema),
        ));
        Ok(Some(out))
    }
}

impl<'de> MapAccess<'de, '_> {
    fn get_next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if let Some((key, value)) = self.was_alias.take() {
            self.key_seen(key);

            let out =
                seed.deserialize(de::value::BorrowedStrDeserializer::<'de, Error>::new(key))?;

            self.value = Some((
                Cow::Borrowed(key),
                value,
                self.schema
                    .lookup_schema(key, &self.data)
                    .map(|(_optional, schema)| schema),
            ));

            return Ok(Some(out));
        }

        let (key, value, schema) = match self.data.next_entry() {
            None => return Ok(None),
            Some((None, value)) => {
                let key = self
                    .schema
                    .default_key()
                    .ok_or_else(|| {
                        Error::msg("value without key, but schema does not define a default key")
                    })
                    .maybe_merge_to(&mut self.errors())?;

                let Some((_optional, schema)) = self.schema.lookup_schema(key, &self.data) else {
                    return Err(Error::msg("bad default key: without schema"));
                };

                (Cow::Borrowed(key), value, Some(schema))
            }
            Some((Some(key), value)) => {
                if let Some(info) = self.try_key_alias_info(&key) {
                    return self.return_key_alias_info(seed, info, value);
                }

                let schema = self
                    .schema
                    .lookup_schema(&key, &self.data)
                    .map(|(_optional, schema)| schema);

                (key, value, schema)
            }
        };

        let out = match &key {
            Cow::Borrowed(key) => {
                seed.deserialize(de::value::BorrowedStrDeserializer::<'de, Error>::new(key))?
            }
            Cow::Owned(key) => {
                seed.deserialize(IntoDeserializer::<Error>::into_deserializer(key.as_str()))?
            }
        };

        self.key_seen(&key);
        self.value = Some((key, value, schema));

        Ok(Some(out))
    }

    fn get_next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: DeserializeSeedOrVerify<'de>,
    {
        let (key, input, schema) = self.value.take().ok_or(Error::msg("bad map access"))?;

        if let Some(schema) = schema {
            let deserializer = SchemaDeserializer::new(input, schema);
            if let Some(verify_state) = &mut self.verify_state {
                let mut errors = ParameterError::new();
                let out = seed.deserialize(deserializer.collect_errors(&mut errors));
                if !errors.is_empty() {
                    verify_state.errors.add_errors(&key, errors.into());
                }
                out.merge_to_with_key(verify_state.errors, || key.to_string())
            } else {
                seed.deserialize(deserializer)
            }
        } else {
            if !self.schema.additional_properties() {
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.errors.push(
                        key.to_string(),
                        format_err!("schema does not allow additional properties"),
                    );
                } else {
                    return Err(Error::msg(format!("unknown key {:?}", key.as_ref())));
                }
            }

            // additional properties are treated as strings...
            let deserializer = no_schema::NoSchemaDeserializer::new(input);
            seed.deserialize_no_schema(deserializer)
                .maybe_merge_to_with_key(&mut self.errors(), || key.to_string())
        }
    }
}

impl<'de> de::MapAccess<'de> for MapAccess<'de, '_> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        self.get_next_key_seed(seed)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        self.get_next_value_seed(seed)
    }
}
