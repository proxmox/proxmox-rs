//! This should test the usage of "external" types. For any unrecognized schema type we expect the
//! type's impl to provide an `pub const API_SCHEMA: &Schema`.

#![allow(dead_code)]

use std::collections::HashMap;

use proxmox_api_macro::api;
use proxmox_schema as schema;
use proxmox_schema::{ApiType, EnumEntry};

use anyhow::Error;
use serde::Deserialize;
use serde_json::Value;

pub const TEXT_SCHEMA: schema::Schema = schema::StringSchema::new("Text.").schema();

#[api(
    type: String,
    description: "A string",
    format: &schema::ApiStringFormat::Enum(&[
        EnumEntry::new("ok", "Ok"),
        EnumEntry::new("not-ok", "Not OK"),
    ]),
)]
//#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OkString(String);

#[test]
fn ok_string() {
    const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::StringSchema::new("A string")
        .format(&schema::ApiStringFormat::Enum(&[
            EnumEntry::new("ok", "Ok"),
            EnumEntry::new("not-ok", "Not OK"),
        ]))
        .schema();
    assert_eq!(TEST_SCHEMA, OkString::API_SCHEMA);
}

#[api]
/// An example of a simple struct type.
pub struct TestStruct {
    /// A test string.
    test_string: String,

    /// An optional auto-derived value for testing:
    another: Option<String>,
}

#[test]
fn test_struct() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "An example of a simple struct type.",
        &[
            (
                "another",
                true,
                &::proxmox_schema::StringSchema::new("An optional auto-derived value for testing:")
                    .schema(),
            ),
            (
                "test_string",
                false,
                &::proxmox_schema::StringSchema::new("A test string.").schema(),
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, TestStruct::API_SCHEMA);
}

#[api]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
/// An example of a struct with renamed fields.
pub struct RenamedStruct {
    /// A test string.
    test_string: String,

    /// An optional auto-derived value for testing:
    #[serde(rename = "SomeOther")]
    another: Option<String>,
}

#[test]
fn renamed_struct() {
    const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "An example of a struct with renamed fields.",
        &[
            (
                "SomeOther",
                true,
                &::proxmox_schema::StringSchema::new("An optional auto-derived value for testing:")
                    .schema(),
            ),
            (
                "test-string",
                false,
                &::proxmox_schema::StringSchema::new("A test string.").schema(),
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, RenamedStruct::API_SCHEMA);
}

#[api]
#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// A selection of either 'onekind', 'another-kind' or 'selection-number-three'.
pub enum Selection {
    /// The first kind.
    #[serde(rename = "onekind")]
    OneKind,
    /// Some other kind.
    #[default]
    AnotherKind,
    /// And yet another.
    SelectionNumberThree,
}

#[test]
fn selection_test() {
    const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::StringSchema::new(
        "A selection of either \'onekind\', \'another-kind\' or \'selection-number-three\'.",
    )
    .format(&::proxmox_schema::ApiStringFormat::Enum(&[
        EnumEntry::new("onekind", "The first kind."),
        EnumEntry::new("another-kind", "Some other kind."),
        EnumEntry::new("selection-number-three", "And yet another."),
    ]))
    .default("another-kind")
    .schema();

    assert_eq!(TEST_SCHEMA, Selection::API_SCHEMA);
}

// Initial test:
#[api(
    input: {
        properties: {
            arg: { type: OkString },
            selection: { type: Selection },
        }
    },
    returns: { optional: true, type: Boolean },
)]
/// Check a string.
///
/// Returns: Whether the string was "ok".
pub fn string_check(arg: Value, selection: Selection) -> Result<bool, Error> {
    let _ = arg;
    let _ = selection;
    panic!("body")
}

#[test]
fn string_check_schema_test() {
    const TEST_METHOD: ::proxmox_router::ApiMethod = ::proxmox_router::ApiMethod::new(
        &::proxmox_router::ApiHandler::Sync(&api_function_string_check),
        &::proxmox_schema::ObjectSchema::new(
            "Check a string.",
            &[
                ("arg", false, &OkString::API_SCHEMA),
                ("selection", false, &Selection::API_SCHEMA),
            ],
        ),
    )
    .returns(::proxmox_schema::ReturnType::new(
        true,
        &::proxmox_schema::BooleanSchema::new("Whether the string was \"ok\".").schema(),
    ))
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_STRING_CHECK);
}

#[api(
    properties: {
        "a-field": {
            description: "Some description.",
        },
    },
)]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Some Description.
pub struct RenamedAndDescribed {
    a_field: String,
}

#[api(
    properties: {},
    additional_properties: "rest",
)]
#[derive(Deserialize)]
/// Some Description.
pub struct UnspecifiedData {
    /// Text.
    field: String,

    /// Remaining data.
    rest: HashMap<String, Value>,
}

#[test]
fn additional_properties_test() {
    const TEST_UNSPECIFIED: ::proxmox_schema::Schema =
        ::proxmox_schema::ObjectSchema::new("Some Description.", &[("field", false, &TEXT_SCHEMA)])
            .additional_properties(true)
            .schema();

    assert_eq!(TEST_UNSPECIFIED, UnspecifiedData::API_SCHEMA);
}
