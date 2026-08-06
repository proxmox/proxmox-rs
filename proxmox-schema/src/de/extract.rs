//! Partial object deserialization by extracting object portions from a Value using an api schema.

use std::cell::RefCell;
use std::fmt;

use serde::de::{self, IntoDeserializer, Visitor};
use serde_json::Value;

use crate::Schema;

pub struct Error {
    msg: String,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.msg, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.msg, f)
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            msg: msg.to_string(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error {
            msg: error.to_string(),
        }
    }
}

pub struct ExtractValueDeserializer<'o> {
    object: &'o mut serde_json::Map<String, Value>,
    schema: &'static dyn crate::ObjectSchemaType,
}

impl<'o> ExtractValueDeserializer<'o> {
    pub fn try_new(
        object: &'o mut serde_json::Map<String, Value>,
        schema: &'static Schema,
    ) -> Option<Self> {
        Some(Self {
            schema: schema.any_object()?,
            object,
        })
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
        // MapAccess wants to *move* data from the object, but the `all_properties()` iterator
        // wants to use it for type information.
        //
        // The iterator's implementation guarantees that the type info is queried before the
        // *contents* of a respective oneOf variant are yielded.
        //
        // `RefCell` allows both accesses: The type query and key removal each happen independently
        // with short-lived borrows, so they don't need to overlap.
        //
        // The RefCell is built here instead of within `MapAccess::new` to avoid another `Rc` layer
        // (since 'new' returns, so it cannot just take a reference on a function-local `RefCell`).
        // For simplicity we also build the iterator here and make `MapAccess` generic over its
        // type to avoid having to write-out a `mapped` iterator type or turning it into a trait
        // object.
        let object = RefCell::new(self.object);
        let with_type_field = |key: &str, func: &mut dyn FnMut(Option<&str>)| {
            func(object.borrow().get(key).and_then(Value::as_str))
        };
        let iter = self
            .schema
            .all_properties()
            .with_types_from(&with_type_field)
            .map(|item| item.name());
        visitor.visit_map(MapAccess::<'_, 'de>::new(&object, iter))
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
        self.deserialize_map(visitor)
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

struct MapAccess<'r, 'de, I> {
    object: &'r RefCell<&'de mut serde_json::Map<String, Value>>,
    // Shares and accesses the above RefCell and thus `iter.next()` must not be called while
    // holding a mutable borrow.
    iter: I,
    value: Option<Value>,
}

impl<'r, 'de, I> MapAccess<'r, 'de, I>
where
    I: Iterator<Item = &'static str>,
{
    fn new(object: &'r RefCell<&'de mut serde_json::Map<String, Value>>, iter: I) -> Self {
        Self {
            object,
            iter,
            value: None,
        }
    }
}

impl<'r, 'de, I> de::MapAccess<'de> for MapAccess<'r, 'de, I>
where
    I: Iterator<Item = &'static str>,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        loop {
            // `iter.next()` must be called without mutable borrows of self.object
            return match self.iter.next() {
                Some(key) => {
                    let value = self.object.borrow_mut().remove(key); // drop RefMut immediately
                    match value {
                        Some(value) => {
                            self.value = Some(value);
                            seed.deserialize(key.into_deserializer()).map(Some)
                        }
                        None => continue,
                    }
                }
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
#[allow(clippy::disallowed_names)]
fn test_extraction() {
    use serde::Deserialize;

    use crate::{ObjectSchema, StringSchema};

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

#[test]
fn test_one_of_extraction() {
    use serde::Deserialize;

    use crate::{ApiType, StringSchema};

    #[derive(Debug, Eq, PartialEq, Deserialize)]
    struct SomeData {
        key1: String,
        key2: String,

        #[serde(flatten)]
        an_enum: AnEnum,
    }

    #[derive(Debug, Eq, PartialEq, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct TypeA {
        clashing: String,
        prop_a: String,
    }

    #[derive(Debug, Eq, PartialEq, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct TypeB {
        clashing: u32,
        prop_b: String,
    }

    #[derive(Debug, Eq, PartialEq, Deserialize)]
    #[serde(tag = "type")]
    enum AnEnum {
        A(TypeA),
        B(TypeB),
    }

    impl ApiType for SomeData {
        const API_SCHEMA: Schema = crate::AllOfSchema::new(
            "Data",
            &[
                &crate::ObjectSchema::new(
                    "Some Data",
                    &[
                        ("key1", false, &StringSchema::new("Key 1").schema()),
                        ("key2", false, &StringSchema::new("Key 2").schema()),
                    ],
                )
                .schema(),
                &crate::OneOfSchema::new(
                    "An A or B",
                    &(
                        "type",
                        false,
                        &StringSchema::new("A or B")
                            .format(&crate::ApiStringFormat::Enum(&[
                                crate::EnumEntry::new("A", "Type A"),
                                crate::EnumEntry::new("B", "Type B"),
                            ]))
                            .schema(),
                    ),
                    &[
                        (
                            "A",
                            &crate::ObjectSchema::new(
                                "Instance A",
                                &[
                                    ("clashing", false, &StringSchema::new("Clashing A").schema()),
                                    ("prop-a", false, &StringSchema::new("Prop A").schema()),
                                ],
                            )
                            .schema(),
                        ),
                        (
                            "B",
                            &crate::ObjectSchema::new(
                                "Instance B",
                                &[
                                    (
                                        "clashing",
                                        false,
                                        &crate::IntegerSchema::new("Clashing B").schema(),
                                    ),
                                    ("prop-b", false, &StringSchema::new("Prop B").schema()),
                                ],
                            )
                            .schema(),
                        ),
                    ],
                )
                .schema(),
            ],
        )
        .schema();
    }

    let mut data = serde_json::json!({
        "kept1": "keep1",
        "kept2": "keep2",
        "key1": "Hello",
        "key2": "Hello2",
        "type": "B",
        "clashing": 33,
        "prop-a": "keep3",
        "prop-b": "What",
    });
    let data = data.as_object_mut().unwrap();

    let extracted = SomeData::deserialize(
        ExtractValueDeserializer::try_new(data, &SomeData::API_SCHEMA).unwrap(),
    )
    .expect("object should be extractable");
    assert_eq!(
        extracted,
        SomeData {
            key1: "Hello".to_string(),
            key2: "Hello2".to_string(),
            an_enum: AnEnum::B(TypeB {
                clashing: 33,
                prop_b: "What".to_string(),
            }),
        },
        "extracted object got the right type",
    );

    assert_eq!(data.len(), 3, "3 keys should be left");
    assert_eq!(data["kept1"].as_str().unwrap(), "keep1", "kept1 stayed");
    assert_eq!(data["kept2"].as_str().unwrap(), "keep2", "kept2 stayed");
    assert_eq!(data["prop-a"].as_str().unwrap(), "keep3", "prop-a stayed");

    let mut data = serde_json::json!({
        "kept1": "keep1",
        "kept2": "keep2",
        "key1": "Hello",
        "key2": "Hello2",
        "type": "A",
        "clashing": "String",
        "prop-a": "Yeah",
        "prop-b": "keep3",
    });
    let data = data.as_object_mut().unwrap();

    let extracted = SomeData::deserialize(
        ExtractValueDeserializer::try_new(data, &SomeData::API_SCHEMA).unwrap(),
    )
    .expect("object should be extractable");
    assert_eq!(
        extracted,
        SomeData {
            key1: "Hello".to_string(),
            key2: "Hello2".to_string(),
            an_enum: AnEnum::A(TypeA {
                clashing: "String".to_string(),
                prop_a: "Yeah".to_string(),
            }),
        },
        "extracted object got the right type",
    );

    assert_eq!(data.len(), 3, "3 keys should be left");
    assert_eq!(data["kept1"].as_str().unwrap(), "keep1", "kept1 stayed");
    assert_eq!(data["kept2"].as_str().unwrap(), "keep2", "kept2 stayed");
    assert_eq!(data["prop-b"].as_str().unwrap(), "keep3", "prop-b stayed");
}
