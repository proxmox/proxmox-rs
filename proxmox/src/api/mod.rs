//! Proxmox API module.
//!
//! This provides utilities to define APIs in a declarative way using
//! Schemas. Primary use case it to define REST/HTTP APIs. Another use case
//! is to define command line tools using Schemas. Finally, it is
//! possible to use schema definitions to derive configuration file
//! parsers.

#[cfg(feature = "api-macro")]
pub use proxmox_api_macro::{api, router};

#[doc(hidden)]
pub mod const_regex;
#[doc(hidden)]
pub mod error;
pub mod schema;
pub mod section_config;

#[doc(inline)]
pub use const_regex::ConstRegexPattern;

#[doc(inline)]
pub use error::HttpError;

#[cfg(any(feature = "router", feature = "cli"))]
#[doc(hidden)]
pub mod rpc_environment;

#[cfg(any(feature = "router", feature = "cli"))]
#[doc(inline)]
pub use rpc_environment::{RpcEnvironment, RpcEnvironmentType};

#[cfg(feature = "router")]
pub mod format;

#[cfg(feature = "router")]
#[doc(hidden)]
pub mod router;

#[cfg(feature = "router")]
#[doc(inline)]
pub use router::{
    ApiFuture, ApiHandler, ApiMethod, ApiResponseFuture, Router, SubRoute, SubdirMap,
};

#[cfg(feature = "cli")]
pub mod cli;
