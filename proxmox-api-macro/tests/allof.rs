//! Testing the `AllOf` schema on structs and methods.

use proxmox::api::schema;
use proxmox_api_macro::api;

use serde::{Deserialize, Serialize};

pub const NAME_SCHEMA: schema::Schema = schema::StringSchema::new("Name.").schema();
pub const VALUE_SCHEMA: schema::Schema = schema::IntegerSchema::new("Value.").schema();
pub const INDEX_SCHEMA: schema::Schema = schema::IntegerSchema::new("Index.").schema();
pub const TEXT_SCHEMA: schema::Schema = schema::StringSchema::new("Text.").schema();

#[api(
    properties: {
        name: { schema: NAME_SCHEMA },
        value: { schema: VALUE_SCHEMA },
    }
)]
/// Name and value.
#[derive(Deserialize, Serialize)]
struct NameValue {
    name: String,
    value: u64,
}

#[api(
    properties: {
        index: { schema: INDEX_SCHEMA },
        text: { schema: TEXT_SCHEMA },
    }
)]
/// Index and text.
#[derive(Deserialize, Serialize)]
struct IndexText {
    index: u64,
    text: String,
}

#[api(
    properties: {
        nv: { type: NameValue },
        it: { type: IndexText },
    },
)]
/// Name, value, index and text.
#[derive(Deserialize, Serialize)]
struct Nvit {
    #[serde(flatten)]
    nv: NameValue,

    #[serde(flatten)]
    it: IndexText,
}

#[test]
fn test_nvit() {
    const TEST_NAME_VALUE_SCHEMA: ::proxmox::api::schema::Schema =
        ::proxmox::api::schema::ObjectSchema::new(
            "Name and value.",
            &[
                ("name", false, &NAME_SCHEMA),
                ("value", false, &VALUE_SCHEMA),
            ],
        )
        .schema();

    const TEST_SCHEMA: ::proxmox::api::schema::Schema = ::proxmox::api::schema::AllOfSchema::new(
        "Name, value, index and text.",
        &[&TEST_NAME_VALUE_SCHEMA, &IndexText::API_SCHEMA],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, Nvit::API_SCHEMA);
}

#[api(
    properties: {
        nv: { type: NameValue },
        it: { type: IndexText },
    },
)]
/// Extra Schema
#[derive(Deserialize, Serialize)]
struct WithExtra {
    #[serde(flatten)]
    nv: NameValue,

    #[serde(flatten)]
    it: IndexText,

    /// Extra field.
    extra: String,
}

#[test]
fn test_extra() {
    const INNER_SCHEMA: ::proxmox::api::schema::Schema = ::proxmox::api::schema::ObjectSchema::new(
        "<INNER: Extra Schema>",
        &[(
            "extra",
            false,
            &::proxmox::api::schema::StringSchema::new("Extra field.").schema(),
        )],
    )
    .schema();

    const TEST_SCHEMA: ::proxmox::api::schema::Schema = ::proxmox::api::schema::AllOfSchema::new(
        "Extra Schema",
        &[
            &INNER_SCHEMA,
            &NameValue::API_SCHEMA,
            &IndexText::API_SCHEMA,
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, WithExtra::API_SCHEMA);
}
