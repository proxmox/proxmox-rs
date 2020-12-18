//! Testing the `AllOf` schema on structs and methods.

use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use proxmox::api::schema;
use proxmox_api_macro::api;

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

#[api(
    properties: {
        nv: { type: NameValue },
        it: { type: IndexText },
    },
)]
/// Name, value, index and text.
#[derive(Deserialize, Serialize)]
pub struct Nvit {
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

#[api(
    input: {
        properties: {
            nv: { flatten: true, type: NameValue },
            it: { flatten: true, type: IndexText },
        },
    },
)]
/// Hello method.
pub fn hello(it: IndexText, nv: NameValue) -> Result<(NameValue, IndexText), Error> {
    Ok((nv, it))
}

#[test]
fn hello_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new_full(
        &::proxmox::api::ApiHandler::Sync(&api_function_hello),
        ::proxmox::api::router::ParameterSchema::AllOf(&::proxmox::api::schema::AllOfSchema::new(
            "Hello method.",
            &[&IndexText::API_SCHEMA, &NameValue::API_SCHEMA],
        )),
    );
    assert_eq!(TEST_METHOD, API_METHOD_HELLO);
}

#[api(
    input: {
        properties: {
            nv: { flatten: true, type: NameValue },
            it: { flatten: true, type: IndexText },
            extra: { description: "An extra field." },
        },
    },
)]
/// Extra method.
pub fn with_extra(
    it: IndexText,
    nv: NameValue,
    extra: String,
) -> Result<(NameValue, IndexText, String), Error> {
    Ok((nv, it, extra))
}

#[test]
fn with_extra_schema_check() {
    const INNER_SCHEMA: ::proxmox::api::schema::Schema = ::proxmox::api::schema::ObjectSchema::new(
        "<INNER: Extra method.>",
        &[(
            "extra",
            false,
            &::proxmox::api::schema::StringSchema::new("An extra field.").schema(),
        )],
    )
    .schema();

    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new_full(
        &::proxmox::api::ApiHandler::Sync(&api_function_with_extra),
        ::proxmox::api::router::ParameterSchema::AllOf(&::proxmox::api::schema::AllOfSchema::new(
            "Extra method.",
            &[
                &INNER_SCHEMA,
                &IndexText::API_SCHEMA,
                &NameValue::API_SCHEMA,
            ],
        )),
    );
    assert_eq!(TEST_METHOD, API_METHOD_WITH_EXTRA);
}

struct RpcEnv;
impl proxmox::api::RpcEnvironment for RpcEnv {
    fn result_attrib_mut(&mut self) -> &mut Value {
        panic!("result_attrib_mut called");
    }

    fn result_attrib(&self) -> &Value {
        panic!("result_attrib called");
    }

    /// The environment type
    fn env_type(&self) -> proxmox::api::RpcEnvironmentType {
        panic!("env_type called");
    }

    /// Set authentication id
    fn set_auth_id(&mut self, user: Option<String>) {
        let _ = user;
        panic!("set_auth_id called");
    }

    /// Get authentication id
    fn get_auth_id(&self) -> Option<String> {
        panic!("get_auth_id called");
    }
}

#[test]
fn test_invocations() {
    let mut env = RpcEnv;
    let value = api_function_hello(
        json!({"name":"Bob", "value":3, "index":4, "text":"Text"}),
        &API_METHOD_HELLO,
        &mut env,
    )
    .expect("hello function should work");

    assert_eq!(value[0]["name"], "Bob");
    assert_eq!(value[0]["value"], 3);
    assert_eq!(value[1]["index"], 4);
    assert_eq!(value[1]["text"], "Text");

    let value = api_function_with_extra(
        json!({"name":"Alice", "value":8, "index":2, "text":"Paragraph", "extra":"Some Extra"}),
        &API_METHOD_WITH_EXTRA,
        &mut env,
    )
    .expect("`with_extra` function should work");

    assert_eq!(value[0]["name"], "Alice");
    assert_eq!(value[0]["value"], 8);
    assert_eq!(value[1]["index"], 2);
    assert_eq!(value[1]["text"], "Paragraph");
    assert_eq!(value[2], "Some Extra");
}
