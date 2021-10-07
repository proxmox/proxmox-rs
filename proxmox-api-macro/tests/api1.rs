use proxmox_api_macro::api;

use anyhow::Error;
use serde_json::{json, Value};

use proxmox_router::Permission;

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
    access: {
        description: "Only root can access this.",
        permission: &Permission::Superuser,
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
    const TEST_METHOD: ::proxmox_router::ApiMethod = ::proxmox_router::ApiMethod::new(
        &::proxmox_router::ApiHandler::Sync(&api_function_create_ticket),
        &::proxmox_schema::ObjectSchema::new(
            "Create or verify authentication ticket.",
            &[
                (
                    "password",
                    false,
                    &::proxmox_schema::StringSchema::new(
                        "The secret password or a valid ticket.",
                    )
                    .schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox_schema::StringSchema::new("User name")
                        .max_length(64)
                        .schema(),
                ),
            ],
        ),
    )
    .returns(::proxmox_schema::ReturnType::new(
        false,
        &::proxmox_schema::ObjectSchema::new(
            "A ticket.",
            &[
                (
                    "CSRFPreventionToken",
                    false,
                    &::proxmox_schema::StringSchema::new(
                        "Cross Site Request Forgerty Prevention Token.",
                    )
                    .schema(),
                ),
                (
                    "ticket",
                    false,
                    &::proxmox_schema::StringSchema::new("Auth ticket.").schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox_schema::StringSchema::new("User name.").schema(),
                ),
            ],
        )
        .schema(),
    ))
    .access(Some("Only root can access this."), &Permission::Superuser)
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
    access: {
        permission: &Permission::World,
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
    const TEST_METHOD: ::proxmox_router::ApiMethod = ::proxmox_router::ApiMethod::new(
        &::proxmox_router::ApiHandler::Sync(&api_function_create_ticket_direct),
        &::proxmox_schema::ObjectSchema::new(
            "Create or verify authentication ticket.",
            &[
                (
                    "password",
                    false,
                    &::proxmox_schema::StringSchema::new(
                        "The secret password or a valid ticket.",
                    )
                    .schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox_schema::StringSchema::new("User name")
                        .max_length(64)
                        .schema(),
                ),
            ],
        ),
    )
    .returns(::proxmox_schema::ReturnType::new(
        false,
        &::proxmox_schema::ObjectSchema::new(
            "A ticket.",
            &[
                (
                    "CSRFPreventionToken",
                    false,
                    &::proxmox_schema::StringSchema::new(
                        "Cross Site Request Forgerty Prevention Token.",
                    )
                    .schema(),
                ),
                (
                    "ticket",
                    false,
                    &::proxmox_schema::StringSchema::new("Auth ticket.").schema(),
                ),
                (
                    "username",
                    false,
                    &::proxmox_schema::StringSchema::new("User name.").schema(),
                ),
            ],
        )
        .schema(),
    ))
    .access(None, &Permission::World)
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
    const TEST_METHOD: ::proxmox_router::ApiMethod = ::proxmox_router::ApiMethod::new(
        &::proxmox_router::ApiHandler::Sync(&api_function_func_with_option),
        &::proxmox_schema::ObjectSchema::new(
            "Optional parameter",
            &[(
                "verbose",
                true,
                &::proxmox_schema::BooleanSchema::new("Verbose output.").schema(),
            )],
        ),
    )
    .protected(false);

    assert_eq!(TEST_METHOD, API_METHOD_FUNC_WITH_OPTION);
}

struct RpcEnv;
impl proxmox_router::RpcEnvironment for RpcEnv {
    fn result_attrib_mut(&mut self) -> &mut Value {
        panic!("result_attrib_mut called");
    }

    fn result_attrib(&self) -> &Value {
        panic!("result_attrib called");
    }

    /// The environment type
    fn env_type(&self) -> proxmox_router::RpcEnvironmentType {
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
