//! This should test the usage of "external" schemas. If a property is declared with a path instead
//! of an object, we expect the path to lead to a schema.

use proxmox::api::{schema, RpcEnvironment};
use proxmox_api_macro::api;

use failure::Error;
use serde_json::{json, Value};

pub const NAME_SCHEMA: schema::Schema = schema::StringSchema::new("Archive name.")
    //.format(&FILENAME_FORMAT)
    .schema();

#[api(
    input: {
        properties: {
            "archive-name": {
                schema: NAME_SCHEMA,
            }
        }
    }
)]
/// Get an archive.
pub fn get_archive(archive_name: String) {
    let _ = archive_name;
}

#[api(
    input: {
        properties: {
            "archive-name": {
                schema: NAME_SCHEMA,
            }
        }
    }
)]
/// Get an archive.
pub fn get_archive_2(param: Value, rpcenv: &mut dyn RpcEnvironment) -> Result<Value, Error> {
    let _ = param;
    let _ = rpcenv;
    Ok(json!("test"))
}

#[api(
    input: {
        properties: {
            "data": {
                description: "The data",
                type: Array,
                items: {
                    schema: NAME_SCHEMA,
                }
            }
        }
    }
)]
/// Get data.
pub fn get_data(param: Value) -> Result<(), Error> {
    let _ = param;
    Ok(())
}