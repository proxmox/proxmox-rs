#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use anyhow::{bail, Error};

#[cfg(feature = "openssl")]
use openssl::sha;

use proxmox_schema::api_types::SHA256_HEX_REGEX;
use proxmox_schema::ApiStringFormat;
use proxmox_schema::ApiType;
use proxmox_schema::Schema;
use proxmox_schema::StringSchema;

pub const PROXMOX_CONFIG_DIGEST_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&SHA256_HEX_REGEX);

pub const PROXMOX_CONFIG_DIGEST_SCHEMA: Schema = StringSchema::new(
    "Prevent changes if current configuration file has different \
    SHA256 digest. This can be used to prevent concurrent \
    modifications.",
)
.format(&PROXMOX_CONFIG_DIGEST_FORMAT)
.schema();

#[derive(Clone, Debug, Eq, PartialEq)]
/// A configuration digest - a SHA256 hash.
pub struct ConfigDigest([u8; 32]);

impl ConfigDigest {
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0[..])
    }

    #[cfg(feature = "openssl")]
    pub fn from_slice<T: AsRef<[u8]>>(data: T) -> ConfigDigest {
        let digest = sha::sha256(data.as_ref());
        ConfigDigest(digest)
    }

    /// Detect modified configuration files
    ///
    /// This function fails with a reasonable error message if checksums do not match.
    pub fn detect_modification(&self, user_digest: Option<&Self>) -> Result<(), Error> {
        if let Some(user_digest) = user_digest {
            if user_digest != self {
                bail!("detected modified configuration - file changed by other user? Try again.");
            }
        }
        Ok(())
    }
}

impl ApiType for ConfigDigest {
    const API_SCHEMA: Schema = PROXMOX_CONFIG_DIGEST_SCHEMA;
}

impl From<[u8; 32]> for ConfigDigest {
    #[inline]
    fn from(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<ConfigDigest> for [u8; 32] {
    #[inline]
    fn from(digest: ConfigDigest) -> Self {
        digest.0
    }
}

impl AsRef<[u8]> for ConfigDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8; 32]> for ConfigDigest {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::ops::Deref for ConfigDigest {
    type Target = [u8; 32];

    fn deref(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::ops::DerefMut for ConfigDigest {
    fn deref_mut(&mut self) -> &mut [u8; 32] {
        &mut self.0
    }
}

impl std::fmt::Display for ConfigDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl std::str::FromStr for ConfigDigest {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, hex::FromHexError> {
        let mut digest = [0u8; 32];
        hex::decode_to_slice(s, &mut digest)?;
        Ok(ConfigDigest(digest))
    }
}

serde_plain::derive_deserialize_from_fromstr!(ConfigDigest, "valid configuration digest");
serde_plain::derive_serialize_from_display!(ConfigDigest);
