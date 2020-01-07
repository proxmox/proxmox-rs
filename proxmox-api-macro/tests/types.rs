//! This should test the usage of "external" types. For any unrecognized schema type we expect the
//! type's impl to provide an `pub const API_SCHEMA: &Schema`.

use proxmox::api::schema;
use proxmox_api_macro::api;

use failure::Error;
use serde::Deserialize;
use serde_json::Value;

#[api(
    type: String,
    description: "A string",
    format: &schema::ApiStringFormat::Enum(&["ok", "not-ok"]),
)]
//#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OkString(String);

#[api(
    properties: {
        test: {
            type: String,
            description: "Hello",
        },
    },
)]
/// A Foo.
pub struct Foo {
    test: String,
}

// generates the following without the '_' prefix in the constant:
impl OkString {
    pub const _API_SCHEMA: &'static schema::Schema = &schema::StringSchema::new("A string")
        .format(&schema::ApiStringFormat::Enum(&["ok", "not-ok"]))
        .schema();
}

#[api]
#[derive(Deserialize)]
/// A selection of either a, B or C
pub enum Selection {
    #[serde(rename = "a")]
    A,
    B,
    C,
}

// Initial test:
#[api(
    input: {
        properties: {
            arg: { type: OkString },
            selection: { type: Selection },
        }
    },
    returns: { type: Boolean },
)]
/// Check a string.
///
/// Returns: Whether the string was "ok".
pub fn string_check(arg: Value, selection: Selection) -> Result<bool, Error> {
    let _ = arg;
    let _ = selection;
    panic!("body")
}
