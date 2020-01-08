use proxmox_api_macro::api;

use failure::Error;
use serde_json::Value;

#[api(
    input: {
        properties: {
            message: {
                description: "The message to print",
            }
        }
    }
)]
/// Print the given message.
pub fn hello(message: String) -> Result<(), Error> {
    println!("Hello there. {}", message);
    Ok(())
}

#[api(
    input: {
        properties: {
            num: {
                description: "The version to upgrade to",
            },
        },
    },
)]
/// Return the number...
pub async fn number(num: u32) -> Result<u32, Error> {
    Ok(num)
}

#[test]
fn number_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Async(&api_function_number),
        &::proxmox::api::schema::ObjectSchema::new(
            "Return the number...",
            &[(
                "num",
                false,
                &::proxmox::api::schema::IntegerSchema::new("The version to upgrade to").schema(),
            )],
        ),
    )
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_NUMBER);
}

#[api(
    input: {
        properties: {
            foo: {
                type: String,
                description: "The great Foo",
            },
            bar: {
                type: String,
                description: "The great Bar",
            },
        },
    },
)]
/// Return the number...
pub async fn more_async_params(param: Value) -> Result<(), Error> {
    let _ = param;
    Ok(())
}

#[test]
fn more_async_params_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Async(&api_function_more_async_params),
        &::proxmox::api::schema::ObjectSchema::new(
            "Return the number...",
            &[
                (
                    "bar",
                    false,
                    &::proxmox::api::schema::StringSchema::new("The great Bar").schema(),
                ),
                (
                    "foo",
                    false,
                    &::proxmox::api::schema::StringSchema::new("The great Foo").schema(),
                ),
            ],
        ),
    )
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_MORE_ASYNC_PARAMS);
}
