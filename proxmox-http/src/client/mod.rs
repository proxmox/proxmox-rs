//! Simple TLS capable HTTP client implementation.
//!
//! Contains a lightweight wrapper around `hyper` with support for TLS connections.

mod connector;
pub use connector::HttpsConnector;

mod simple;
pub use simple::SimpleHttp;
pub use simple::SimpleHttpOptions;
