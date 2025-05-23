//! The `Error` type and some ACME error constants for reference.

use std::fmt;

use openssl::error::ErrorStack as SslErrorStack;

/// The ACME error string for a "bad nonce" error.
pub const BAD_NONCE: &str = "urn:ietf:params:acme:error:badNonce";

/// The ACME error string for a "user action required" error.
pub const USER_ACTION_REQUIRED: &str = "urn:ietf:params:acme:error:userActionRequired";

/// Error types returned by this crate.
#[derive(Debug)]
#[must_use = "unused errors have no effect"]
pub enum Error {
    /// A `badNonce` API response. The request should be retried with the new nonce received along
    /// with this response.
    BadNonce,

    /// A `userActionRequired` API response. Typically this means there was a change to the ToS and
    /// the user has to agree to the new terms.
    UserActionRequired(String),

    /// Other error responses from the Acme API not handled specially.
    Api(crate::request::ErrorResponse),

    /// The Acme API behaved unexpectedly.
    InvalidApi(String),

    /// Tried to use an `Account` or `AccountCreator` without a private key.
    MissingKey,

    /// Tried to create an `Account` without providing a single contact info.
    MissingContactInfo,

    /// Tried to use an empty `Order`.
    EmptyOrder,

    /// A raw `openssl::PKey` containing an unsupported key was passed.
    UnsupportedKeyType,

    /// A raw `openssl::PKey` or `openssl::EcKey` with an unsupported curve was passed.
    UnsupportedGroup,

    /// Failed to parse the account data returned by the API upon account creation.
    BadAccountData(String),

    /// Failed to  parse the order data returned by the API from a new-order request.
    BadOrderData(String),

    /// An openssl error occurred during a crypto operation.
    RawSsl(SslErrorStack),

    /// An openssl error occurred during a crypto operation.
    /// With some textual context.
    Ssl(&'static str, SslErrorStack),

    /// An otherwise uncaught serde error happened.
    Json(serde_json::Error),

    /// Failed to parse
    BadBase64(proxmox_base64::DecodeError),

    /// Can be used by the user for textual error messages without having to downcast to regular
    /// acme errors.
    Custom(String),

    /// If built with the `client` feature, this is where general ureq/network errors end up.
    /// This is usually a `ureq::Error`, however in order to provide an API which is not
    /// feature-dependent, this variant is always present and contains a boxed `dyn Error`.
    HttpClient(Box<dyn std::error::Error + Send + Sync + 'static>),

    /// If built with the `client` feature, this is where client specific errors which are not from
    /// errors forwarded from `ureq` end up.
    Client(String),

    /// A non-openssl error occurred while building data for the CSR.
    Csr(String),
}

impl Error {
    /// Create an `Error` from a custom text.
    pub fn custom<T: std::fmt::Display>(s: T) -> Self {
        Error::Custom(s.to_string())
    }

    /// Convenience method to check if this error represents a bad nonce error in which case the
    /// request needs to be re-created using a new nonce.
    pub fn is_bad_nonce(&self) -> bool {
        matches!(self, Error::BadNonce)
    }
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Api(err) => match err.detail.as_deref() {
                Some(detail) => write!(f, "{}: {}", err.ty, detail),
                None => fmt::Display::fmt(&err.ty, f),
            },
            Error::InvalidApi(err) => write!(f, "Acme Server API misbehaved: {}", err),
            Error::BadNonce => f.write_str("bad nonce, please retry with a new nonce"),
            Error::UserActionRequired(err) => write!(f, "user action required: {}", err),
            Error::MissingKey => f.write_str("cannot build an account without a key"),
            Error::MissingContactInfo => f.write_str("account requires contact info"),
            Error::EmptyOrder => f.write_str("cannot make an empty order"),
            Error::UnsupportedKeyType => f.write_str("unsupported key type"),
            Error::UnsupportedGroup => f.write_str("unsupported EC group"),
            Error::BadAccountData(err) => {
                write!(f, "bad response to account query or creation: {}", err)
            }
            Error::BadOrderData(err) => {
                write!(f, "bad response to new-order query or creation: {}", err)
            }
            Error::RawSsl(err) => fmt::Display::fmt(err, f),
            Error::Ssl(context, err) => {
                write!(f, "{}: {}", context, err)
            }
            Error::Json(err) => fmt::Display::fmt(err, f),
            Error::Custom(err) => fmt::Display::fmt(err, f),
            Error::HttpClient(err) => fmt::Display::fmt(err, f),
            Error::Client(err) => fmt::Display::fmt(err, f),
            Error::Csr(err) => fmt::Display::fmt(err, f),
            Error::BadBase64(err) => fmt::Display::fmt(err, f),
        }
    }
}

impl From<SslErrorStack> for Error {
    fn from(e: SslErrorStack) -> Self {
        Error::RawSsl(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<crate::request::ErrorResponse> for Error {
    fn from(e: crate::request::ErrorResponse) -> Self {
        Error::Api(e)
    }
}

impl From<proxmox_base64::DecodeError> for Error {
    fn from(e: proxmox_base64::DecodeError) -> Self {
        Error::BadBase64(e)
    }
}
