//! ACME Directory information.

use serde::{Deserialize, Serialize};

/// An ACME Directory. This contains the base URL and the directory data as received via a `GET`
/// request to the URL.
pub struct Directory {
    /// The main entry point URL to the ACME directory.
    pub url: String,

    /// The json structure received via a `GET` request to the directory URL. This contains the
    /// URLs for various API entry points.
    pub data: DirectoryData,
}

/// The ACME Directory object structure.
///
/// The data in here is typically not relevant to the user of this crate.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryData {
    /// The entry point to create a new account.
    pub new_account: String,

    /// The entry point to retrieve a new nonce, should be used with a `HEAD` request.
    pub new_nonce: String,

    /// URL to post new orders to.
    pub new_order: String,

    /// URL to use for certificate revocation.
    pub revoke_cert: String,

    /// Account key rollover URL.
    pub key_change: String,

    /// Metadata object, for additional information which aren't directly part of the API
    /// itself, such as the terms of service.
    pub meta: Meta,
}

/// The directory's "meta" object.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// The terms of service. This is typically in the form of an URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,
}

impl Directory {
    /// Create a `Directory` given the parsed `DirectoryData` of a `GET` request to the directory
    /// URL.
    pub fn from_parts(url: String, data: DirectoryData) -> Self {
        Self { url, data }
    }

    /// Get the ToS URL.
    pub fn terms_of_service_url(&self) -> Option<&str> {
        self.data.meta.terms_of_service.as_deref()
    }

    /// Get the "newNonce" URL. Use `HEAD` requests on this to get a new nonce.
    pub fn new_nonce_url(&self) -> &str {
        &self.data.new_nonce
    }

    pub(crate) fn new_account_url(&self) -> &str {
        &self.data.new_account
    }

    pub(crate) fn new_order_url(&self) -> &str {
        &self.data.new_order
    }

    /// Access to the in the Acme spec defined metadata structure.
    /// Currently only contains the ToS URL already exposed via the `terms_of_service_url()`
    /// method.
    pub fn meta(&self) -> &Meta {
        &self.data.meta
    }
}
