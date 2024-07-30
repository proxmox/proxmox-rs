use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::mem;

use anyhow::format_err;
use serde::de::{self, Deserialize, Unexpected};

use super::Schema;
use crate::schema::ParameterError;

struct VerifyState {
    schema: Option<&'static Schema>,
    path: String,
}

thread_local! {
    static VERIFY_SCHEMA: RefCell<Option<VerifyState>> = const { RefCell::new(None) };
    static ERRORS: RefCell<Vec<(String, anyhow::Error)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct SchemaGuard(Option<VerifyState>);

impl Drop for SchemaGuard {
    fn drop(&mut self) {
        VERIFY_SCHEMA.with(|schema| {
            if self.0.is_none() {
                ERRORS.with(|errors| errors.borrow_mut().clear())
            }
            *schema.borrow_mut() = self.0.take();
        });
    }
}

impl SchemaGuard {
    /// If this is the "final" guard, take out the errors:
    fn errors(self) -> Option<Vec<(String, anyhow::Error)>> {
        if self.0.is_none() {
            let errors = ERRORS.with(|e| mem::take(&mut *e.borrow_mut()));
            (!errors.is_empty()).then_some(errors)
        } else {
            None
        }
    }
}

pub(crate) fn push_schema(schema: Option<&'static Schema>, path: Option<&str>) -> SchemaGuard {
    SchemaGuard(VERIFY_SCHEMA.with(|s| {
        let prev = s.borrow_mut().take();
        let path = match (path, &prev) {
            (Some(path), Some(prev)) => join_path(&prev.path, path),
            (Some(path), None) => path.to_owned(),
            (None, Some(prev)) => prev.path.clone(),
            (None, None) => String::new(),
        };

        *s.borrow_mut() = Some(VerifyState { schema, path });

        prev
    }))
}

fn get_path() -> Option<String> {
    VERIFY_SCHEMA.with(|s| s.borrow().as_ref().map(|state| state.path.clone()))
}

fn get_schema() -> Option<&'static Schema> {
    VERIFY_SCHEMA.with(|s| s.borrow().as_ref().and_then(|state| state.schema))
}

pub(crate) fn is_verifying() -> bool {
    VERIFY_SCHEMA.with(|s| s.borrow().as_ref().is_some())
}

fn join_path(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{}/{}", a, b)
    }
}

fn push_errstr_path(err_path: &str, err: &str) {
    if let Some(path) = get_path() {
        push_err_do(join_path(&path, err_path), format_err!("{}", err));
    }
}

fn push_err(err: impl fmt::Display) {
    if let Some(path) = get_path() {
        push_err_do(path, format_err!("{}", err));
    }
}

fn push_err_do(path: String, err: anyhow::Error) {
    ERRORS.with(move |errors| errors.borrow_mut().push((path, err)))
}

/// Helper to collect multiple deserialization errors for better reporting.
///
/// This is similar to [`IgnoredAny`](serde::de::IgnoredAny) in that it implements [`Deserialize`]
/// but does not actually deserialize to anything, however, when a deserialization error occurs,
/// it'll try to continue and collect further errors.
///
/// This only makes sense with the [`SchemaDeserializer`](super::SchemaDeserializer).
pub struct Verifier;

impl<'de> Deserialize<'de> for Verifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        if let Some(schema) = get_schema() {
            let visitor = Visitor(schema);
            match schema {
                Schema::Boolean(_) => deserializer.deserialize_bool(visitor),
                Schema::Integer(_) => deserializer.deserialize_i64(visitor),
                Schema::Number(_) => deserializer.deserialize_f64(visitor),
                Schema::String(_) => deserializer.deserialize_str(visitor),
                Schema::Object(_) => deserializer.deserialize_map(visitor),
                Schema::AllOf(_) => deserializer.deserialize_map(visitor),
                Schema::OneOf(_) => deserializer.deserialize_map(visitor),
                Schema::Array(_) => deserializer.deserialize_seq(visitor),
                Schema::Null => deserializer.deserialize_unit(visitor),
            }
        } else {
            Ok(Verifier)
        }
    }
}

pub fn verify(schema: &'static Schema, value: &str) -> Result<(), anyhow::Error> {
    let guard = push_schema(Some(schema), None);
    Verifier::deserialize(super::SchemaDeserializer::new(value, schema))?;

    if let Some(errors) = guard.errors() {
        Err(ParameterError::from_list(errors).into())
    } else {
        Ok(())
    }
}

