use std::error::Error as StdError;
use std::fmt::Display;

use crate::Config;
use serde::Serialize;

pub mod common;
#[cfg(feature = "sendmail")]
pub mod sendmail;

#[derive(Debug, Serialize)]
pub struct ApiError {
    /// HTTP Error code
    code: u16,
    /// Error message
    message: String,
    #[serde(skip_serializing)]
    /// The underlying cause of the error
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl ApiError {
    fn new<S: AsRef<str>>(
        message: S,
        code: u16,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            message: message.as_ref().into(),
            code,
            source,
        }
    }

    pub fn bad_request<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 400, source)
    }

    pub fn not_found<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 404, source)
    }

    pub fn internal_server_error<S: AsRef<str>>(
        message: S,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        Self::new(message, 500, source)
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{} {}", self.code, self.message))
    }
}

impl StdError for ApiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.source {
            None => None,
            Some(source) => Some(&**source),
        }
    }
}

fn verify_digest(config: &Config, digest: Option<&[u8]>) -> Result<(), ApiError> {
    if let Some(digest) = digest {
        if config.digest != *digest {
            return Err(ApiError::bad_request(
                "detected modified configuration - file changed by other user? Try again.",
                None,
            ));
        }
    }

    Ok(())
}

fn endpoint_exists(config: &Config, name: &str) -> bool {
    let mut exists = false;

    #[cfg(feature = "sendmail")]
    {
        exists = exists || sendmail::get_endpoint(config, name).is_ok();
    }

    exists
}

#[cfg(test)]
mod test_helpers {
    use crate::Config;

    pub fn empty_config() -> Config {
        Config::new("", "").unwrap()
    }
}
