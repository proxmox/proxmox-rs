#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::collections::HashMap;
use std::future::Future;

use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod error;

pub use error::Error;

pub use proxmox_login::tfa::TfaChallenge;
pub use proxmox_login::{Authentication, Ticket};

mod api_path_builder;
pub use api_path_builder::ApiPathBuilder;

pub(crate) mod auth;
pub use auth::{AuthenticationKind, Token};

#[cfg(feature = "hyper-client")]
mod client;
#[cfg(feature = "hyper-client")]
pub use client::{Client, TlsOptions};

/// HTTP client backend trait. This should be implemented for a HTTP client capable of making
/// *authenticated* API requests to a proxmox HTTP API.
pub trait HttpApiClient {
    /// An API call should return a status code and the raw body.
    type ResponseFuture<'a>: Future<Output = Result<HttpApiResponse, Error>> + 'a
    where
        Self: 'a;

    /// Some requests are better "streamed" than collected in RAM, for this, the body type used by
    /// the underlying client needs to be exposed.
    type Body;

    /// Future for streamed requests.
    type ResponseStreamFuture<'a>: Future<Output = Result<HttpApiResponseStream<Self::Body>, Error>>
        + 'a
    where
        Self: 'a;

    /// An *authenticated* asynchronous request with a path and query component (no hostname), and
    /// an optional body, of which the response body is read to completion.
    ///
    /// For this request, authentication headers should be set!
    fn request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseFuture<'a>
    where
        T: Serialize + 'a;

    /// An *authenticated* asynchronous request with a path and query component (no hostname), and
    /// an optional body. The response status is returned, but the body is returned for the caller
    /// to read from.
    ///
    /// For this request, authentication headers should be set!
    fn streaming_request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseStreamFuture<'a>
    where
        T: Serialize + 'a;

    /// This is deprecated.
    /// Calls `self.request` with `Method::GET` and `None` for the body.
    fn get<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        self.request(Method::GET, path_and_query, None::<()>)
    }

    /// This is deprecated.
    /// Calls `self.request` with `Method::POST`.
    fn post<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        self.request(Method::POST, path_and_query, Some(params))
    }

    /// This is deprecated.
    /// Calls `self.request` with `Method::POST` and `None` for the body..
    fn post_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        self.request(Method::POST, path_and_query, None::<()>)
    }

    /// This is deprecated.
    /// Calls `self.request` with `Method::PUT`.
    fn put<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        self.request(Method::PUT, path_and_query, Some(params))
    }

    /// This is deprecated.
    /// Calls `self.request` with `Method::PUT` and `None` for the body..
    fn put_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        self.request(Method::PUT, path_and_query, None::<()>)
    }

    /// This is deprecated.
    /// Calls `self.request` with `Method::DELETE`.
    fn delete<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        self.request(Method::DELETE, path_and_query, None::<()>)
    }
}

