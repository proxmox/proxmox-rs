//! Token bucket based traffic rate limiter implementations.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "rate-limiter")]
mod rate_limiter;
#[cfg(feature = "rate-limiter")]
pub use rate_limiter::{RateLimit, RateLimiter, RateLimiterVec, ShareableRateLimit};

#[cfg(feature = "shared-rate-limiter")]
mod shared_rate_limiter;
#[cfg(feature = "shared-rate-limiter")]
pub use shared_rate_limiter::SharedRateLimiter;
