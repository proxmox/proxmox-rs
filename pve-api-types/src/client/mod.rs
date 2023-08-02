//! The generated API client code.

use proxmox_client::Client as ProxmoxClient;
use proxmox_client::{ApiResponse, Environment, Error, HttpClient};

use crate::types::*;

pub struct Client<C, E: Environment> {
    client: ProxmoxClient<C, E>,
}

impl<C, E: Environment> Client<C, E> {
    /// Get the underlying client object.
    pub fn inner(&self) -> &ProxmoxClient<C, E> {
        &self.client
    }

    /// Get a mutable reference to the underlying client object.
    pub fn inner_mut(&mut self) -> &mut ProxmoxClient<C, E> {
        &mut self.client
    }
}

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

include!("../generated/code.rs");
