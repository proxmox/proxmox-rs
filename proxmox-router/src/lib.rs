//! API Router and Command Line Interface utilities.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

pub mod format;

#[cfg(feature = "cli")]
pub mod cli;

// this is public so the `http_err!` macro can access `http::StatusCode` through it
#[doc(hidden)]
#[cfg(feature = "server")]
pub mod error;

mod permission;
mod router;
mod rpc_environment;
mod serializable_return;

#[doc(inline)]
#[cfg(feature = "server")]
pub use error::*;

pub use permission::*;
pub use router::*;
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};
pub use serializable_return::SerializableReturn;

// make list_subdirs_api_method! work without an explicit proxmox-schema dependency:
#[doc(hidden)]
pub use proxmox_schema::ObjectSchema as ListSubdirsObjectSchema;

pub mod stream;
