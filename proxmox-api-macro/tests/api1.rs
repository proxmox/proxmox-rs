use proxmox_api_macro::api;

use failure::Error;
use serde_json::{json, Value};

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

#[api(
    input: {
        properties: {
            verbose: {
                type: Boolean,
                optional: true,
                description: "Verbose output.",
            },
        },
    },
)]
/// Optional parameter
pub fn func_with_option(verbose: Option<bool>) -> Result<(), Error> {
    let _ = verbose;
    Ok(())
}

struct RpcEnv;
impl proxmox::api::RpcEnvironment for RpcEnv {
    fn set_result_attrib(&mut self, name: &str, value: Value) {
        let _ = (name, value);
        panic!("set_result_attrib called");
    }

    /// Query additional result data.
    fn get_result_attrib(&self, name: &str) -> Option<&Value> {
        let _ = name;
        panic!("get_result_attrib called");
    }

    /// The environment type
    fn env_type(&self) -> proxmox::api::RpcEnvironmentType {
        panic!("env_type called");
    }

    /// Set user name
    fn set_user(&mut self, user: Option<String>) {
        let _ = user;
        panic!("set_user called");
    }

    /// Get user name
    fn get_user(&self) -> Option<String> {
        panic!("get_user called");
    }
}

#[test]
fn test_invocations() {
    let mut env = RpcEnv;
    api_function_func_with_option(json!({}), &API_METHOD_FUNC_WITH_OPTION, &mut env)
        .expect("func with option should work");
    api_function_func_with_option(
        json!({ "verbose": true }),
        &API_METHOD_FUNC_WITH_OPTION,
        &mut env,
    )
    .expect("func with option should work");
}
