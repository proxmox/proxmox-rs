//! Proxmox API module. This provides utilities for HTTP and command line APIs.

use std::fmt;
use std::future::Future;

use failure::Error;
use hyper::http::request::Parts;
use hyper::{Body, Response};
use serde_json::Value;

pub mod const_regex;
pub mod error;
pub mod format;
pub mod router;
pub mod rpc_environment;
pub mod schema;

#[doc(inline)]
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};

#[doc(inline)]
pub use router::Router;

#[doc(inline)]
pub use error::HttpError;

/// A synchronous API handler gets a json Value as input and returns a json Value as output.
pub type ApiHandlerFn = &'static (dyn Fn(Value, &ApiMethod, &mut dyn RpcEnvironment) -> Result<Value, Error>
              + Send
              + Sync
              + 'static);

/// Asynchronous API handlers get more lower level access to request data.
pub type ApiAsyncHandlerFn = &'static (dyn Fn(
    Parts,
    Body,
    Value,
    &'static ApiMethod,
    Box<dyn RpcEnvironment>,
) -> Result<ApiFuture, Error>
              + Send
              + Sync
              + 'static);

/// The output of an asynchronous API handler is a futrue yielding a `Response`.
pub type ApiFuture = Box<dyn Future<Output = Result<Response<Body>, failure::Error>> + Send>;

pub enum ApiHandler {
    Sync(ApiHandlerFn),
    Async(ApiAsyncHandlerFn),
}

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
