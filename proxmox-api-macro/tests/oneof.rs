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

#[test]
fn test_a_or_b_schema() {
    const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::OneOfSchema::new(
        "An A or a B.",
        &(
            "type",
            false,
            &schema::StringSchema::new("Type of the object.")
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
                .schema(),
        ),
        &[("A", &NameValue::API_SCHEMA), ("B", &IndexText::API_SCHEMA)],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, AOrB::API_SCHEMA);
}
