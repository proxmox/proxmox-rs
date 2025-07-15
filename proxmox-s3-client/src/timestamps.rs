use anyhow::{anyhow, Error};

#[derive(Debug)]
/// Last modified timestamp as obtained from API response http headers.
pub struct LastModifiedTimestamp {
    _datetime: iso8601::DateTime,
}

impl std::str::FromStr for LastModifiedTimestamp {
    type Err = Error;

    fn from_str(timestamp: &str) -> Result<Self, Self::Err> {
        let _datetime = iso8601::datetime(timestamp).map_err(|err| anyhow!(err))?;
        Ok(Self { _datetime })
    }
}

serde_plain::derive_deserialize_from_fromstr!(LastModifiedTimestamp, "last modified timestamp");