struct Visitor(&'static Schema);

impl<'de> de::Visitor<'de> for Visitor {
    type Value = Verifier;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Schema::Boolean(_) => f.write_str("boolean"),
            Schema::Integer(_) => f.write_str("integer"),
            Schema::Number(_) => f.write_str("number"),
            Schema::String(_) => f.write_str("string"),
            Schema::Object(_) => f.write_str("object"),
            Schema::AllOf(_) => f.write_str("allOf"),
            Schema::OneOf(_) => f.write_str("oneOf"),
            Schema::Array(_) => f.write_str("Array"),
            Schema::Null => f.write_str("null"),
        }
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        match self.0 {
            Schema::Boolean(_) => (),
            _ => return Err(E::invalid_type(Unexpected::Bool(v), &self)),
        }
        Ok(Verifier)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        match self.0 {
            Schema::Integer(schema) => match schema.check_constraints(v as isize) {
                Ok(()) => Ok(Verifier),
                Err(err) => Err(E::custom(err)),
            },
            _ => Err(E::invalid_type(Unexpected::Signed(v), &self)),
        }
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        match self.0 {
            Schema::Integer(schema) => match schema.check_constraints(v as isize) {
                Ok(()) => Ok(Verifier),
                Err(err) => Err(E::custom(err)),
            },
            _ => Err(E::invalid_type(Unexpected::Unsigned(v), &self)),
        }
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        match self.0 {
            Schema::Number(schema) => match schema.check_constraints(v) {
                Ok(()) => Ok(Verifier),
                Err(err) => Err(E::custom(err)),
            },
            _ => Err(E::invalid_type(Unexpected::Float(v), &self)),
        }
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        use de::Error;

        let schema = match self.0 {
            Schema::Array(schema) => schema,
            _ => return Err(A::Error::invalid_type(Unexpected::Seq, &self)),
        };

        let _guard = push_schema(Some(schema.items), None);

        let mut count = 0;
        loop {
            match seq.next_element::<Verifier>() {
                Ok(Some(_)) => count += 1,
                Ok(None) => break,
                Err(err) => push_err(err),
            }
        }

        schema.check_length(count).map_err(de::Error::custom)?;

        Ok(Verifier)
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use de::Error;

        let schema: &'static dyn crate::schema::ObjectSchemaType = match self.0 {
            Schema::Object(schema) => schema,
            Schema::AllOf(schema) => schema,
            Schema::OneOf(schema) => schema,
            _ => return Err(A::Error::invalid_type(Unexpected::Map, &self)),
        };

        // The tests need this to be in a predictable order, so HashSet won't work as it uses a
        // randomized default state.
        let mut required_keys = BTreeSet::<&'static str>::new();
        for (key, optional, _schema) in schema.properties() {
            if !optional {
                required_keys.insert(key);
            }
        }

        let mut other_keys = HashSet::<String>::new();
        loop {
            let key: Cow<'de, str> = match map.next_key()? {
                Some(key) => key,
                None => break,
            };

            let _guard = match schema.lookup(&key) {
                Some((optional, schema)) => {
                    if !optional {
                        // required keys are only tracked in the required_keys hashset
                        if !required_keys.remove(key.as_ref()) {
                            // duplicate key
                            push_errstr_path(&key, "duplicate key");
                        }
                    } else {
                        // optional keys
                        if !other_keys.insert(key.clone().into_owned()) {
                            push_errstr_path(&key, "duplicate key");
                        }
                    }

                    push_schema(Some(schema), Some(&key))
                }
                None => {
                    if !schema.additional_properties() {
                        push_errstr_path(&key, "schema does not allow additional properties");
                    } else if !other_keys.insert(key.clone().into_owned()) {
                        push_errstr_path(&key, "duplicate key");
                    }

                    push_schema(None, Some(&key))
                }
            };

            match map.next_value::<Verifier>() {
                Ok(Verifier) => (),
                Err(err) => push_err(err),
            }
        }

        for key in required_keys {
            push_errstr_path(key, "property is missing and it is not optional");
        }

        Ok(Verifier)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        let schema = match self.0 {
            Schema::String(schema) => schema,
            _ => return Err(E::invalid_type(Unexpected::Str(value), &self)),
        };

        #[allow(clippy::let_unit_value)]
        let _: () = schema.check_constraints(value).map_err(E::custom)?;

        Ok(Verifier)
    }
}
