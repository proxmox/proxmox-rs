use std::fmt;

use failure::Fail;

#[doc(hidden)]
pub use http::StatusCode;

/// HTTP error including `StatusCode` and message.
#[derive(Debug, Fail)]
pub struct HttpError {
    pub code: StatusCode,
    pub message: String,
}

impl HttpError {
    pub fn new(code: StatusCode, message: String) -> Self {
        HttpError { code, message }
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Macro to create a HttpError inside a failure::Error
#[macro_export]
macro_rules! http_err {
    ($status:ident, $msg:expr) => {{
        ::failure::Error::from($crate::api::error::HttpError::new(
            $crate::api::error::StatusCode::$status,
            $msg,
        ))
    }};
}
