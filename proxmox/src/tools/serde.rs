//! Serialization helpers for serde

/// Serialize Unix epoch (i64) as RFC3339.
///
/// Usage example:
/// ```
/// # use proxmox::tools;
///
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox::tools::serde::epoch_as_rfc3339")]
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
        let s = crate::tools::time::epoch_to_rfc3339(*epoch)
            .map_err(|err| Error::custom(err.to_string()))?;

        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            crate::tools::time::parse_rfc3339(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

/// Serialize Vec<u8> as base64 encoded string.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox::tools::serde::bytes_as_base64")]
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
        String::deserialize(deserializer).and_then(|string| {
            base64::decode(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

/// Serialize String as base64 encoded string.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox::tools::serde::string_as_base64")]
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

    pub fn serialize<S>(data: &str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::encode(data.as_bytes()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let string = String::deserialize(deserializer)?;
        let bytes = base64::decode(&string).map_err(|err| {
            let msg = format!("base64 decode: {}", err.to_string());
            Error::custom(msg)
        })?;
        String::from_utf8(bytes).map_err(|err| {
            let msg = format!("utf8 decode: {}", err.to_string());
            Error::custom(msg)
        })
    }
}

/// Serialize Vec<u8> as base64url encoded string without padding.
///
/// Usage example:
/// ```
/// use serde::{Deserialize, Serialize};
///
/// # #[derive(Debug)]
/// #[derive(Deserialize, PartialEq, Serialize)]
/// struct Foo {
///     #[serde(with = "proxmox::tools::serde::bytes_as_base64url_nopad")]
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
            base64::decode_config(&string, base64::URL_SAFE_NO_PAD)
                .map_err(|err| Error::custom(err.to_string()))
        })
    }
}
