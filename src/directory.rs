use serde::{Deserialize, Serialize};

pub struct Directory {
    pub url: String,
    pub data: DirectoryData,
}

/// The ACME Directory object structure.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryData {
    pub new_account: String,
    pub new_nonce: String,
    pub new_order: String,
    pub revoke_cert: String,
    pub key_change: String,
    pub meta: Meta,
}

/// The directory's "meta" object.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
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
