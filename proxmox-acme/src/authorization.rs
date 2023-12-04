//! Authorization and Challenge data.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::order::Identifier;
use crate::request::Request;
use crate::Error;

/// Status of an [`Authorization`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The authorization was deactivated by the client.
    Deactivated,

    /// The authorization expired.
    Expired,

    /// The authorization failed and is now invalid.
    Invalid,

    /// Validation is pending.
    Pending,

    /// The authorization was revoked by the server.
    Revoked,

    /// The identifier is authorized.
    Valid,
}

impl Status {
    /// Convenience method to check if the status is 'pending'.
    #[inline]
    pub fn is_pending(self) -> bool {
        self == Status::Pending
    }

    /// Convenience method to check if the status is 'valid'.
    #[inline]
    pub fn is_valid(self) -> bool {
        self == Status::Valid
    }
}

/// Represents an authorization state for an order. The user is expected to pick a challenge,
/// execute it, and the request validation for it.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    /// The identifier (usually domain name) this authorization is for.
    pub identifier: Identifier,

    /// The current status of this authorization entry.
    pub status: Status,

    /// Expiration date for the authorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,

    /// List of challenges which can be used to complete this authorization.
    pub challenges: Vec<Challenge>,

    /// The authorization is for a wildcard domain.
    #[serde(default, skip_serializing_if = "is_false")]
    pub wildcard: bool,
}

/// The state of a challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChallengeStatus {
    /// The challenge is pending and has not been validated yet.
    Pending,

    /// The valiation is in progress.
    Processing,

    /// The challenge was successfully validated.
    Valid,

    /// Validation of this challenge failed.
    Invalid,
}

impl ChallengeStatus {
    /// Convenience method to check if the status is 'pending'.
    #[inline]
    pub fn is_pending(self) -> bool {
        self == ChallengeStatus::Pending
    }

    /// Convenience method to check if the status is 'valid'.
    #[inline]
    pub fn is_valid(self) -> bool {
        self == ChallengeStatus::Valid
    }
}

/// A challenge object contains information on how to complete an authorization for an order.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Challenge {
    /// The challenge type (such as `"dns-01"`).
    #[serde(rename = "type")]
    pub ty: String,

    /// The current challenge status.
    pub status: ChallengeStatus,

    /// The URL used to post to in order to begin the validation for this challenge.
    pub url: String,

    /// Contains the remaining fields of the Challenge object, such as the `token`.
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
/// This is created via [`Account::get_authorization`](crate::Account::get_authorization()).
pub struct GetAuthorization {
    //order: OrderData,
    /// The request to send to the ACME provider. This is wrapped in an option in order to allow
    /// moving it out instead of copying the contents.
    ///
    /// When generated via [`Account::get_authorization`](crate::Account::get_authorization()),
    /// this is guaranteed to be `Some`.
    ///
    /// The response should be passed to the the [`response`](GetAuthorization::response()) method.
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
