#![allow(dead_code)]

use proxmox::api::{ApiMethod, RpcEnvironment};
use proxmox_api_macro::api;

use failure::Error;
use serde_json::Value;

#[api]
#[input(Object(default: "test", {
    "username": String("User name.").max_length(64),
    "password": String("The secret password or a valid ticket."),
    optional "test": Integer("What?", default: 3),
    "data": Array("Some Integers", Integer("Some Thing").maximum(4)),
}))]
#[returns(Object("Returns a ticket", {
    "username": String("User name."),
    "ticket": String("Auth ticket."),
    "CSRFPreventionToken": String("Cross Site Request Forgerty Prevention Token."),
}))]
/// Create or verify authentication ticket.
///
/// Returns: ...
fn create_ticket(
    _param: Value,
    _info: &ApiMethod,
    _rpcenv: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    panic!("implement me");
}
