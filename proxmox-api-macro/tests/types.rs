//! This should test the usage of "external" types. For any unrecognized schema type we expect the
//! type's impl to provide an `pub const API_SCHEMA: &Schema`.

use proxmox::api::schema;
use proxmox_api_macro::api;

use failure::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OkString(String);
impl OkString {
    pub const API_SCHEMA: &'static schema::Schema = &schema::StringSchema::new("A string")
        .format(&schema::ApiStringFormat::Enum(&["ok", "not-ok"]))
        .schema();
}

// Initial test:
#[api(
    input: {
        properties: {
            arg: { type: OkString },
        }
    },
    returns: { type: Boolean },
)]
/// Check a string.
///
/// Returns: Whether the string was "ok".
pub fn string_check(arg: OkString) -> Result<bool, Error> {
    Ok(arg.0 == "ok")
}