/// A response from the HTTP API as required by the [`HttpApiClient`] trait.
pub struct HttpApiResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl HttpApiResponse {
    /// Expect a JSON response as returned by the `extjs` formatter.
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
        match self
            .content_type
            .as_deref()
            .and_then(|v| v.split(';').next())
        {
            Some("application/json") => Ok(()),
            Some(other) => Err(Error::BadApi(
                format!("expected json body, got {other}",),
                None,
            )),
            None => Err(Error::BadApi(
                "expected json body, but no Content-Type was sent".to_string(),
                None,
            )),
        }
    }

    /// Expect that the API call did *not* return any data in the `data` field.
    pub fn nodata(self) -> Result<(), Error> {
        let response = serde_json::from_slice::<RawApiResponse<()>>(&self.body)
            .map_err(|err| Error::bad_api("unexpected api response", err))?;

        if response.data.is_some() {
            Err(Error::UnexpectedData)
        } else {
            response.check_nodata()?;
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
    #[serde(default, deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    status: Option<u16>,
    message: Option<String>,
    #[serde(default, deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    success: Option<bool>,
    data: Option<T>,

    #[serde(default)]
    errors: HashMap<String, String>,

    #[serde(default, flatten)]
    attribs: HashMap<String, Value>,
}

impl<T> RawApiResponse<T>
where
    T: for<'de> Deserialize<'de>,
{
    fn check_success(mut self) -> Result<Self, Error> {
        if self.success == Some(true) {
            return Ok(self);
        }

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

        Err(Error::api(status, message))
    }

    fn check(self) -> Result<ApiResponseData<T>, Error> {
        let this = self.check_success()?;

        // RawApiResponse has no data, but this also happens for Value::Null, and T
        // might be deserializeable from that, so try here again
        let data = match this.data {
            Some(data) => data,
            None => serde_json::from_value(Value::Null)
                .map_err(|_| Error::BadApi("api returned no data".to_string(), None))?,
        };

        Ok(ApiResponseData {
            data,
            attribs: this.attribs,
        })
    }

    fn check_nodata(self) -> Result<ApiResponseData<()>, Error> {
        let this = self.check_success()?;

        Ok(ApiResponseData {
            data: (),
            attribs: this.attribs,
        })
    }
}

impl<C> HttpApiClient for &C
where
    C: HttpApiClient,
{
    type ResponseFuture<'a>
        = C::ResponseFuture<'a>
    where
        Self: 'a;

    type Body = C::Body;

    type ResponseStreamFuture<'a>
        = C::ResponseStreamFuture<'a>
    where
        Self: 'a;

    fn request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::request(self, method, path_and_query, params)
    }

    fn streaming_request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseStreamFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::streaming_request(self, method, path_and_query, params)
    }

    fn get<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::get(self, path_and_query)
    }

    fn post<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::post(self, path_and_query, params)
    }

    fn post_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::post_without_body(self, path_and_query)
    }

    fn put<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::put(self, path_and_query, params)
    }

    fn put_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::put_without_body(self, path_and_query)
    }

    fn delete<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::delete(self, path_and_query)
    }
}

impl<C> HttpApiClient for std::sync::Arc<C>
where
    C: HttpApiClient,
{
    type ResponseFuture<'a>
        = C::ResponseFuture<'a>
    where
        Self: 'a;

    type Body = C::Body;

    type ResponseStreamFuture<'a>
        = C::ResponseStreamFuture<'a>
    where
        Self: 'a;

    fn request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::request(self, method, path_and_query, params)
    }

    fn streaming_request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseStreamFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::streaming_request(self, method, path_and_query, params)
    }

    fn get<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::get(self, path_and_query)
    }

    fn post<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::post(self, path_and_query, params)
    }

    fn post_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::post_without_body(self, path_and_query)
    }

    fn put<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::put(self, path_and_query, params)
    }

    fn put_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::put_without_body(self, path_and_query)
    }

    fn delete<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::delete(self, path_and_query)
    }
}

impl<C> HttpApiClient for std::rc::Rc<C>
where
    C: HttpApiClient,
{
    type ResponseFuture<'a>
        = C::ResponseFuture<'a>
    where
        Self: 'a;

    type Body = C::Body;

    type ResponseStreamFuture<'a>
        = C::ResponseStreamFuture<'a>
    where
        Self: 'a;

    fn request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::request(self, method, path_and_query, params)
    }

    fn streaming_request<'a, T>(
        &'a self,
        method: Method,
        path_and_query: &'a str,
        params: Option<T>,
    ) -> Self::ResponseStreamFuture<'a>
    where
        T: Serialize + 'a,
    {
        C::streaming_request(self, method, path_and_query, params)
    }

    fn get<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::get(self, path_and_query)
    }

    fn post<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::post(self, path_and_query, params)
    }

    fn post_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::post_without_body(self, path_and_query)
    }

    fn put<'a, T>(&'a self, path_and_query: &'a str, params: &'a T) -> Self::ResponseFuture<'a>
    where
        T: ?Sized + Serialize,
    {
        C::put(self, path_and_query, params)
    }

    fn put_without_body<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::put_without_body(self, path_and_query)
    }

    fn delete<'a>(&'a self, path_and_query: &'a str) -> Self::ResponseFuture<'a> {
        C::delete(self, path_and_query)
    }
}

/// A streaming response from the HTTP API as required by the [`HttpApiClient`] trait.
pub struct HttpApiResponseStream<Body> {
    pub status: u16,
    pub content_type: Option<String>,
    /// Requests where the response has no body may put `None` here.
    pub body: Option<Body>,
}
