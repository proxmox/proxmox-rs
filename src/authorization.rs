use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::order::Identifier;
use crate::request::Request;
use crate::Error;

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

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChallengeStatus {
    Pending,
    Processing,
    Valid,
    Invalid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Challenge {
    #[serde(rename = "type")]
    pub ty: String,

    pub status: ChallengeStatus,

    pub url: String,

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

/// Represents an in-flight query for an authorization.
///
/// This is created via [`Account::get_authorization`].
pub struct GetAuthorization {
    //order: OrderData,
    pub request: Option<Request>,
}

impl GetAuthorization {
    pub(crate) fn new(request: Request) -> Self {
        Self {
            request: Some(request),
        }
    }

    /// Deal with the response we got from the server.
    pub fn response(self, response_body: &[u8]) -> Result<Authorization, Error> {
        Ok(serde_json::from_slice(response_body)?)
    }
}
