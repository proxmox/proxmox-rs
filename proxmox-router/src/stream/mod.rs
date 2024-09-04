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
}
