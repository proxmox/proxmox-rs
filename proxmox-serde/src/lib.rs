//! Serialization helpers for serde

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
pub mod serde_macros;

#[cfg(feature = "serde_json")]
pub mod json;

#[cfg(feature = "perl")]
pub mod perl;

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

/// Mostly for backward compat and convenience, as one can normally use the newer [`proxmox_base64`]
/// directly.
pub use proxmox_base64::url::as_base64_no_pad_indifferent as bytes_as_base64url_nopad;

/// Mostly for backward compat and convenience, as one can normally use the newer [`proxmox_base64`]
/// directly.
pub use proxmox_base64::url::string_as_base64_no_pad_indifferent as string_as_base64url_nopad;

/// Mostly for backward compat and convenience, as one can normally use the newer [`proxmox_base64`]
/// directly.
pub use proxmox_base64::as_base64 as bytes_as_base64;

/// Serialize `String` or `Option<String>` as base64 encoded.
///
/// If you do not need the convenience of handling both String and Option transparently, you could
/// also use [`proxmox_base64`] directly.
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

        let bytes = proxmox_base64::decode(string).map_err(|err| {
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
            serializer.serialize_str(&proxmox_base64::encode(self.as_bytes()))
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
