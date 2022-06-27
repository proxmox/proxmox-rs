//! Simple TLS capable HTTP client implementation.
//!
//! Contains a lightweight wrapper around `hyper` with support for TLS connections.

mod rate_limiter;
pub use rate_limiter::{RateLimit, RateLimiter, RateLimiterVec, ShareableRateLimit};

mod rate_limited_stream;
pub use rate_limited_stream::RateLimitedStream;

mod connector;
pub use connector::HttpsConnector;

mod simple;
pub use simple::SimpleHttp;
pub use simple::SimpleHttpOptions;

pub mod tls;
