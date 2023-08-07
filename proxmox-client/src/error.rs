use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The environment did not provide a way to get a 2nd factor.
    TfaNotSupported,

    /// The task API wants to poll for completion of a task at regular intervals, for this it needs
    /// to sleep. This signals that the environment does not support that.
    SleepNotSupported,

    /// Tried to make an API call without a ticket which requires ones.
    Unauthorized,

    /// The API responded with an error code.
    Api(http::StatusCode, String),

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
            Self::TfaNotSupported => f.write_str("tfa not supported by environment"),
            Self::SleepNotSupported => f.write_str("environment does not support sleeping"),
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::Api(status, msg) => write!(f, "api error (status = {status}): {msg}"),
            Self::Other(err) => f.write_str(err),
            Self::Authentication(err) => write!(f, "authentication error: {err}"),
            Self::Ticket(err) => write!(f, "authentication error: {err}"),
            Self::Client(err) => fmt::Display::fmt(err, f),
            Self::Internal(msg, _) => f.write_str(msg),
            Self::Anyhow(err) => fmt::Display::fmt(err, f),
        }
    }
}

impl Error {
    pub(crate) fn api<T: Display>(status: http::StatusCode, msg: T) -> Self {
        Self::Api(status, msg.to_string())
    }

    pub(crate) fn internal<E>(context: &'static str, err: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::Internal(context, Box::new(err))
    }
}

impl From<proxmox_login::error::ResponseError> for Error {
    fn from(err: proxmox_login::error::ResponseError) -> Self {
        Self::Authentication(err)
    }
}
