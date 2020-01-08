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
pub fn create_ticket(param: Value) -> Result<Value, Error> {
    let obj = param.as_object().expect("expected object parameter");
    assert!(obj.contains_key("username"));
    assert!(obj.contains_key("password"));
    let user = obj["username"].as_str().expect("expected a username");
    assert!(obj["password"].as_str().is_some());
    Ok(json!({
        "username": user,
        "ticket": "<TICKET>",
        "CSRFPreventionToken": "<TOKEN>",
    }))
}

#[test]
fn create_ticket_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_create_ticket),
        &::proxmox::api::schema::ObjectSchema::new(
            "Create or verify authentication ticket.",
            &[
                (
                    "password",
                    false,
                    &::proxmox::api::schema::StringSchema::new(
                        "The secret password or a valid ticket.",
                    )
                    .schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox::api::schema::StringSchema::new("User name")
                        .max_length(64)
                        .schema(),
                ),
            ],
        ),
    )
    .returns(
        &::proxmox::api::schema::ObjectSchema::new(
            "A ticket.",
            &[
                (
                    "CSRFPreventionToken",
                    false,
                    &::proxmox::api::schema::StringSchema::new(
                        "Cross Site Request Forgerty Prevention Token.",
                    )
                    .schema(),
                ),
                (
                    "ticket",
                    false,
                    &::proxmox::api::schema::StringSchema::new("Auth ticket.").schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox::api::schema::StringSchema::new("User name.").schema(),
                ),
            ],
        )
        .schema(),
    )
    .protected(true);
    assert_eq!(TEST_METHOD, API_METHOD_CREATE_TICKET);
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

#[test]
fn create_ticket_direct_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_create_ticket_direct),
        &::proxmox::api::schema::ObjectSchema::new(
            "Create or verify authentication ticket.",
            &[
                (
                    "password",
                    false,
                    &::proxmox::api::schema::StringSchema::new(
                        "The secret password or a valid ticket.",
                    )
                    .schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox::api::schema::StringSchema::new("User name")
                        .max_length(64)
                        .schema(),
                ),
            ],
        ),
    )
    .returns(
        &::proxmox::api::schema::ObjectSchema::new(
            "A ticket.",
            &[
                (
                    "CSRFPreventionToken",
                    false,
                    &::proxmox::api::schema::StringSchema::new(
                        "Cross Site Request Forgerty Prevention Token.",
                    )
                    .schema(),
                ),
                (
                    "ticket",
                    false,
                    &::proxmox::api::schema::StringSchema::new("Auth ticket.").schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox::api::schema::StringSchema::new("User name.").schema(),
                ),
            ],
        )
        .schema(),
    )
    .protected(true);
    assert_eq!(TEST_METHOD, API_METHOD_CREATE_TICKET_DIRECT);
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

#[test]
fn func_with_option_schema_check() {
    const TEST_METHOD: ::proxmox::api::ApiMethod = ::proxmox::api::ApiMethod::new(
        &::proxmox::api::ApiHandler::Sync(&api_function_func_with_option),
        &::proxmox::api::schema::ObjectSchema::new(
            "Optional parameter",
            &[(
                "verbose",
                true,
                &::proxmox::api::schema::BooleanSchema::new("Verbose output.").schema(),
            )],
        ),
    )
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_FUNC_WITH_OPTION);
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

    let login = api_function_create_ticket(
        json!({"username":"hello","password":"world"}),
        &API_METHOD_CREATE_TICKET,
        &mut env,
    )
    .expect("expected a ticket");
    let login = login.as_object().expect("expected a valid result");
    assert_eq!(login["username"], "hello");
    assert_eq!(login["ticket"], "<TICKET>");
    assert_eq!(login["CSRFPreventionToken"], "<TOKEN>");

    let login = api_function_create_ticket_direct(
        json!({"username":"hello","password":"world"}),
        &API_METHOD_CREATE_TICKET,
        &mut env,
    )
    .expect("expected a ticket");
    assert_eq!(login, "an:invalid:ticket");
}
