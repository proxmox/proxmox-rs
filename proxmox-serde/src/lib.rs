//! Serialization helpers for serde

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
pub mod serde_macros;

#[cfg(feature = "serde_json")]
pub mod json;

/// Serialize Unix epoch (i64) as RFC3339.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox_serde::epoch_as_rfc3339")]
///     date: i64,
/// }
///
/// let obj = Foo { date: 86400 }; // random test value
/// let json = serde_json::to_string(&obj).unwrap();
///
/// let deserialized: Foo = serde_json::from_str(&json).unwrap();
/// assert_eq!(obj, deserialized);
/// ```
pub mod epoch_as_rfc3339 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(epoch: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let s =
            proxmox_time::epoch_to_rfc3339(*epoch).map_err(|err| Error::custom(err.to_string()))?;

        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            proxmox_time::parse_rfc3339(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

/// Serialize `Vec<u8>` as base64 encoded string.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox_serde::bytes_as_base64")]
///     data: Vec<u8>,
/// }
///
/// let obj = Foo { data: vec![1, 2, 3, 4] };
/// let json = serde_json::to_string(&obj).unwrap();
/// assert_eq!(json, r#"{"data":"AQIDBA=="}"#);
///
/// let deserialized: Foo = serde_json::from_str(&json).unwrap();
/// assert_eq!(obj, deserialized);
/// ```
pub mod bytes_as_base64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: AsRef<[u8]>,
        S: Serializer,
    {
        serializer.serialize_str(&base64::encode(data.as_ref()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|string| base64::decode(string).map_err(|err| Error::custom(err.to_string())))
    }
}

/// Serialize `String` or `Option<String>` as base64 encoded.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox_serde::string_as_base64")]
///     data: String,
/// }
///
/// let obj = Foo { data: "FOO".to_string() };
/// let json = serde_json::to_string(&obj).unwrap();
/// assert_eq!(json, r#"{"data":"Rk9P"}"#);
///
/// let deserialized: Foo = serde_json::from_str(&json).unwrap();
/// assert_eq!(obj, deserialized);
/// ```
pub mod string_as_base64 {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Private trait to enable `string_as_base64` for `Option<String>` in addition to `String`.
    #[doc(hidden)]
    pub trait StrAsBase64: Sized {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
        fn de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
    }

    fn finish_deserializing<'de, D: Deserializer<'de>>(string: String) -> Result<String, D::Error> {
        use serde::de::Error;

        let bytes = base64::decode(string).map_err(|err| {
            let msg = format!("base64 decode: {}", err);
            Error::custom(msg)
        })?;

        String::from_utf8(bytes).map_err(|err| {
            let msg = format!("utf8 decode: {}", err);
            Error::custom(msg)
        })
    }

    impl StrAsBase64 for String {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&base64::encode(self.as_bytes()))
        }

        fn de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            finish_deserializing::<'de, D>(String::deserialize(deserializer)?)
        }
    }

    impl StrAsBase64 for Option<String> {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            match self {
                Some(s) => StrAsBase64::ser(s, serializer),
                None => serializer.serialize_none(),
            }
        }

        fn de<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            match Self::deserialize(deserializer)? {
                Some(s) => Ok(Some(finish_deserializing::<'de, D>(s)?)),
                None => Ok(None),
            }
        }
    }

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: StrAsBase64,
    {
        <T as StrAsBase64>::ser(data, serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: StrAsBase64,
    {
        <T as StrAsBase64>::de::<'de, D>(deserializer)
    }
}

/// Serialize `Vec<u8>` as base64url encoded string without padding.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox_serde::bytes_as_base64url_nopad")]
///     data: Vec<u8>,
/// }
///
/// let obj = Foo { data: vec![1, 2, 3, 4] };
/// let json = serde_json::to_string(&obj).unwrap();
/// assert_eq!(json, r#"{"data":"AQIDBA"}"#);
///
/// let deserialized: Foo = serde_json::from_str(&json).unwrap();
/// assert_eq!(obj, deserialized);
/// ```
pub mod bytes_as_base64url_nopad {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: AsRef<[u8]>,
        S: Serializer,
    {
        serializer.serialize_str(&base64::encode_config(
            data.as_ref(),
            base64::URL_SAFE_NO_PAD,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            base64::decode_config(string, base64::URL_SAFE_NO_PAD)
                .map_err(|err| Error::custom(err.to_string()))
        })
    }
}

/// Serialize `String` as base64url encoded string without padding.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox_serde::string_as_base64url_nopad")]
///     data: String,
/// }
///
/// let obj = Foo { data: "FOO".to_string() };
/// let json = serde_json::to_string(&obj).unwrap();
/// assert_eq!(json, r#"{"data":"Rk9P"}"#);
///
/// let deserialized: Foo = serde_json::from_str(&json).unwrap();
/// assert_eq!(obj, deserialized);
/// ```
pub mod string_as_base64url_nopad {
    use serde::Deserializer;

    pub use super::bytes_as_base64::serialize;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let bytes = super::bytes_as_base64::deserialize::<'de, D>(deserializer)?;
        String::from_utf8(bytes).map_err(|err| Error::custom(err.to_string()))
    }
}
