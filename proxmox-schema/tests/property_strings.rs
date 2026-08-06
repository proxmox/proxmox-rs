use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use proxmox_api_macro::api;

use proxmox_schema::property_string;
use proxmox_schema::ser::PropertyStringSerializer;
use proxmox_schema::{NumberSchema, ObjectSchema, OneOfSchema, Schema, StringSchema};

#[api]
/// A one-of backed PropertyString
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnEnum {
    /// Variant A.
    A(VariantA),
    /// Variant B.
    B(VariantB),
}

#[api]
/// Variant A.
#[derive(Deserialize, Serialize)]
struct VariantA {
    /// Clashing in A.
    clashing: String,
    /// Property A.
    prop_a: String,
}

#[api(
    properties: {
        clashing: {
            type: Array,
            items: {
                type: Integer,
                description: "A number",
            },
        },
    },
    default_key: "prop_b",
)]
/// Variant B.
#[derive(Clone, Deserialize, Serialize)]
struct VariantB {
    /// Clashing in B.
    clashing: Vec<u32>,
    /// Property B.
    prop_b: String,
}

#[test]
fn test_clashing_one_of_serialization() {
    let value = AnEnum::A(VariantA {
        clashing: "hey".to_string(),
        prop_a: "foo".to_string(),
    });
    let s = property_string::print(&value).expect("failed to serialize test object");
    assert_eq!(s, "type=a,clashing=hey,prop_a=foo");

    let inner = VariantB {
        clashing: vec![33, 44],
        prop_b: "foo".to_string(),
    };
    let s = property_string::print(&inner).expect("failed to serialize test object");
    assert_eq!(s, "clashing=\"33,44\",foo");

    // Coming through a `oneOf`, default keys are ignored since they could differ across variants
    let value = AnEnum::B(inner);
    let s = property_string::print(&value).expect("failed to serialize test object");
    assert_eq!(s, "type=b,clashing=\"33,44\",prop_b=foo");
}

#[test]
fn test_one_of_type_property_names_with_other_types() {
    // `kind` is the type of the nested choice, but a map in the other variant
    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "lowercase")]
    enum Outer {
        Nested(Inner),
        Map { kind: BTreeMap<Key, &'static str> },
    }

    #[derive(Serialize)]
    #[serde(tag = "kind", rename_all = "lowercase")]
    enum Inner {
        Inner { payload: String },
    }

    // cannot be a JSON object key
    #[derive(Serialize, Eq, Ord, PartialEq, PartialOrd)]
    enum Key {
        Wrapped(&'static str),
    }

    static STRING: Schema = StringSchema::new("A string.").schema();
    static INNER: Schema = OneOfSchema::new(
        "Inner choice.",
        &("kind", false, &STRING),
        &[(
            "inner",
            &ObjectSchema::new("Payload.", &[("payload", false, &STRING)]).schema(),
        )],
    )
    .schema();
    static MAP: Schema = ObjectSchema::new("Map.", &[("name", false, &STRING)]).schema();
    static SCHEMA: Schema = OneOfSchema::new(
        "Outer choice.",
        &("type", false, &STRING),
        &[
            (
                "map",
                &ObjectSchema::new("Map variant.", &[("kind", false, &MAP)]).schema(),
            ),
            ("nested", &INNER),
        ],
    )
    .schema();

    for (value, expected) in [
        (
            Outer::Nested(Inner::Inner {
                payload: "value".into(),
            }),
            "type=nested,kind=inner,payload=value",
        ),
        (
            Outer::Map {
                kind: [(Key::Wrapped("name"), "value")].into(),
            },
            r#"type=map,kind="name=value""#,
        ),
    ] {
        let serialized = value
            .serialize(PropertyStringSerializer::new(String::new(), &SCHEMA))
            .unwrap();
        assert_eq!(serialized, expected);
    }
}

#[test]
fn test_escaped_one_of_type() {
    #[derive(Serialize)]
    #[serde(tag = "type")]
    enum Escaped {
        #[serde(rename = "a\\b")]
        Backslash { payload: String },
    }

    static STRING: Schema = StringSchema::new("A string.").schema();
    static SCHEMA: Schema = OneOfSchema::new(
        "Escaped type.",
        &("type", false, &STRING),
        &[(
            "a\\b",
            &ObjectSchema::new("Payload.", &[("payload", false, &STRING)]).schema(),
        )],
    )
    .schema();

    let serialized = Escaped::Backslash {
        payload: "value".into(),
    }
    .serialize(PropertyStringSerializer::new(String::new(), &SCHEMA))
    .unwrap();
    assert_eq!(serialized, r#"type="a\\b",payload=value"#);
}

#[test]
fn test_numeric_one_of_type() {
    #[derive(Serialize)]
    struct NumberTagged<T> {
        kind: T,
        payload: &'static str,
    }

    static SCHEMA: Schema = OneOfSchema::new(
        "Numeric type.",
        &("kind", false, &NumberSchema::new("Type.").schema()),
        &[(
            "1",
            &ObjectSchema::new(
                "Payload.",
                &[("payload", false, &StringSchema::new("A string.").schema())],
            )
            .schema(),
        )],
    )
    .schema();

    fn serialize(value: impl Serialize) -> String {
        value
            .serialize(PropertyStringSerializer::new(String::new(), &SCHEMA))
            .unwrap()
    }
    // `1.0_f64` is written as `1`, the variant lookup must use the same
    for serialized in [
        serialize(NumberTagged {
            kind: 1_u64,
            payload: "value",
        }),
        serialize(NumberTagged {
            kind: 1.0_f64,
            payload: "value",
        }),
    ] {
        assert_eq!(serialized, "kind=1,payload=value");
    }
}
