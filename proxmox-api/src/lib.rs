//! Proxmox API module. This provides utilities for HTTP and command line APIs.

use std::future::Future;
use std::pin::Pin;

use failure::Error;
use http::Response;

/// Return type of an API method.
pub type ApiOutput<Body> = Result<Response<Body>, Error>;

/// Future type of an API method. In order to support `async fn` this is a pinned box.
pub type ApiFuture<Body> = Pin<Box<dyn Future<Output = ApiOutput<Body>> + Send>>;
