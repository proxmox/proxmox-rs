//! Partial object deserialization by extracting object portions from a Value using an api schema.

use std::fmt;

use serde::de::{self, IntoDeserializer, Visitor};
use serde_json::Value;

use crate::api::schema::{ObjectSchemaType, Schema};

pub struct Error {
    inner: anyhow::Error,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            inner: anyhow::format_err!("{}", msg),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(inner: serde_json::Error) -> Self {
        Error {
            inner: inner.into(),
        }
    }
}

pub struct ExtractValueDeserializer<'o> {
    object: &'o mut serde_json::Map<String, Value>,
    schema: &'static Schema,
}

impl<'o> ExtractValueDeserializer<'o> {
    pub fn try_new(
        object: &'o mut serde_json::Map<String, Value>,
        schema: &'static Schema,
    ) -> Option<Self> {
        match schema {
            Schema::Object(_) | Schema::AllOf(_) => Some(Self { object, schema }),
            _ => None,
        }
    }
}

macro_rules! deserialize_non_object {
    ($name:ident) => {
        fn $name<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom(
                "deserializing partial object into type which is not an object",
            ))
        }
    };
    ($name:ident ( $($args:tt)* )) => {
        fn $name<V>(self, $($args)*, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            Err(de::Error::custom(
                "deserializing partial object into type which is not an object",
            ))
        }
    };
}

impl<'de> de::Deserializer<'de> for ExtractValueDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        use serde::de::Error;

        match self.schema {
            Schema::Object(schema) => visitor.visit_map(MapAccess::<'de>::new(
                self.object,
                schema.properties().map(|(name, _, _)| *name),
            )),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::<'de>::new(
                self.object,
                schema.properties().map(|(name, _, _)| *name),
            )),

            // The following should be caught by ExtractValueDeserializer::new()!
            _ => Err(Error::custom(
                "ExtractValueDeserializer used with invalid schema",
            )),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        use serde::de::Error;

        match self.schema {
            Schema::Object(schema) => visitor.visit_map(MapAccess::<'de>::new(
                self.object,
                schema.properties().map(|(name, _, _)| *name),
            )),
            Schema::AllOf(schema) => visitor.visit_map(MapAccess::<'de>::new(
                self.object,
                schema.properties().map(|(name, _, _)| *name),
            )),

            // The following should be caught by ExtractValueDeserializer::new()!
            _ => Err(Error::custom(
                "ExtractValueDeserializer used with invalid schema",
            )),
        }
    }

    deserialize_non_object!(deserialize_i8);
    deserialize_non_object!(deserialize_i16);
    deserialize_non_object!(deserialize_i32);
    deserialize_non_object!(deserialize_i64);
    deserialize_non_object!(deserialize_u8);
    deserialize_non_object!(deserialize_u16);
    deserialize_non_object!(deserialize_u32);
    deserialize_non_object!(deserialize_u64);
    deserialize_non_object!(deserialize_f32);
    deserialize_non_object!(deserialize_f64);
    deserialize_non_object!(deserialize_char);
    deserialize_non_object!(deserialize_bool);
    deserialize_non_object!(deserialize_str);
    deserialize_non_object!(deserialize_string);
    deserialize_non_object!(deserialize_bytes);
    deserialize_non_object!(deserialize_byte_buf);
    deserialize_non_object!(deserialize_option);
    deserialize_non_object!(deserialize_seq);
    deserialize_non_object!(deserialize_unit);
    deserialize_non_object!(deserialize_identifier);
    deserialize_non_object!(deserialize_unit_struct(_: &'static str));
    deserialize_non_object!(deserialize_newtype_struct(_: &'static str));
    deserialize_non_object!(deserialize_tuple(_: usize));
    deserialize_non_object!(deserialize_tuple_struct(_: &'static str, _: usize));
    deserialize_non_object!(deserialize_enum(
        _: &'static str,
        _: &'static [&'static str]
    ));

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

struct MapAccess<'o, I> {
    object: &'o mut serde_json::Map<String, Value>,
    iter: I,
    value: Option<Value>,
}

impl<'o, I> MapAccess<'o, I>
where
    I: Iterator<Item = &'static str>,
{
    fn new(object: &'o mut serde_json::Map<String, Value>, iter: I) -> Self {
        Self {
            object,
            iter,
            value: None,
        }
    }
}

impl<'de, I> de::MapAccess<'de> for MapAccess<'de, I>
where
    I: Iterator<Item = &'static str>,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        loop {
            return match self.iter.next() {
                Some(key) => match self.object.remove(key) {
                    Some(value) => {
                        self.value = Some(value);
                        seed.deserialize(key.into_deserializer()).map(Some)
                    }
                    None => continue,
                },
                None => Ok(None),
            };
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(value).map_err(Error::from),
            None => Err(de::Error::custom("value is missing")),
        }
    }
}

#[test]
fn test_extraction() {
    use serde::Deserialize;

    use crate::api::schema::{ObjectSchema, StringSchema};

    #[derive(Deserialize)]
    struct Foo {
        foo1: String,
        foo2: String,
    }

    const SIMPLE_STRING: Schema = StringSchema::new("simple").schema();
    const FOO_SCHEMA: Schema = ObjectSchema::new(
        "A Foo",
        &[
            ("foo1", false, &SIMPLE_STRING),
            ("foo2", false, &SIMPLE_STRING),
        ],
    )
    .schema();

    #[derive(Deserialize)]
    struct Bar {
        bar1: String,
        bar2: String,
    }

    const BAR_SCHEMA: Schema = ObjectSchema::new(
        "A Bar",
        &[
            ("bar1", false, &SIMPLE_STRING),
            ("bar2", false, &SIMPLE_STRING),
        ],
    )
    .schema();

    let mut data = serde_json::json!({
        "foo1": "hey1",
        "foo2": "hey2",
        "bar1": "there1",
        "bar2": "there2",
    });

    let data = data.as_object_mut().unwrap();

    let foo: Foo =
        Foo::deserialize(ExtractValueDeserializer::try_new(data, &FOO_SCHEMA).unwrap()).unwrap();

    assert!(data.remove("foo1").is_none());
    assert!(data.remove("foo2").is_none());
    assert_eq!(foo.foo1, "hey1");
    assert_eq!(foo.foo2, "hey2");

    let bar =
        Bar::deserialize(ExtractValueDeserializer::try_new(data, &BAR_SCHEMA).unwrap()).unwrap();

    assert!(data.is_empty());
    assert_eq!(bar.bar1, "there1");
    assert_eq!(bar.bar2, "there2");
}
