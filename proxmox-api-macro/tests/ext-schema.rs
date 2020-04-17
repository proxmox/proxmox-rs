//! This should test the usage of "external" schemas. If a property is declared with a path instead
//! of an object, we expect the path to lead to a schema.

use proxmox::api::{schema, RpcEnvironment};
use proxmox_api_macro::api;

use anyhow::Error;
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

#[test]
fn get_archive_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_get_archive),
        &::proxmox::api::schema::ObjectSchema::new(
            "Get an archive.",
            &[("archive-name", false, &NAME_SCHEMA)],
        ),
    )
    .protected(false);
    assert_eq!(TEST_METHOD, API_METHOD_GET_ARCHIVE);
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

#[test]
fn get_archive_2_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_get_archive_2),
        &::proxmox::api::schema::ObjectSchema::new(
            "Get an archive.",
            &[("archive-name", false, &NAME_SCHEMA)],
        ),
    )
    .protected(false);
    assert_eq!(TEST_METHOD, API_METHOD_GET_ARCHIVE_2);
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

#[test]
fn get_data_schema_test() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_get_data),
        &::proxmox::api::schema::ObjectSchema::new(
            "Get data.",
            &[(
                "data",
                false,
                &::proxmox::api::schema::ArraySchema::new("The data", &NAME_SCHEMA).schema(),
            )],
        ),
    )
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_GET_DATA);
}
