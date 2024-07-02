#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod config;
#[cfg(feature = "impl")]
pub use config::*;

#[cfg(feature = "impl")]
mod api_impl;
#[cfg(feature = "impl")]
pub use api_impl::{create_interface, update_interface};
