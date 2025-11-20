use std::fmt::{self, Display};

use anyhow::Error;
use serde::{Deserialize, Serialize};

#[cfg(feature = "enum-fallback")]
use proxmox_fixed_string::FixedString;

use proxmox_schema::api;

#[api(default: "encrypt")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Defines whether data is encrypted (using an AEAD cipher), only signed, or neither.
pub enum CryptMode {
    /// Don't encrypt.
    None,
    /// Encrypt.
    Encrypt,
    /// Only sign.
    SignOnly,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Deserialize, Serialize)]
#[serde(transparent)]
/// 32-byte fingerprint, usually calculated with SHA256.
pub struct Fingerprint {
    #[serde(with = "bytes_as_fingerprint")]
    bytes: [u8; 32],
}

impl Fingerprint {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }
    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
    pub fn signature(&self) -> String {
        as_fingerprint(&self.bytes)
    }
}

/// Display as short key ID
impl Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", as_fingerprint(&self.bytes[0..8]))
    }
}

impl std::str::FromStr for Fingerprint {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let mut tmp = s.to_string();
        tmp.retain(|c| c != ':');
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(&tmp, &mut bytes)?;
        Ok(Fingerprint::new(bytes))
    }
}

fn as_fingerprint(bytes: &[u8]) -> String {
    hex::encode(bytes)
        .as_bytes()
        .chunks(2)
        .map(|v| unsafe { std::str::from_utf8_unchecked(v) }) // it's a hex string
        .collect::<Vec<&str>>()
        .join(":")
}

pub mod bytes_as_fingerprint {
    use std::fmt;

    use serde::{de, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = super::as_fingerprint(bytes);
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FingerprintVisitor;

        impl<'de> de::Visitor<'de> for FingerprintVisitor {
            type Value = [u8; 32];

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a 32-byte fingerprint hex string with colons")
            }

            fn visit_str<E>(self, val: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut filtered = [0u8; 64];
                let mut idx = 0;

                for &b in val.as_bytes().iter().filter(|&&b| b != b':') {
                    if idx == 64 {
                        return Err(E::custom("fingerprint too long"));
                    }
                    filtered[idx] = b;
                    idx += 1;
                }

                if idx != 64 {
                    return Err(E::custom("fingerprint too short"));
                }

                let mut out = [0u8; 32];
                hex::decode_to_slice(filtered, &mut out).map_err(serde::de::Error::custom)?;
                Ok(out)
            }
        }

        deserializer.deserialize_str(FingerprintVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::Fingerprint;

    static SAMPLE_BYTES: [u8; 32] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0,
        0xf0, 0x01,
    ];

    #[test]
    fn serialize_valid() {
        let s = Fingerprint::new(SAMPLE_BYTES);
        let encoded = serde_plain::to_string(&s).unwrap();
        assert!(encoded.contains("00:11:22:33:44:55:66:77"));
        assert!(encoded.contains("f0:01"));
    }

    #[test]
    fn deserialize_valid() {
        let s = "00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:10:20:30:40:50:60:70:80:90:a0:b0:c0:d0:e0:f0:01";
        let parsed = serde_plain::from_str::<Fingerprint>(s).unwrap();
        assert_eq!(parsed.bytes(), &SAMPLE_BYTES);
    }

    #[test]
    fn roundtrip() {
        let original = Fingerprint::new(SAMPLE_BYTES);
        let encoded = serde_plain::to_string(&original).unwrap();
        let decoded: Fingerprint = serde_plain::from_str(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn deserialize_invalid_char() {
        let s = "zz:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:10:20:30:40:50:60:70:80:90:a0:b0:c0:d0:e0:f0:01";
        let parsed = serde_plain::from_str::<Fingerprint>(s);
        assert!(parsed.is_err());
    }

    #[test]
    fn deserialize_too_short() {
        let s = "00:11:22:33";
        let parsed = serde_plain::from_str::<Fingerprint>(s);
        assert!(parsed.is_err());
    }

    #[test]
    fn deserialize_too_long() {
        let s = &"00:".repeat(33);
        let parsed = serde_plain::from_str::<Fingerprint>(s);
        assert!(parsed.is_err());
    }
}
