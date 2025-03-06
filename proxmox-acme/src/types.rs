//! Define types which are exposed with the proxmox API

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg_attr(feature = "api-types", proxmox_schema::api())]
/// External Account Bindings
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAccountBinding {
    /// JOSE Header (see RFC 7515)
    pub protected: String,
    /// Payload
    pub payload: String,
    /// HMAC signature
    pub signature: String,
}

/// Status of an ACME account.
#[cfg_attr(feature = "api-types", proxmox_schema::api())]
#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    /// This is not part of the ACME API, but a temporary marker for us until the ACME provider
    /// tells us the account's real status.
    #[serde(rename = "<invalid>")]
    New,

    /// Means the account is valid and can be used.
    Valid,

    /// The account has been deactivated by its user and cannot be used anymore.
    Deactivated,

    /// The account has been revoked by the server and cannot be used anymore.
    Revoked,
}

impl Default for AccountStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountStatus {
    /// Create a new instance with state New.
    #[inline]
    pub fn new() -> Self {
        AccountStatus::New
    }

    /// Return true if state is New
    #[inline]
    pub fn is_new(&self) -> bool {
        *self == AccountStatus::New
    }
}

#[inline]
fn default_true() -> bool {
    true
}

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg_attr(feature="api-types", proxmox_schema::api(
    properties: {
        extra: {
            type: Object,
            properties: {},
            additional_properties: true,
        },
        contact: {
            type: Array,
            items: {
                type: String,
                description: "Contact Info.",
            },
        },
    }
))]
/// ACME Account data. This is the part of the account returned from and possibly sent to the ACME
/// provider. Some fields may be uptdated by the user via a request to the account location, others
/// may not be changed.
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    /// The current account status.
    #[serde(
        skip_serializing_if = "AccountStatus::is_new",
        default = "AccountStatus::new"
    )]
    pub status: AccountStatus,

    /// URLs to currently pending orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders: Option<String>,

    /// The account's contact info.
    ///
    /// This usually contains a `"mailto:<email address>"` entry but may also contain some other
    /// data if the server accepts it.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contact: Vec<String>,

    /// Indicated whether the user agreed to the ACME provider's terms of service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service_agreed: Option<bool>,

    /// External account information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_account_binding: Option<ExternalAccountBinding>,

    /// This is only used by the client when querying an account.
    #[serde(default = "default_true", skip_serializing_if = "is_false")]
    pub only_return_existing: bool,

    /// Stores unknown fields if there are any.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}
