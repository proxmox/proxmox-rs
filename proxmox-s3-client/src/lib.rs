//! Low level REST API client for AWS S3 compatible object stores
//!
//! # Example
//! A basic example on how to use the client can be found in
//! `proxmox-s3-client/examples/s3_client.rs` and run via
//! `cargo run --example s3_client --features impl` from the main
//! repository folder.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod aws_sign_v4;
#[cfg(feature = "impl")]
pub use aws_sign_v4::uri_decode;
#[cfg(feature = "impl")]
mod client;
#[cfg(feature = "impl")]
pub use client::{S3Client, S3ClientOptions, S3PathPrefix};
#[cfg(feature = "impl")]
mod timestamps;
#[cfg(feature = "impl")]
pub use timestamps::*;
#[cfg(feature = "impl")]
mod object_key;
#[cfg(feature = "impl")]
pub use object_key::S3ObjectKey;
#[cfg(feature = "impl")]
mod response_reader;
