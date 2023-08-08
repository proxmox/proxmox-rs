use std::future::Future;

use serde::Serialize;

mod error;

pub use error::Error;

pub use proxmox_login::tfa::TfaChallenge;
pub use proxmox_login::{Authentication, Ticket};

pub(crate) mod auth;
pub use auth::Token;

#[cfg(feature = "hyper-client")]
mod client;
#[cfg(feature = "hyper-client")]
pub use client::{Client, TlsOptions};

/// A response from the HTTP API as required by the [`HttpApiClient`] trait.
pub struct HttpApiResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// HTTP client backend trait. This should be implemented for a HTTP client capable of making
/// *authenticated* API requests to a proxmox HTTP API.
pub trait HttpApiClient: Send + Sync {
    /// An API call should return a status code and the raw body.
    type ResponseFuture: Future<Output = Result<HttpApiResponse, Error>>;

    /// `GET` request with a path and query component (no hostname).
    ///
    /// For this request, authentication headers should be set!
    fn get(&self, path_and_query: &str) -> Self::ResponseFuture;

    /// `POST` request with a path and query component (no hostname), and a serializable body.
    ///
    /// The body should be serialized to json and sent with `Content-type: applicaion/json`.
    ///
    /// For this request, authentication headers should be set!
    fn post<T>(&self, path_and_query: &str, params: &T) -> Self::ResponseFuture
    where
        T: ?Sized + Serialize;

    /// `DELETE` request with a path and query component (no hostname).
    ///
    /// For this request, authentication headers should be set!
    fn delete(&self, path_and_query: &str) -> Self::ResponseFuture;
}
