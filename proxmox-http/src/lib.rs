//! HTTP related utilities used by various Proxmox products.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(any(feature = "http-helpers", feature = "api-types"))]
pub mod uri;

#[cfg(any(feature = "http-helpers", feature = "api-types"))]
pub mod proxy_config;
#[cfg(feature = "api-types")]
pub use proxy_config::HTTP_PROXY_SCHEMA;
#[cfg(any(feature = "http-helpers", feature = "api-types"))]
pub use proxy_config::ProxyConfig;

#[cfg(feature = "http-helpers")]
mod http_options;
#[cfg(feature = "http-helpers")]
pub use http_options::HttpOptions;

#[cfg(any(feature = "client", feature = "client-sync"))]
pub mod client;

#[cfg(feature = "client-trait")]
mod client_trait;
#[cfg(feature = "client-trait")]
pub use client_trait::HttpClient;

#[cfg(feature = "rate-limited-stream")]
mod rate_limited_stream;
#[cfg(feature = "rate-limited-stream")]
pub use rate_limited_stream::{
    RateLimitedStream, RateLimiterTag, RateLimiterTags, RateLimiterTagsHandle,
};

#[cfg(feature = "body")]
mod body;
#[cfg(feature = "body")]
pub use body::Body;
