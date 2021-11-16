//! u2f configuration and challenge data

use serde::{Deserialize, Serialize};

use crate::u2f;

pub use crate::u2f::{Registration, U2f};

/// The U2F authentication configuration.
#[derive(Clone, Deserialize, Serialize)]
pub struct U2fConfig {
    pub appid: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// A u2f registration challenge.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct U2fRegistrationChallenge {
    /// JSON formatted challenge string.
    pub challenge: String,

    /// The description chosen by the user for this registration.
    pub description: String,

    /// When the challenge was created as unix epoch. They are supposed to be short-lived.
    created: i64,
}

impl super::IsExpired for U2fRegistrationChallenge {
    fn is_expired(&self, at_epoch: i64) -> bool {
        self.created < at_epoch
    }
}

impl U2fRegistrationChallenge {
    pub fn new(challenge: String, description: String) -> Self {
        Self {
            challenge,
            description,
            created: proxmox_time::epoch_i64(),
        }
    }
}

/// Data used for u2f authentication challenges.
///
/// This is sent to the client at login time.
#[derive(Deserialize, Serialize)]
pub struct U2fChallenge {
    /// AppID and challenge data.
    pub(super) challenge: u2f::AuthChallenge,

    /// Available tokens/keys.
    pub(super) keys: Vec<u2f::RegisteredKey>,
}

/// The challenge data we need on the server side to verify the challenge:
/// * It can only be used once.
/// * It can expire.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct U2fChallengeEntry {
    challenge: u2f::AuthChallenge,
    created: i64,
}

impl U2fChallengeEntry {
    pub fn new(challenge: &U2fChallenge) -> Self {
        Self {
            challenge: challenge.challenge.clone(),
            created: proxmox_time::epoch_i64(),
        }
    }
}

impl super::IsExpired for U2fChallengeEntry {
    fn is_expired(&self, at_epoch: i64) -> bool {
        self.created < at_epoch
    }
}

impl PartialEq<u2f::AuthChallenge> for U2fChallengeEntry {
    fn eq(&self, other: &u2f::AuthChallenge) -> bool {
        self.challenge.challenge == other.challenge && self.challenge.app_id == other.app_id
    }
}
