//! Proxmox API module. This provides utilities for HTTP and command line APIs.
//!
//! The main component here is the [`Router`] which is filled with entries pointing to
//! [`ApiMethodInfos`](crate::ApiMethodInfo).
//!
//! Note that you'll rarely need the [`Router`] type itself, as you'll most likely be creating them
//! with the `router` macro provided by the `proxmox-api-macro` crate.

// This saves us some repetition (which we could do via macros), but this makes the code shorter
// and easier to review.
// FIXME: While the RFC has been approved a while ago and the implementation is there, there isn't
// much activity on the issue tracker for this, so should we remove this?
// Currently this is only used in cli.rs for a `default fn` which could instead made explicit for
// our types
#![feature(specialization)]

use std::future::Future;
use std::pin::Pin;

use failure::Error;
use http::Response;

mod api_output;
pub use api_output::*;

mod api_type;
pub use api_type::*;

mod router;
pub use router::*;

pub mod cli;

/// Return type of an API method.
pub type ApiOutput<Body> = Result<Response<Body>, Error>;

/// Future type of an API method. In order to support `async fn` this is a pinned box.
pub type ApiFuture<Body> = Pin<Box<dyn Future<Output = ApiOutput<Body>> + Send>>;
