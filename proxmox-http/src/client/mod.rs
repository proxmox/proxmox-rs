//! Simple TLS capable HTTP client implementations.
//!
//! Feature `client` contains a lightweight wrapper around `hyper` with support for TLS connections
//! in [`SimpleHttp`](crate::client::SimpleHttp).
//!
//! Feature `client-sync` contains a lightweight wrapper around `ureq` in
//! [`sync::Client`](crate::client::sync::Client).
//!
//! Both clients implement [`HttpClient`](crate::HttpClient) if the feature `client-trait` is enabled.

#[cfg(feature = "client")]
mod rate_limiter;
#[cfg(feature = "client")]
pub use rate_limiter::{RateLimit, RateLimiter, RateLimiterVec, ShareableRateLimit};

#[cfg(feature = "client")]
mod rate_limited_stream;
#[cfg(feature = "client")]
pub use rate_limited_stream::RateLimitedStream;

#[cfg(feature = "client")]
mod connector;
#[cfg(feature = "client")]
pub use connector::HttpsConnector;

#[cfg(feature = "client")]
mod simple;
#[cfg(feature = "client")]
pub use simple::SimpleHttp;

#[cfg(feature = "client")]
pub mod tls;

#[cfg(feature = "client-sync")]
/// Blocking HTTP client
pub mod sync;
