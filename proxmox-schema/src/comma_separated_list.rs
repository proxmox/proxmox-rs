//! Comma-separated list strings.
//!
//! This module provides [`CommaSeparatedList<T>`], a newtype wrapper around `Vec<T>` that
//! serializes to and deserializes from a comma-separated string representation (e.g. `"1,2,3"`).
//! This is useful for API parameters that accept multiple values encoded in a single string field.
//!
//! Element types must implement the [`CommaSeparatedListSchema`] trait, which provides the static
//! [`ArraySchema`](crate::ArraySchema) used for validation during deserialization, but currently
//! not on serialization.
//!
//! Note that individual element values are **not quoted** in the serialized string — they are
//! simply joined with commas. This means element types must serialize to simple strings that do not
//! themselves contain commas.
//!
//! # Example
//!
//! ```
//! use proxmox_schema::{ApiType, Schema, ArraySchema, IntegerSchema};
//! use proxmox_schema::comma_separated_list::{CommaSeparatedList, CommaSeparatedListSchema};
//!
//! #[derive(Clone, Debug, PartialEq,  serde::Serialize, serde::Deserialize)]
//! struct Port(u16);
//!
//! const PORT_SCHEMA: Schema = IntegerSchema::new("A network port")
//!     .minimum(1)
//!     .maximum(65535)
//!     .schema();
//!
//! impl ApiType for Port {
//!     const API_SCHEMA: Schema = PORT_SCHEMA;
//! }
//!
//! impl CommaSeparatedListSchema for Port {
//!     const ARRAY_SCHEMA: Schema =
//!         ArraySchema::new("List of network ports.", &PORT_SCHEMA).schema();
//! }
//!
//! // Deserialize from a comma-separated string:
//! let ports: CommaSeparatedList<Port> =
//!     serde_json::from_value("80,443,8080".into()).unwrap();
//! assert_eq!(ports.len(), 3);
//! assert_eq!(ports[0], Port(80));
//! assert_eq!(ports[1], Port(443));
//! assert_eq!(ports[2], Port(8080));
//!
//! // Serialize back to a comma-separated string:
//! let value = serde_json::to_value(&ports).unwrap();
//! assert_eq!(value.as_str(), Some("80,443,8080"));
//! ```
//!
use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ApiStringFormat, ApiType, Schema, StringSchema};

fn serialize<S, T>(
    data: &[T],
    serializer: S,
    array_schema: &'static Schema,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    use serde::ser::{Error, SerializeSeq};

    let mut ser = crate::ser::PropertyStringSerializer::new(String::new(), array_schema)
        .serialize_seq(Some(data.len()))
        .map_err(S::Error::custom)?;

    for element in data {
        ser.serialize_element(element).map_err(S::Error::custom)?;
    }

    let out = ser.end().map_err(S::Error::custom)?;
    serializer.serialize_str(&out)
}

fn deserialize<'de, D, T>(
    deserializer: D,
    array_schema: &'static Schema,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    use serde::de::Error;

    let string = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;

    Vec::<T>::deserialize(crate::de::SchemaDeserializer::new(string, array_schema))
        .map_err(D::Error::custom)
}

/// Trait to provide a static array schema for a type.
///
/// This is needed because generic const items are unstable in Rust.
pub trait CommaSeparatedListSchema: ApiType {
    /// The static array schema for this type.
    const ARRAY_SCHEMA: Schema;
}

/// A [`Vec<T>`] that serializes to and deserializes from a comma-separated string.
///
/// The inner type `T` must implement [`CommaSeparatedListSchema`] so that element-level API schema
/// is available for the API definitions and so that validation is applied during deserialization.
///
/// Element values **must not** contain commas themselves - the format does not support quoting or
/// escaping.
///
/// See the [module-level documentation](self) for usage examples and details.
#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct CommaSeparatedList<T>(pub Vec<T>);

impl<T> ApiType for CommaSeparatedList<T>
where
    T: CommaSeparatedListSchema,
{
    const API_SCHEMA: Schema = StringSchema::new(T::ARRAY_SCHEMA.unwrap_array_schema().description)
        .format(&ApiStringFormat::PropertyString(&T::ARRAY_SCHEMA))
        .schema();
}

impl<T: CommaSeparatedListSchema + Serialize> Serialize for CommaSeparatedList<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(&self.0, serializer, &T::ARRAY_SCHEMA)
    }
}

impl<'de, T: CommaSeparatedListSchema + Deserialize<'de>> Deserialize<'de>
    for CommaSeparatedList<T>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<T> = deserialize(deserializer, &T::ARRAY_SCHEMA)?;
        Ok(CommaSeparatedList(vec))
    }
}

