#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;

use std::cell::RefCell;

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

/// Produce a compile error which does not immediately abort.
macro_rules! error {
    ($($msg:tt)*) => {{ crate::add_error(format_err!($($msg)*)); }}
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
mod updater;
mod util;

/// Handle errors by appending a `compile_error!()` macro invocation to the original token stream.
fn handle_error(mut item: TokenStream, data: Result<TokenStream, Error>) -> TokenStream {
    let mut data = match data {
        Ok(output) => output,
        Err(err) => match err.downcast::<syn::Error>() {
            Ok(err) => {
                item.extend(err.to_compile_error());
                item
            }
            Err(err) => panic!("error in api/router macro: {}", err),
        },
    };
    data.extend(take_non_fatal_errors());
    data
}

/// TODO!
#[proc_macro]
pub fn router(item: TokenStream_1) -> TokenStream_1 {
    let _error_guard = init_local_error();
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
    # use proxmox_router::{ApiMethod, RpcEnvironment};

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
        pub const API_SCHEMA: &'static ::proxmox_schema::Schema =
            &::proxmox_schema::ObjectSchema::new(
                "An example of a struct with renamed fields.",
                &[
                    (
                        "test-string",
                        false,
                        &::proxmox_schema::StringSchema::new("A test string.").schema(),
                    ),
                    (
                        "SomeOther",
                        true,
                        &::proxmox_schema::StringSchema::new(
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

    # Deriving an `Updater`:

    An "Updater" struct can be generated automatically for a type. This affects the `UpdaterType`
    trait implementation generated, as it will set the associated
    `type Updater = TheDerivedUpdater`.

    In order to do this, simply add `#[derive(Updater)]` to the `#[api]`-macro using api type.
    This is only supported for `struct`s with named fields and will generate a new `struct` whose
    name is suffixed with `Updater` containing the `Updater` types of each field as a member.

    Additionally the `#[updater(fixed)]` option is available to make it illegal for an updater to
    modify a field (generating an error if it is set), while still allowing it to be used to create
    a new object via the `build_from()` method.

    ```ignore
    #[api]
    /// An example of a simple struct type.
    #[derive(Updater)]
    pub struct MyType {
        /// A string.
        one: String,

        /// An optional string.
        /// Note that using `Option::is_empty` for the serde attribute only works for types which
        /// use an `Option` as their `Updater`. For a `String` this works. Otherwise we'd have to
        /// use `Updater::is_empty` instead.
        #[serde(skip_serializing_if = "Option::is_none")]
        opt: Option<String>,
    }
    ```

    The above will automatically generate the following:
    ```ignore
    #[api]
    /// An example of a simple struct type.
    pub struct MyTypeUpdater {
        one: Option<String>, // really <String as UpdaterType>::Updater

        #[serde(skip_serializing_if = "Option::is_none")]
        opt: Option<String>, // really <Option<String> as UpdaterType>::Updater
    }

    impl Updater for MyTypeUpdater {
        fn is_empty(&self) -> bool {
            self.one.is_empty() && self.opt.is_empty()
        }
    }

    impl UpdaterType for MyType {
        type Updater = MyTypeUpdater;
    }

    ```
*/
#[proc_macro_attribute]
pub fn api(attr: TokenStream_1, item: TokenStream_1) -> TokenStream_1 {
    let _error_guard = init_local_error();
    let item: TokenStream = item.into();
    handle_error(item.clone(), api::api(attr.into(), item)).into()
}

/// This is a dummy derive macro actually handled by `#[api]`!
#[doc(hidden)]
#[proc_macro_derive(Updater, attributes(updater, serde))]
pub fn derive_updater(_item: TokenStream_1) -> TokenStream_1 {
    TokenStream_1::new()
}

/// Create the default `UpdaterType` implementation as an `Option<Self>`.
#[proc_macro_derive(UpdaterType, attributes(updater_type, serde))]
pub fn derive_updater_type(item: TokenStream_1) -> TokenStream_1 {
    let _error_guard = init_local_error();
    let item: TokenStream = item.into();
    handle_error(
        item.clone(),
        updater::updater_type(item).map_err(Error::from),
    )
    .into()
}

thread_local!(static NON_FATAL_ERRORS: RefCell<Option<TokenStream>> = RefCell::new(None));

/// The local error TLS must be freed at the end of a macro as any leftover `TokenStream` (even an
/// empty one) will just panic between different runs as the multiple source files are handled by
/// the same compiler thread.
struct LocalErrorGuard;

impl Drop for LocalErrorGuard {
    fn drop(&mut self) {
        NON_FATAL_ERRORS.with(|errors| {
            *errors.borrow_mut() = None;
        });
    }
}

fn init_local_error() -> LocalErrorGuard {
    NON_FATAL_ERRORS.with(|errors| {
        *errors.borrow_mut() = Some(TokenStream::new());
    });
    LocalErrorGuard
}

pub(crate) fn add_error(err: syn::Error) {
    NON_FATAL_ERRORS.with(|errors| {
        errors
            .borrow_mut()
            .as_mut()
            .expect("missing call to init_local_error")
            .extend(err.to_compile_error())
    });
}

pub(crate) fn take_non_fatal_errors() -> TokenStream {
    NON_FATAL_ERRORS.with(|errors| {
        errors
            .borrow_mut()
            .take()
            .expect("missing call to init_local_mut")
    })
}
