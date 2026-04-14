use serde::{Deserialize, Serialize};

use proxmox_api_macro::api;
use proxmox_schema as schema;
use proxmox_schema::ApiType;

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
pub struct NameValue {
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
pub struct IndexText {
    index: u64,
    text: String,
}

#[api]
/// An A or a B.
#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum AOrB {
    /// Type A.
    A(NameValue),
    /// Type B.
    B(IndexText),
}

const A_OR_B_TYPE_SCHEMA: schema::Schema = schema::StringSchema::new("Type of the object.")
    .format(&schema::ApiStringFormat::Enum(&[
        schema::EnumEntry {
            value: "A",
            description: "Type A.",
        },
        schema::EnumEntry {
            value: "B",
            description: "Type B.",
        },
    ]))
    .schema();

#[test]
fn test_a_or_b_schema() {
    const TEST_SCHEMA: schema::Schema = schema::OneOfSchema::new(
        "An A or a B.",
        &("type", false, &A_OR_B_TYPE_SCHEMA),
        &[("A", &NameValue::API_SCHEMA), ("B", &IndexText::API_SCHEMA)],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, AOrB::API_SCHEMA);
}

#[api]
/// An A or a B - adjacently tagged.
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum AOrBAdjacent {
    /// Type A.
    A(NameValue),
    /// Type B.
    B(IndexText),
}

#[test]
fn test_adjacently_tagged() {
    const TEST_SCHEMA: schema::Schema = schema::OneOfSchema::new(
        "An A or a B - adjacently tagged.",
        &("type", false, &A_OR_B_TYPE_SCHEMA),
        &[
            (
                "A",
                &schema::ObjectSchema::new(
                    "An instance of A.",
                    &[("value", false, &NameValue::API_SCHEMA)],
                )
                .schema(),
            ),
            (
                "B",
                &schema::ObjectSchema::new(
                    "An instance of B.",
                    &[("value", false, &IndexText::API_SCHEMA)],
                )
                .schema(),
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, AOrBAdjacent::API_SCHEMA);
}
