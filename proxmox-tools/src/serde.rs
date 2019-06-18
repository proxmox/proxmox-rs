//! Serialization helpers for serde

/// Sertialize DateTime<Local> as RFC3339
pub mod date_time_as_rfc3339 {

    use chrono::{Local, DateTime};
    use serde::{Serializer, Deserializer, Deserialize};

    pub fn serialize<S>(time: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_str(&time.to_rfc3339())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
        where D: Deserializer<'de>
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|string| {
                string.parse::<DateTime<Local>>()
                    .map_err(|err| Error::custom(err.to_string()))
            })
    }
}


/// Serialize Vec<u8> as base64 encoded string.
pub mod bytes_as_base64 {

    use base64;
    use serde::{Serializer,Deserializer, Deserialize};

    pub fn serialize<S, T>(data: &T, serializer: S) -> Result<S::Ok, S::Error>
        where T: AsRef<[u8]>,
              S: Serializer,
    {
        serializer.serialize_str(&base64::encode(data.as_ref()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
        where D: Deserializer<'de>
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|string| {
                base64::decode(&string)
                    .map_err(|err| Error::custom(err.to_string()))
            })
    }
}
