#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;

use anyhow::Error;

use proc_macro::TokenStream as TokenStream_1;
use proc_macro2::TokenStream;

/// Our `format_err` macro replacement to enforce the inclusion of a `Span`.
/// The arrow variant takes a spanned syntax element, the comma variant expects an actual `Span` as
/// first parameter.
macro_rules! format_err {
    ($span:expr => $($msg:tt)*) => { syn::Error::new_spanned($span, format!($($msg)*)) };
    ($span:expr, $($msg:tt)*) => { syn::Error::new($span, format!($($msg)*)) };
}

/// Our `bail` macro replacement to enforce the inclusion of a `Span`.
/// The arrow variant takes a spanned syntax element, the comma variant expects an actual `Span` as
/// first parameter.
macro_rules! bail {
    ($span:expr => $($msg:tt)*) => { return Err(format_err!($span => $($msg)*).into()) };
    ($span:expr, $($msg:tt)*) => { return Err(format_err!($span, $($msg)*).into()) };
}

mod api;
mod serde;
mod util;

/// Handle errors by appending a `compile_error!()` macro invocation to the original token stream.
fn handle_error(mut item: TokenStream, data: Result<TokenStream, Error>) -> TokenStream {
    match data {
        Ok(output) => output,
        Err(err) => match err.downcast::<syn::Error>() {
            Ok(err) => {
                item.extend(err.to_compile_error());
                item
            }
            Err(err) => panic!("error in api/router macro: {}", err),
        },
    }
}

/// TODO!
#[proc_macro]
pub fn router(item: TokenStream_1) -> TokenStream_1 {
    let item: TokenStream = item.into();
    handle_error(item.clone(), router_do(item)).into()
}

fn router_do(item: TokenStream) -> Result<TokenStream, Error> {
    Ok(item)
}

/**
    Macro for building API methods and types:

    ```
    # use proxmox_api_macro::api;
    # use proxmox::api::{ApiMethod, RpcEnvironment};

    use anyhow::Error;
    use serde_json::Value;

    #[api(
        input: {
            type: Object,
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
            type: Object,
            description: "Returns a ticket",
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
    )]
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
    ```

    The above code expands to:

    ```ignore
    const API_METHOD_CREATE_TICKET: ApiMethod =
        ApiMethod::new(
            &ApiHandler::Sync(&create_ticket),
            &ObjectSchema::new(
                "Create or verify authentication ticket",
                &[ // Sorted:
                    ("password", false, &StringSchema::new("The secret password or a valid ticket.")
                        .schema()),
                    ("username", false, &StringSchema::new("User name.")
                        .max_length(64)
                        .schema()),
                ]
            )
        )
        .returns(
            &ObjectSchema::new(
            )
        )
        .protected(false);
    fn create_ticket(
        param: Value,
        info: &ApiMethod,
        rpcenv: &mut dyn RpcEnvironment,
    ) -> Result<Value, Error> {
        ...
    }
    ```

    The `#[api]` macro can also be used on type declarations to create schemas for structs to be
    used instead of accessing json values via string indexing.

    For a simple struct, the schema can be left empty and will be completely derived from the
    information available in rust:

    ```no_run
    # use proxmox_api_macro::api;
    # use serde::{Deserialize, Serialize};
    #[api]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    /// An example of a struct with renamed fields.
    pub struct RenamedStruct {
        /// A test string.
        test_string: String,

        /// An optional auto-derived value for testing:
        #[serde(rename = "SomeOther")]
        another: Option<String>,
    }
    ```

    This will produce the following schema:
    ```no_run
    # struct RenamedStruct;
    impl RenamedStruct {
        pub const API_SCHEMA: &'static ::proxmox::api::schema::Schema =
            &::proxmox::api::schema::ObjectSchema::new(
                "An example of a struct with renamed fields.",
                &[
                    (
                        "test-string",
                        false,
                        &::proxmox::api::schema::StringSchema::new("A test string.").schema(),
                    ),
                    (
                        "SomeOther",
                        true,
                        &::proxmox::api::schema::StringSchema::new(
                            "An optional auto-derived value for testing:",
                        )
                        .schema(),
                    ),
                ],
            )
            .schema();
    }
    ```

    Note that when writing out parts or all of the schema manually, the schema itself has to
    contain the already renamed fields!

    ```
    # use proxmox_api_macro::api;
    # use serde::{Deserialize, Serialize};
    #[api(
        properties: {
            "A-RENAMED-FIELD": { description: "Some description.", },
            "another": { description: "Some description.", },
        },
    )]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "SCREAMING-KEBAB-CASE")]
    /// Some Description.
    pub struct SomeStruct {
        a_renamed_field: String,
        #[serde(rename = "another")]
        AND_MORE: String,
    }
    ```

    There are a few shortcuts for schemas: if the `type` refers to an arbitrary rust type other
    than strings or integers, we assume that it has an `impl` block containing a `pub const
    API_SCHEMA: &'static Schema`. This is what the `#[api]` macro produces on `struct` and `enum`
    declarations. If it contains a `schema` key, this is expected to be the path to an existing
    schema. (Hence `type: Foo` is the same as `schema: Foo::API_SCHEMA`.)

*/
#[proc_macro_attribute]
pub fn api(attr: TokenStream_1, item: TokenStream_1) -> TokenStream_1 {
    let item: TokenStream = item.into();
    handle_error(item.clone(), api::api(attr.into(), item)).into()
}
