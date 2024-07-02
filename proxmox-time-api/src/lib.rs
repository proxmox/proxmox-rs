#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod time_impl;
#[cfg(feature = "impl")]
pub use time_impl::*;
