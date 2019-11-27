#![allow(dead_code)]

use proxmox::api::{ApiMethod, RpcEnvironment};
use proxmox_api_macro::api;

use failure::Error;
use serde_json::Value;

#[api]
#[input({
    properties: {
        username: {
            type: String,
            description: "User name",
            max_length: 64,
        },
        password: {
            type: String,
            description: "The secret password or a valid ticket.",
        },
    }
})]
#[returns({
    properties: {
        "username": {
            type: String,
            description: "User name.",
        },
        "ticket": {
            type: String,
            description: "Auth ticket.",
        },
        "CSRFPreventionToken": {
            type: String,
            description: "Cross Site Request Forgerty Prevention Token.",
        },
    },
})]
#[protected]
/// Create or verify authentication ticket.
///
/// Returns: A ticket.
fn create_ticket(
    _param: Value,
    _info: &ApiMethod,
    _rpcenv: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    panic!("implement me");
}
