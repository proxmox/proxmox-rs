//! The generated API client code.

#[cfg(feature = "client")]
mod code;
#[cfg(feature = "client")]
pub use code::*;

#[cfg(feature = "hyper-client")]
mod hyper_client;
#[cfg(feature = "hyper-client")]
pub use hyper_client::{BadFingerprint, HyperClient, Options};

/// Add an optional string parameter to the query, and if it was added, change `separator` to `&`.
pub fn add_query_arg<T>(query: &mut String, separator: &mut char, name: &str, value: &Option<T>)
where
    T: std::fmt::Display,
{
    if let Some(value) = value {
        query.push(*separator);
        *separator = '&';
        query.push_str(name);
        query.push('=');
        query.extend(percent_encoding::percent_encode(
            value.to_string().as_bytes(),
            percent_encoding::NON_ALPHANUMERIC,
        ));
    }
}

/// Add an optional boolean parameter to the query, and if it was added, change `separator` to `&`.
pub fn add_query_bool(query: &mut String, separator: &mut char, name: &str, value: Option<bool>) {
    if let Some(value) = value {
        query.push(*separator);
        *separator = '&';
        query.push_str(name);
        query.push_str(if value { "=1" } else { "=0" });
    }
}
