//! Records of streamed API responses.
//!
//! A streaming API handler returns [`crate::Record`]s, which the REST server sends as an RFC 7464
//! JSON text sequence (`application/json-seq`). Each record is an object with either a `data` or an
//! `error` member, which [`Record<T>`] reads back. An error is a message or any serializable value:
//! a handler sends a value with [`crate::Record::error_value`], and a client that has to act on it
//! reads it back with [`Record::into_result_value`].

use anyhow::Error;
use serde::{Deserialize, Serialize};

#[cfg(feature = "stream")]
mod parsing;
#[cfg(feature = "stream")]
pub use parsing::{BodyBufReader, JsonRecords, Records};

/// Streamed JSON records can contain either "data" or an error.
///
/// Errors can be a simple string or structured data.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Record<T> {
    /// A successful record.
    Data(T),
    /// An error entry.
    Error(serde_json::Value),
}

impl<T> Record<T> {
    /// Convenience method to turn the record into a `Result`.
    ///
    /// The error is converted to either a message or, for structured errors, a json
    /// representation.
    pub fn into_result(self) -> Result<T, Error> {
        match self {
            Self::Data(data) => Ok(data),
            Self::Error(serde_json::Value::String(s)) => Err(Error::msg(s)),
            Self::Error(other) => match serde_json::to_string(&other) {
                Ok(s) => Err(Error::msg(s)),
                Err(err) => Err(Error::from(err)),
            },
        }
    }

    /// Turn the record into a `Result` that keeps an error as the value that was sent.
    ///
    /// Unlike [`into_result`](Self::into_result), a structured error, such as an envelope sent with
    /// [`crate::Record::error_value`], stays a value for the caller to deserialize:
    ///
    /// ```
    /// # use proxmox_router::stream::Record;
    /// let record: Record<u64> =
    ///     serde_json::from_str(r#"{"error":{"kind":"rate-limited","message":"later"}}"#)?;
    /// let error = record.into_result_value().unwrap_err();
    /// assert_eq!(error["kind"], "rate-limited");
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn into_result_value(self) -> Result<T, serde_json::Value> {
        match self {
            Self::Data(data) => Ok(data),
            Self::Error(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Record;

    fn parse(text: &str) -> Record<u64> {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn errors_keep_the_value_that_was_sent() {
        assert_eq!(parse(r#"{"data":1}"#).into_result_value(), Ok(1));
        assert_eq!(
            parse(r#"{"error":"failed"}"#).into_result_value(),
            Err(json!("failed"))
        );
        assert_eq!(
            parse(r#"{"error":{"kind":"rate-limited","message":"later"}}"#).into_result_value(),
            Err(json!({"kind": "rate-limited", "message": "later"}))
        );
    }

    #[test]
    fn into_result_turns_errors_into_their_text() {
        assert_eq!(parse(r#"{"data":1}"#).into_result().unwrap(), 1);
        assert_eq!(
            parse(r#"{"error":"failed"}"#)
                .into_result()
                .unwrap_err()
                .to_string(),
            "failed"
        );
        assert_eq!(
            parse(r#"{"error":{"kind":"rate-limited"}}"#)
                .into_result()
                .unwrap_err()
                .to_string(),
            r#"{"kind":"rate-limited"}"#
        );
    }
}
