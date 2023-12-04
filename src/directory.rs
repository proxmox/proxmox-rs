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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// The directory's "meta" object.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// The terms of service. This is typically in the form of an URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,

    /// Flag indicating if EAB is required, None is equivalent to false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_account_required: Option<bool>,

    /// Website with information about the ACME Server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,

    /// List of hostnames used by the CA, intended for the use with caa dns records
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caa_identities: Vec<String>,
}

impl Directory {
    /// Create a `Directory` given the parsed `DirectoryData` of a `GET` request to the directory
    /// URL.
    pub fn from_parts(url: String, data: DirectoryData) -> Self {
        Self { url, data }
    }

    /// Get the ToS URL.
    pub fn terms_of_service_url(&self) -> Option<&str> {
        match &self.data.meta {
            Some(meta) => meta.terms_of_service.as_deref(),
            None => None,
        }
    }

    /// Get if external account binding is required
    pub fn external_account_binding_required(&self) -> bool {
        matches!(
            &self.data.meta,
            Some(Meta {
                external_account_required: Some(true),
                ..
            })
        )
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
    pub fn meta(&self) -> Option<&Meta> {
        self.data.meta.as_ref()
    }
}
