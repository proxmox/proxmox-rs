use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::order::Identifier;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Deactivated,
    Expired,
    Invalid,
    Pending,
    Revoked,
    Valid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    pub identifier: Identifier,

    pub status: Status,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,

    pub challenges: Vec<Challenge>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub wildcard: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Challenge {
    #[serde(rename = "type")]
    pub ty: String,

    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

impl Challenge {
    /// Most challenges have a `token` used for key authorizations. This is a convenience helper to
    /// access it.
    pub fn token(&self) -> Option<&str> {
        self.data.get("token").and_then(Value::as_str)
    }
}

/// Serde helper
#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}
