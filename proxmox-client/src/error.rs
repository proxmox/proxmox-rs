use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Tried to make an API call without a ticket.
    Unauthorized,

    /// The API responded with an error code.
    Api(http::StatusCode, String),

    /// The API returned something unexpected.
    BadApi(String, Option<Box<dyn StdError + Send + Sync + 'static>>),

    /// An API call which is meant to return nothing returned unexpected data.
    UnexpectedData,

    /// An error occurred in the authentication API.
    Authentication(proxmox_login::error::ResponseError),

    /// The current ticket was rejected.
    Ticket(proxmox_login::error::TicketError),

    /// Generic errors.
    Other(&'static str),

    /// Failed to establish a new connection (DNS, TCP, TLS).
    ///
    /// The request was guaranteed never sent. Safe to retry on a different endpoint; retrying the
    /// same endpoint is unlikely to help unless the failure was transient.
    Connect(Box<dyn StdError + Send + Sync + 'static>),

    /// An error after the connection was already established.
    ///
    /// Note that hyper-util already retries internally when a pooled connection turns out to be
    /// stale (request never left the client). Errors that reach this variant have typically
    /// progressed past that point, meaning the request was likely serialized onto the wire and the
    /// server may have processed it. Callers must not retry non-idempotent requests blindly.
    Client(Box<dyn StdError + Send + Sync + 'static>),

    /// Another internal error occurred.
    Internal(&'static str, Box<dyn StdError + Send + Sync + 'static>),

    /// An `anyhow` error because `proxmox_http::Client` uses it...
    Anyhow(anyhow::Error),
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Authentication(err) => Some(err),
            Self::BadApi(_, Some(err)) => Some(&**err),
            Self::Ticket(err) => Some(err),
            Self::Connect(err) => Some(&**err),
            Self::Client(err) => Some(&**err),
            Self::Internal(_, err) => Some(&**err),
            Self::Anyhow(err) => err.chain().next(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::Api(status, msg) => write!(f, "api error (status = {}: {msg})", status.as_u16()),
            Self::UnexpectedData => write!(f, "api unexpectedly returned data"),
            Self::BadApi(msg, _) => write!(f, "api returned unexpected data - {msg}"),
            Self::Other(err) => f.write_str(err),
            Self::Authentication(err) => write!(f, "authentication error: {err}"),
            Self::Ticket(err) => write!(f, "authentication error: {err}"),
            Self::Connect(err) => write!(f, "connection failed: {err}"),
            Self::Client(err) => fmt::Display::fmt(err, f),
            Self::Internal(msg, _) => f.write_str(msg),
            Self::Anyhow(err) => fmt::Display::fmt(err, f),
        }
    }
}

impl From<proxmox_login::error::ResponseError> for Error {
    fn from(err: proxmox_login::error::ResponseError) -> Self {
        Self::Authentication(err)
    }
}

impl Error {
    pub(crate) fn bad_api<T, E>(msg: T, err: E) -> Self
    where
        T: Into<String>,
        E: StdError + Send + Sync + 'static,
    {
        Self::BadApi(msg.into(), Some(Box::new(err)))
    }

    pub(crate) fn api<T: Into<String>>(status: http::StatusCode, msg: T) -> Self {
        Self::Api(status, msg.into())
    }

    /// Build an [`Error::Api`] from a raw response body.
    ///
    /// Tries to parse the Proxmox JSON envelope (`{"message": "..."}`) and uses the `message` field
    /// as the error string. Falls back to the raw UTF-8 body, or a status-only message if the body
    /// is not valid text.
    pub(crate) fn api_from_body(status: http::StatusCode, body: &[u8]) -> Self {
        #[derive(serde::Deserialize)]
        struct ApiError {
            #[serde(default)]
            message: Option<String>,
        }

        if let Ok(ApiError {
            message: Some(message),
        }) = serde_json::from_slice::<ApiError>(body)
        {
            return Self::Api(status, message);
        }

        match std::str::from_utf8(body) {
            Ok(text) if !text.is_empty() => Self::Api(status, text.to_owned()),
            _ => Self::Api(status, format!("HTTP error {}", status.as_u16())),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseFingerprintError;

impl StdError for ParseFingerprintError {}

impl fmt::Display for ParseFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("failed to parse fingerprint")
    }
}
