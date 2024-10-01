#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "u2f")]
pub mod u2f;

#[cfg(feature = "totp")]
pub mod totp;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "types")]
mod types;
#[cfg(feature = "types")]
pub use types::{TfaInfo, TfaType, TfaUpdateInfo, TfaUser, TypedTfaInfo};
