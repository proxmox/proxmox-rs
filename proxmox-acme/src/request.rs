use serde::Deserialize;

pub(crate) const JSON_CONTENT_TYPE: &str = "application/jose+json";

/// A request which should be performed on the ACME provider.
pub struct Request {
    /// The complete URL to send the request to.
    pub url: String,

    /// The HTTP method name to use.
    pub method: &'static str,

    /// The `Content-Type` header to pass along.
    pub content_type: &'static str,

    /// The body to pass along with request, or an empty string.
    pub body: String,

    /// The expected status code a compliant ACME provider will return on success.
    pub expected: u16,
}

/// Common HTTP status codes used in ACME responses.
pub(crate) mod http_status {
    /// 200 OK
    pub(crate) const OK: u16 = 200;
    /// 201 Created
    pub(crate) const CREATED: u16 = 201;
    /// 204 No Content
    pub(crate) const NO_CONTENT: u16 = 204;
}

/// An ACME error response contains a specially formatted type string, and can optionally
/// contain textual details and a set of sub problems.
#[derive(Clone, Debug, Deserialize)]
pub struct ErrorResponse {
    /// The ACME error type string.
    ///
    /// Most of the time we're only interested in the "bad nonce" or "user action required"
    /// errors. When an [`Error`](crate::Error) is built from this error response, it will map
    /// to the corresponding enum values (eg. [`Error::BadNonce`](crate::Error::BadNonce)).
    #[serde(rename = "type")]
    pub ty: String,

    /// A textual detail string optionally provided by the ACME provider to inform the user more
    /// verbosely about why the error occurred.
    pub detail: Option<String>,

    /// Additional json data containing information as to why the error occurred.
    pub subproblems: Option<serde_json::Value>,
}
