//! Proxmox API module. This provides utilities for HTTP and command line APIs.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use failure::Error;
use hyper::http::request::Parts;
use hyper::{Body, Response};
use serde_json::Value;

#[doc(hidden)]
pub mod const_regex;
#[doc(hidden)]
pub mod error;
pub mod format;
#[doc(hidden)]
pub mod router;
#[doc(hidden)]
pub mod rpc_environment;
pub mod schema;

use schema::{ObjectSchema, Schema};

#[doc(inline)]
pub use const_regex::ConstRegexPattern;

#[doc(inline)]
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};

#[doc(inline)]
pub use router::{Router, SubRoute, SubdirMap};

#[doc(inline)]
pub use error::HttpError;

/// A synchronous API handler gets a json Value as input and returns a json Value as output.
///
/// Most API handler are synchronous. Use this to define such handler:
/// ```
/// # use failure::*;
/// # use serde_json::{json, Value};
/// # use proxmox_api::{*, schema::*};
/// #
/// fn hello(
///    param: Value,
///    info: &ApiMethod,
///    rpcenv: &mut dyn RpcEnvironment,
/// ) -> Result<Value, Error> {
///    Ok(json!("Hello world!"))
/// }
///
/// const API_METHOD_HELLO: ApiMethod = ApiMethod::new(
///    &ApiHandler::Sync(&hello),
///    &ObjectSchema::new("Hello World Example", &[])
/// );
/// ```
pub type ApiHandlerFn = &'static (dyn Fn(Value, &ApiMethod, &mut dyn RpcEnvironment) -> Result<Value, Error>
              + Send
              + Sync
              + 'static);

/// Asynchronous HTTP API handlers
///
/// They get low level access to request and response data. Use this
/// to implement custom upload/download functions.
/// ```
/// # use failure::*;
/// # use serde_json::{json, Value};
/// # use proxmox_api::{*, schema::*};
/// #
/// use futures::*;
/// use hyper::{Body, Response, http::request::Parts};
///
/// fn low_level_hello(
///    parts: Parts,
///    req_body: Body,
///    param: Value,
///    info: &ApiMethod,
///    rpcenv: Box<dyn RpcEnvironment>,
/// ) -> ApiFuture {
///    async move {
///        let response = http::Response::builder()
///            .status(200)
///            .body(Body::from("Hello world!"))?;
///        Ok(response)
///    }.boxed()
/// }
///
/// const API_METHOD_LOW_LEVEL_HELLO: ApiMethod = ApiMethod::new(
///    &ApiHandler::AsyncHttp(&low_level_hello),
///    &ObjectSchema::new("Hello World Example (low level)", &[])
/// );
/// ```
pub type ApiAsyncHttpHandlerFn = &'static (dyn Fn(Parts, Body, Value, &'static ApiMethod, Box<dyn RpcEnvironment>) -> ApiFuture
              + Send
              + Sync
              + 'static);

/// The output of an asynchronous API handler is a futrue yielding a `Response`.
pub type ApiFuture = Pin<Box<dyn Future<Output = Result<Response<Body>, failure::Error>> + Send>>;

/// Enum for different types of API handler functions.
pub enum ApiHandler {
    Sync(ApiHandlerFn),
    AsyncHttp(ApiAsyncHttpHandlerFn),
}

const NULL_SCHEMA: Schema = Schema::Null;

fn dummy_handler_fn(
    _arg: Value,
    _method: &ApiMethod,
    _env: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    // do nothing
    Ok(Value::Null)
}

const DUMMY_HANDLER: ApiHandler = ApiHandler::Sync(&dummy_handler_fn);

/// This struct defines synchronous API call which returns the restulkt as json `Value`
pub struct ApiMethod {
    /// The protected flag indicates that the provides function should be forwarded
    /// to the deaemon running in priviledged mode.
    pub protected: bool,
    /// This flag indicates that the provided method may change the local timezone, so the server
    /// should do a tzset afterwards
    pub reload_timezone: bool,
    /// Parameter type Schema
    pub parameters: &'static schema::ObjectSchema,
    /// Return type Schema
    pub returns: &'static schema::Schema,
    /// Handler function
    pub handler: &'static ApiHandler,
}

impl std::fmt::Debug for ApiMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ApiMethod {{ ")?;
        write!(f, "  parameters: {:?}", self.parameters)?;
        write!(f, "  returns: {:?}", self.returns)?;
        write!(f, "  handler: {:p}", &self.handler)?;
        write!(f, "}}")
    }
}

impl ApiMethod {
    pub const fn new(handler: &'static ApiHandler, parameters: &'static ObjectSchema) -> Self {
        Self {
            parameters,
            handler,
            returns: &NULL_SCHEMA,
            protected: false,
            reload_timezone: false,
        }
    }

    pub const fn new_dummy(parameters: &'static ObjectSchema) -> Self {
        Self {
            parameters,
            handler: &DUMMY_HANDLER,
            returns: &NULL_SCHEMA,
            protected: false,
            reload_timezone: false,
        }
    }

    pub const fn returns(mut self, schema: &'static Schema) -> Self {
        self.returns = schema;

        self
    }

    pub const fn protected(mut self, protected: bool) -> Self {
        self.protected = protected;

        self
    }

    pub const fn reload_timezone(mut self, reload_timezone: bool) -> Self {
        self.reload_timezone = reload_timezone;

        self
    }
}
