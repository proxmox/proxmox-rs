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

    /// Generic errors bubbled up from a deeper source, usually the http client.
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
}

#[derive(Clone, Copy, Debug)]
pub struct ParseFingerprintError;

impl StdError for ParseFingerprintError {}

impl fmt::Display for ParseFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("failed to parse fingerprint")
    }
}
