fn config() -> base64::Config {
    base64::Config::new(base64::CharacterSet::UrlSafe, false)
}

/// Encode bytes as base64url into a `String`.
pub fn encode(data: &[u8]) -> String {
    base64::encode_config(data, config())
}

// curiously currently unused as we don't deserialize any of that
// /// Decode bytes from a base64url string.
// pub fn decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
//     base64::decode_config(data, config())
// }

/// Our serde module for encoding bytes as base64url encoded strings.
pub mod bytes {
    use serde::{Serialize, Serializer};
    //use serde::{Deserialize, Deserializer};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        super::encode(data).serialize(serializer)
    }

    // curiously currently unused as we don't deserialize any of that
    // pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    // where
    //     D: Deserializer<'de>,
    // {
    //     use serde::de::Error;

    //     Ok(super::decode(&String::deserialize(deserializer)?)
    //         .map_err(|e| D::Error::custom(e.to_string()))?)
    // }
}
