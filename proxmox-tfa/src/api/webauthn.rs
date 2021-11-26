//! Webauthn configuration and challenge data.

use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::{api, Updater, UpdaterType};

use url::Url;

use webauthn_rs::proto::{COSEKey, Credential, CredentialID, UserVerificationPolicy};

use super::IsExpired;

#[derive(Clone, Deserialize, Serialize)]
/// Origin URL for WebauthnConfig
pub struct OriginUrl(Url);

#[cfg(feature = "api-types")]
impl UpdaterType for OriginUrl {
    type Updater = Option<Self>;
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        rp: { type: String },
        origin: { type: String },
        id: { type: String },
    }
))]
#[cfg_attr(feature = "api-types", derive(Updater))]
/// Server side webauthn server configuration.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebauthnConfig {
    /// Relying party name. Any text identifier.
    ///
    /// Changing this *may* break existing credentials.
    pub rp: String,

    /// Site origin. Must be a `https://` URL (or `http://localhost`). Should contain the address
    /// users type in their browsers to access the web interface.
    ///
    /// Changing this *may* break existing credentials.
    pub origin: OriginUrl,

    /// Relying party ID. Must be the domain name without protocol, port or location.
    ///
    /// Changing this *will* break existing credentials.
    pub id: String,
}

/// For now we just implement this on the configuration this way.
///
/// Note that we may consider changing this so `get_origin` returns the `Host:` header provided by
/// the connecting client.
impl webauthn_rs::WebauthnConfig for WebauthnConfig {
    fn get_relying_party_name(&self) -> &str {
        &self.rp
    }

    fn get_origin(&self) -> &Url {
        &self.origin.0
    }

    fn get_relying_party_id(&self) -> &str {
        &self.id
    }
}

/// A webauthn registration challenge.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebauthnRegistrationChallenge {
    /// Server side registration state data.
    pub(super) state: webauthn_rs::RegistrationState,

    /// While this is basically the content of a `RegistrationState`, the webauthn-rs crate doesn't
    /// make this public.
    pub(super) challenge: String,

    /// The description chosen by the user for this registration.
    pub(super) description: String,

    /// When the challenge was created as unix epoch. They are supposed to be short-lived.
    created: i64,
}

impl WebauthnRegistrationChallenge {
    pub fn new(
        state: webauthn_rs::RegistrationState,
        challenge: String,
        description: String,
    ) -> Self {
        Self {
            state,
            challenge,
            description,
            created: proxmox_time::epoch_i64(),
        }
    }
}

impl IsExpired for WebauthnRegistrationChallenge {
    fn is_expired(&self, at_epoch: i64) -> bool {
        self.created < at_epoch
    }
}

/// A webauthn authentication challenge.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebauthnAuthChallenge {
    /// Server side authentication state.
    pub(super) state: webauthn_rs::AuthenticationState,

    /// While this is basically the content of a `AuthenticationState`, the webauthn-rs crate
    /// doesn't make this public.
    pub(super) challenge: String,

    /// When the challenge was created as unix epoch. They are supposed to be short-lived.
    created: i64,
}

impl WebauthnAuthChallenge {
    pub fn new(state: webauthn_rs::AuthenticationState, challenge: String) -> Self {
        Self {
            state,
            challenge,
            created: proxmox_time::epoch_i64(),
        }
    }
}

impl IsExpired for WebauthnAuthChallenge {
    fn is_expired(&self, at_epoch: i64) -> bool {
        self.created < at_epoch
    }
}

/// A webauthn credential
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebauthnCredential {
    /// The ID of this credential.
    pub cred_id: CredentialID,
    /// The public key of this credential
    pub cred: COSEKey,
    /// The counter for this credential
    pub counter: u32,
}

/// ignores verified and registration_policy fields for now
impl From<Credential> for WebauthnCredential {
    fn from(cred: Credential) -> Self {
        Self {
            cred_id: cred.cred_id,
            cred: cred.cred,
            counter: cred.counter,
        }
    }
}

/// always sets verified to false and registration_policy to Discouraged for now
impl From<WebauthnCredential> for Credential {
    fn from(val: WebauthnCredential) -> Self {
        Credential {
            cred_id: val.cred_id,
            cred: val.cred,
            counter: val.counter,
            verified: false,
            registration_policy: UserVerificationPolicy::Discouraged,
        }
    }
}
