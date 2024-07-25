#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

pub use deflate::{
    DeflateDecoder, DeflateDecoderBuilder, DeflateEncoder, DeflateEncoderBuilder, Level,
};

mod deflate;
pub mod tar;
pub mod zip;
pub mod zstd;
