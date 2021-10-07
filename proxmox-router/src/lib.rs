//! API Router and Command Line Interface utilities.

pub mod format;

#[cfg(feature = "cli")]
pub mod cli;

// this is public so the `http_err!` macro can access `http::StatusCode` through it
#[doc(hidden)]
pub mod error;

mod permission;
mod router;
mod rpc_environment;

#[doc(inline)]
pub use error::HttpError;

pub use permission::*;
pub use router::*;
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};

// make list_subdirs_api_method! work without an explicit proxmox-schema dependency:
#[doc(hidden)]
pub use proxmox_schema::ObjectSchema as ListSubdirsObjectSchema;
