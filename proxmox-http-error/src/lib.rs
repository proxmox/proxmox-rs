use std::fmt;

use serde::{ser::SerializeStruct, Serialize, Serializer};

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

impl Serialize for HttpError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("HttpError", 2)?;
        state.serialize_field("code", &self.code.as_u16())?;
        state.serialize_field("message", &self.message)?;
        state.end()
    }
}

/// Macro to create a HttpError inside a anyhow::Error
#[macro_export]
macro_rules! http_err {
    ($status:ident, $($fmt:tt)+) => {{
        ::anyhow::Error::from($crate::HttpError::new(
            $crate::StatusCode::$status,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_err() {
        // Make sure the macro generates valid code.
        http_err!(IM_A_TEAPOT, "Cannot brew coffee");
    }

    #[test]
    fn test_http_bail() {
        fn t() -> Result<(), anyhow::Error> {
            // Make sure the macro generates valid code.
            http_bail!(
                UNAVAILABLE_FOR_LEGAL_REASONS,
                "Nothing to see here, move along"
            );
        }

        assert!(t().is_err());
    }
}
