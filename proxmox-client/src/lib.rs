use std::collections::HashMap;
use std::future::Future;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// HTTP client backend trait. This should be implemented for a HTTP client capable of making
/// *authenticated* API requests to a proxmox HTTP API.
pub trait HttpApiClient: Send + Sync {
    /// An API call should return a status code and the raw body.
    type ResponseFuture<'a>: Future<Output = Result<HttpApiResponse, Error>> + 'a
    where
        Self: 'a;

    /// `GET` request with a path and query component (no hostname).
    ///
    /// For this request, authentication headers should be set!
    fn get<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a>;

    /// `POST` request with a path and query component (no hostname), and a serializable body.
    ///
    /// The body should be serialized to json and sent with `Content-type: applicaion/json`.
    ///
    /// For this request, authentication headers should be set!
    fn post<'a, T>(&'a self, path_and_query: &'a str, params: &T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize;

    /// `PUT` request with a path and query component (no hostname), and a serializable body.
    ///
    /// The body should be serialized to json and sent with `Content-type: applicaion/json`.
    ///
    /// For this request, authentication headers should be set!
    fn put<'a, T>(&'a self, path_and_query: &'a str, params: &T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize;

    /// `PUT` request with a path and query component (no hostname), no request body.
    ///
    /// For this request, authentication headers should be set!
    fn put_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a>;

    /// `DELETE` request with a path and query component (no hostname).
    ///
    /// For this request, authentication headers should be set!
    fn delete<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a>;
}

/// A response from the HTTP API as required by the [`HttpApiClient`] trait.
pub struct HttpApiResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl HttpApiResponse {
    /// Expect a JSON response as returend by the `extjs` formatter.
    pub fn expect_json<T>(self) -> Result<ApiResponseData<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.assert_json_content_type()?;

        serde_json::from_slice::<RawApiResponse<T>>(&self.body)
            .map_err(|err| Error::bad_api("failed to parse api response", err))?
            .check()
    }

    fn assert_json_content_type(&self) -> Result<(), Error> {
        match self.content_type.as_deref() {
            Some("application/json") => Ok(()),
            Some(other) => {
                return Err(Error::BadApi(
                    format!("expected json body, got {other}",),
                    None,
                ))
            }
            None => {
                return Err(Error::BadApi(
                    "expected json body, but no Content-Type was sent".to_string(),
                    None,
                ))
            }
        }
    }

    /// Expect that the API call did *not* return any data in the `data` field.
    pub fn nodata(self) -> Result<(), Error> {
        let response = serde_json::from_slice::<RawApiResponse<()>>(&self.body)
            .map_err(|err| Error::bad_api("failed to parse api response", err))?;

        if response.data.is_some() {
            Err(Error::UnexpectedData)
        } else {
            response.check()?;
            Ok(())
        }
    }
}

/// API responses can have additional *attributes* added to their data.
pub struct ApiResponseData<T> {
    pub attribs: HashMap<String, Value>,
    pub data: T,
}

#[derive(serde::Deserialize)]
struct RawApiResponse<T> {
    #[serde(default, deserialize_with = "proxmox_login::parse::deserialize_u16")]
    status: Option<u16>,
    message: Option<String>,
    #[serde(default, deserialize_with = "proxmox_login::parse::deserialize_bool")]
    success: Option<bool>,
    data: Option<T>,

    #[serde(default)]
    errors: HashMap<String, String>,

    #[serde(default, flatten)]
    attribs: HashMap<String, Value>,
}

impl<T> RawApiResponse<T> {
    pub fn check(mut self) -> Result<ApiResponseData<T>, Error> {
        if !self.success.unwrap_or(false) {
            let status = http::StatusCode::from_u16(self.status.unwrap_or(400))
                .unwrap_or(http::StatusCode::BAD_REQUEST);
            let mut message = self
                .message
                .take()
                .unwrap_or_else(|| "no message provided".to_string());
            for (param, error) in self.errors {
                use std::fmt::Write;
                let _ = write!(message, "\n{param}: {error}");
            }

            return Err(Error::api(status, message));
        }

        Ok(ApiResponseData {
            data: self
                .data
                .ok_or_else(|| Error::BadApi("api returned no data".to_string(), None))?,
            attribs: self.attribs,
        })
    }
}
