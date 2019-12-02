use proxmox_api_macro::api;

use failure::Error;
use serde_json::Value;

#[api(
    input: {
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
    },
    returns: {
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
    },
    protected: true,
)]
/// Create or verify authentication ticket.
///
/// Returns: A ticket.
pub fn create_ticket(_param: Value) -> Result<Value, Error> {
    panic!("implement me");
}

#[api(
    input: {
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
    },
    returns: {
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
    },
    protected: true,
)]
/// Create or verify authentication ticket.
///
/// Returns: A ticket.
pub fn create_ticket_direct(username: String, password: String) -> Result<&'static str, Error> {
    let _ = username;
    let _ = password;
    // This sill not pass the schema's output validation, but the code still compiles as this can
    // successfully produce a `serde_json::Value`!
    Ok("an:invalid:ticket")
}

#[api(
    input: {
        properties: {
            verbose: {
                type: Boolean,
                description: "Verbose output.",
            },
        },
    },
)]
/// Test something
pub fn some_call(verbose: bool) -> Result<(), Error> {
    let _ = verbose;
    Ok(())
}

#[api]
/// Basic function
pub fn basic_function() -> Result<(), Error> {
    Ok(())
}
