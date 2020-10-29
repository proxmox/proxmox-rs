use proxmox_api_macro::api;

use anyhow::Error;
use serde_json::{json, Value};

#[api(
    input: {
        properties: {
            value: {
                description: "The optional value with default.",
                optional: true,
                default: false,
            }
        }
    }
)]
/// Print the given message.
///
/// Returns: the input.
pub fn test_option(value: bool) -> Result<bool, Error> {
    Ok(value)
}

#[api(
    input: {
        properties: {
            value: {
                description: "The optional value with default.",
                optional: true,
                default: 5,
            }
        }
    }
)]
/// Print the given message.
///
/// Returns: the input.
pub fn test_default_macro(value: Option<isize>) -> Result<isize, Error> {
    Ok(value.unwrap_or(api_get_default!("value")))
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
    let value = api_function_test_option(json!({}), &API_METHOD_TEST_OPTION, &mut env)
        .expect("func with option should work");
    assert_eq!(value, false);

    let value = api_function_test_option(json!({"value": true}), &API_METHOD_TEST_OPTION, &mut env)
        .expect("func with option should work");
    assert_eq!(value, true);

    let value =
        api_function_test_option(json!({"value": false}), &API_METHOD_TEST_OPTION, &mut env)
            .expect("func with option should work");
    assert_eq!(value, false);

    let value =
        api_function_test_default_macro(json!({}), &API_METHOD_TEST_DEFAULT_MACRO, &mut env)
            .expect("func with option should work");
    assert_eq!(value, 5);
}
