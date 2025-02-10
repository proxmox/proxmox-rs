//! The generated API client code.

#[cfg(feature = "client")]
mod code;
#[cfg(feature = "client")]
pub use code::*;

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

/// For `<type>-list` entries we turn an array into a string ready for perl's `split_list()` call.
pub fn add_query_arg_string_list<T>(
    query: &mut String,
    separator: &mut char,
    name: &str,
    value: &Option<Vec<T>>,
) where
    T: std::fmt::Display,
{
    let Some(value) = value else { return };
    query.push(*separator);
    *separator = '&';
    query.push_str(name);
    query.push('=');

    let mut separator = "";
    for entry in value {
        query.push_str(separator);
        separator = "%00";
        query.extend(percent_encoding::percent_encode(
            entry.to_string().as_bytes(),
            percent_encoding::NON_ALPHANUMERIC,
        ));
    }
}
