//! Proxmox API module. This provides utilities for HTTP and command line APIs.

use std::future::Future;

use failure::Error;
use hyper::http::request::Parts;
use hyper::{Body, Response};
use serde_json::Value;

pub mod error;
pub mod router;
pub mod rpc_environment;
pub mod schema;

#[doc(inline)]
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};

#[doc(inline)]
pub use router::ApiMethod;

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
