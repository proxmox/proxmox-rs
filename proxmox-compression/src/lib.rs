#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod compression;
pub use compression::*;

pub mod tar;
pub mod zip;
pub mod zstd;
