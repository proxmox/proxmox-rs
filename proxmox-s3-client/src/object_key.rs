use anyhow::{bail, Error};

/// Byte limit for s3 object keys.
/// See https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-keys.html
const S3_OBJECT_KEY_MAX_LENGTH: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
/// S3 Object Key
pub enum S3ObjectKey {
    /// Object key which will not be prefixed any further by the client
    Full(String),
    /// Object key which will be expanded by the client with its configured common prefix
    Relative(String),
}

impl core::convert::TryFrom<&str> for S3ObjectKey {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Error> {
        let (key, key_byte_length) = if let Some(s) = s.strip_prefix("/") {
            (Self::Full(s.to_string()), s.as_bytes().len())
        } else {
            (Self::Relative(s.to_string()), s.as_bytes().len())
        };
        if key_byte_length > S3_OBJECT_KEY_MAX_LENGTH {
            bail!(
                "Object key length of {key_byte_length} exceeds limit of {S3_OBJECT_KEY_MAX_LENGTH}",
            );
        }
        Ok(key)
    }
}
impl S3ObjectKey {
    /// Convert the given object key to a full key by extending it via given prefix
    /// If the object key is already a full key, the prefix is ignored.
    pub(crate) fn to_full_key(&self, prefix: &str) -> Self {
        match self {
            Self::Full(ref key) => Self::Full(key.to_string()),
            Self::Relative(ref key) => {
                let prefix = prefix.strip_prefix("/").unwrap_or(&prefix);
                Self::Full(format!("{prefix}/{key}"))
            }
        }
    }
}

impl std::ops::Deref for S3ObjectKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Full(key) => key,
            Self::Relative(key) => key,
        }
    }
}

impl std::fmt::Display for S3ObjectKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(key) => write!(f, "{key}"),
            Self::Relative(key) => write!(f, "{key}"),
        }
    }
}

impl std::str::FromStr for S3ObjectKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

// Do not mangle with prefixes when de-serializing
impl<'de> serde::Deserialize<'de> for S3ObjectKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let object_key = std::borrow::Cow::<'de, str>::deserialize(deserializer)?.to_string();
        Ok(Self::Full(object_key))
    }
}

impl S3ObjectKey {
    /// Generate source key for copy object operations given the source bucket.
    /// Extends relative object key variants also by the given prefix.
    pub fn to_copy_source_key(&self, source_bucket: &str, prefix: &str) -> Self {
        match self {
            Self::Full(key) => Self::Full(format!("{source_bucket}{key}")),
            Self::Relative(key) => {
                let prefix = prefix.strip_prefix("/").unwrap_or(&prefix);
                Self::Full(format!("{source_bucket}/{prefix}/{key}"))
            }
        }
    }
}
