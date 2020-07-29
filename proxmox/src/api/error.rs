use std::fmt;

#[doc(hidden)]
pub use http::StatusCode;

/// HTTP error including `StatusCode` and message.
#[derive(Debug)]
pub struct HttpError {
    pub code: StatusCode,
    pub message: String,
}

impl std::error::Error for HttpError {}

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

/// Macro to create a HttpError inside a anyhow::Error
#[macro_export]
macro_rules! http_err {
    ($status:ident, $($fmt:tt)+) => {{
        ::anyhow::Error::from($crate::api::error::HttpError::new(
            $crate::api::error::StatusCode::$status,
            format!($($fmt)+)
        ))
    }};
}

/// Bail with an error generated with the `http_err!` macro.
#[macro_export]
macro_rules! http_bail {
    ($status:ident, $($fmt:tt)+) => {{
        return Err($crate::http_err!($status, $($fmt)+));
    }};
}
