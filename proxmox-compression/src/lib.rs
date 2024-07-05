#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

pub use deflate::{DeflateEncoder, Level};

mod deflate;
pub mod tar;
pub mod zip;
pub mod zstd;
