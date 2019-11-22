#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;

use failure::Error;

use proc_macro::TokenStream as TokenStream_1;
use proc_macro2::TokenStream;

macro_rules! format_err {
    ($span:expr => $($msg:tt)*) => { syn::Error::new_spanned($span, format!($($msg)*)) };
    ($span:expr, $($msg:tt)*) => { syn::Error::new($span, format!($($msg)*)) };
}

macro_rules! bail {
    ($span:expr => $($msg:tt)*) => { return Err(format_err!($span => $($msg)*).into()) };
    ($span:expr, $($msg:tt)*) => { return Err(format_err!($span, $($msg)*).into()) };
}

mod api;

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

/// Macro for building a Router:
///
/// ```ignore
/// router! {
///     pub const ROUTER = {
///         "access": {
///             "ticket": {
///                 post = create_ticket,
///             }
///         }
///     };
/// }
///
/// #[api]
/// fn create_ticket(param: Value) -> Result<Value, Error> { ... }
/// ```
#[proc_macro]
pub fn router(item: TokenStream_1) -> TokenStream_1 {
    let item: TokenStream = item.into();
    handle_error(item.clone(), router_do(item)).into()
}

fn router_do(item: TokenStream) -> Result<TokenStream, Error> {
    Ok(item)
}

/**
    Macro for building an API method:

    ```
    # use proxmox_api_macro::api;
    # use proxmox::api::{ApiMethod, RpcEnvironment};

    use failure::Error;
    use serde_json::Value;

    #[api]
    #[input(Object({
        "username": String("User name.").max_length(64),
        "password": String("The secret password or a valid ticket."),
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
*/
#[proc_macro_attribute]
pub fn api(attr: TokenStream_1, item: TokenStream_1) -> TokenStream_1 {
    let item: TokenStream = item.into();
    handle_error(item.clone(), api::api(attr.into(), item)).into()
}