impl<T> CommaSeparatedList<T> {
    pub fn new(inner: Vec<T>) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> Deref for CommaSeparatedList<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for CommaSeparatedList<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<Vec<T>> for CommaSeparatedList<T> {
    fn from(inner: Vec<T>) -> Self {
        Self::new(inner)
    }
}

impl<T> IntoIterator for CommaSeparatedList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArraySchema, IntegerSchema};

    // Test type that implements CommaSeparatedListSchema
    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestNum(u32);

    const TEST_NUM_SCHEMA: Schema = IntegerSchema::new("Test number (0-3)").maximum(3).schema();
    const TEST_NUM_ARRAY_SCHEMA: Schema =
        ArraySchema::new("Array of test numbers.", &TEST_NUM_SCHEMA).schema();

    impl ApiType for TestNum {
        const API_SCHEMA: Schema = TEST_NUM_SCHEMA;
    }

    impl CommaSeparatedListSchema for TestNum {
        const ARRAY_SCHEMA: Schema = TEST_NUM_ARRAY_SCHEMA;
    }

    #[test]
    fn test_comma_separated_list_serialize() {
        let list = CommaSeparatedList(vec![TestNum(1), TestNum(2), TestNum(3)]);
        let s = serde_json::to_value(&list).unwrap();
        // The serialize function should produce a property string
        assert_eq!(s.as_str(), Some("1,2,3"));
    }

    #[test]
    fn test_comma_separated_list_deref() {
        let list = CommaSeparatedList(vec![TestNum(42)]);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], TestNum(42));
    }

    #[test]
    fn test_comma_separated_list_deserialize() {
        let list: CommaSeparatedList<TestNum> = serde_json::from_value("1,2,3".into()).unwrap();
        assert_eq!(list.0, vec![TestNum(1), TestNum(2), TestNum(3)]);
        // test integer maximum (4 > maximum)
        let _ = serde_json::from_value::<CommaSeparatedList<TestNum>>("3,4".into()).unwrap_err();
    }

    #[test]
    fn test_comma_separated_list_description() {
        let descr = CommaSeparatedList::<TestNum>::API_SCHEMA
            .unwrap_string_schema()
            .description;
        assert_eq!(descr, "Array of test numbers.");
    }

    #[test]
    fn test_round_trip() {
        let original = CommaSeparatedList::new(vec![TestNum(1), TestNum(2), TestNum(3)]);
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: CommaSeparatedList<TestNum> = serde_json::from_value(serialized).unwrap();
        assert_eq!(original.0, deserialized.0);
    }

    #[test]
    fn test_single_element_serialize() {
        let list = CommaSeparatedList::new(vec![TestNum(2)]);
        let serialized = serde_json::to_value(&list).unwrap();
        // Must not produce trailing/leading commas.
        assert_eq!(serialized.as_str(), Some("2"));
    }

    #[test]
    fn test_single_element_deserialize() {
        let list: CommaSeparatedList<TestNum> = serde_json::from_value("2".into()).unwrap();
        assert_eq!(list.0, vec![TestNum(2)]);
    }

    #[test]
    fn test_single_element_round_trip() {
        let original = CommaSeparatedList::new(vec![TestNum(0)]);
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: CommaSeparatedList<TestNum> = serde_json::from_value(serialized).unwrap();
        assert_eq!(original.0, deserialized.0);
    }

    #[test]
    fn test_empty_list_serialize() {
        let list: CommaSeparatedList<TestNum> = CommaSeparatedList::new(vec![]);
        let serialized = serde_json::to_value(&list).unwrap();
        // An empty vec should serialize to an empty string.
        assert_eq!(serialized.as_str(), Some(""));
    }

    #[test]
    fn test_empty_list_deserialize() {
        let result = serde_json::from_value::<CommaSeparatedList<TestNum>>("".into());

        match result {
            Ok(list) => assert!(
                list.is_empty(),
                "empty string should deserialize to an empty list, got {:?}",
                list.0
            ),
            Err(err) => {
                panic!("empty string should deserialize to an empty list but got error {err:?}");
            }
        }
    }

    #[test]
    fn test_empty_list_round_trip() {
        let original: CommaSeparatedList<TestNum> = CommaSeparatedList::new(vec![]);
        let serialized = serde_json::to_value(&original).unwrap();

        if let Ok(deserialized) = serde_json::from_value::<CommaSeparatedList<TestNum>>(serialized)
        {
            assert!(deserialized.is_empty());
        }
    }

    #[test]
    fn test_from_vec_conversion() {
        let v = vec![TestNum(1), TestNum(2), TestNum(3)];
        let list: CommaSeparatedList<TestNum> = v.clone().into();
        assert_eq!(&list.0, &v);
    }

    #[test]
    fn test_from_vec_empty() {
        let v: Vec<TestNum> = vec![];
        let list: CommaSeparatedList<TestNum> = v.into();
        assert!(list.is_empty());
    }

    #[test]
    fn test_serialize_validation_exceeds_maximum() {
        // TestNum schema has maximum(3), but serialization must not reject out-of-range values —
        // validation is a deserialization concern only.
        // TODO: re-evaluate if this is really what we want.
        let list = CommaSeparatedList::new(vec![TestNum(1), TestNum(4)]);
        let serialized = serde_json::to_value(&list).unwrap();
        assert_eq!(serialized.as_str(), Some("1,4"));
    }

    #[test]
    fn test_deserialize_validation_exceeds_maximum() {
        // TestNum schema has maximum(3), so "4" violates the constraint.
        // TODO: re-evaluate if this is really what we *always* want (yes for config parsers, but
        // maybe not for clients consuming responses from different major versions of an API)
        let result = serde_json::from_value::<CommaSeparatedList<TestNum>>("1,4".into());
        assert!(
            result.is_err(),
            "deserializing a value exceeding maximum should fail, got: {:?}",
            result.unwrap()
        );
    }
}
