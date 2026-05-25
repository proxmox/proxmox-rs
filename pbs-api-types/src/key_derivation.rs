use serde::{Deserialize, Serialize};

#[cfg(feature = "enum-fallback")]
use proxmox_fixed_string::FixedString;

use proxmox_schema::api_types::SAFE_ID_FORMAT;
use proxmox_schema::{Schema, StringSchema, Updater, api};

use crate::CERT_FINGERPRINT_SHA256_SCHEMA;

#[api(default: "scrypt")]
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
/// Key derivation function for password protected encryption keys.
pub enum Kdf {
    /// Do not encrypt the key.
    None,
    /// Encrypt they key with a password using SCrypt.
    Scrypt,
    /// Encrtypt the Key with a password using PBKDF2
    PBKDF2,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

impl Default for Kdf {
    #[inline]
    fn default() -> Self {
        Kdf::Scrypt
    }
}

#[api(
    properties: {
        kdf: {
            type: Kdf,
        },
        fingerprint: {
            schema: CERT_FINGERPRINT_SHA256_SCHEMA,
            optional: true,
        },
    },
)]
#[derive(Clone, Default, Deserialize, Serialize, Updater, PartialEq)]
/// Encryption Key Information
pub struct KeyInfo {
    /// Path to key (if stored in a file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub kdf: Kdf,
    /// Key creation time
    pub created: i64,
    /// Key modification time
    pub modified: i64,
    /// Key fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Password hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// ID to uniquely identify an encryption/decryption key.
pub const CRYPT_KEY_ID_SCHEMA: Schema =
    StringSchema::new("ID to uniquely identify encryption/decription key")
        .format(&SAFE_ID_FORMAT)
        .min_length(3)
        .max_length(32)
        .schema();

#[api(
    properties: {
        id: {
            schema: CRYPT_KEY_ID_SCHEMA,
        },
        info: {
            type: KeyInfo,
        },
    },
)]
#[derive(Clone, Default, Deserialize, Serialize, Updater, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Encryption/Decryption Key Info with ID.
pub struct CryptKey {
    #[updater(skip)]
    pub id: String,
    #[serde(flatten)]
    pub info: KeyInfo,
    /// Timestamp when key was archived (not set if key is active).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
}
