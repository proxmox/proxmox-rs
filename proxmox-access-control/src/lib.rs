#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

pub mod types;

#[cfg(feature = "acl")]
pub mod acl;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "acl")]
pub mod init;

#[cfg(feature = "impl")]
pub mod token_shadow;

#[cfg(feature = "impl")]
pub mod user;

#[cfg(feature = "impl")]
mod cached_user_info;
#[cfg(feature = "impl")]
pub use cached_user_info::CachedUserInfo;
