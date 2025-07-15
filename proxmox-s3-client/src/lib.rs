//! Low level REST API client for AWS S3 compatible object stores
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod client;
#[cfg(feature = "impl")]
pub use client::{S3Client, S3ClientOptions};
