//! Webauthn configuration and challenge data.

use std::fmt;

use anyhow::{format_err, Context as _, Error};
use serde::{Deserialize, Serialize};
use url::Url;
use webauthn_rs::prelude::SecurityKeyAuthentication;
use webauthn_rs::prelude::SecurityKeyRegistration;
use webauthn_rs_core::proto::{COSEKey, CredentialID, UserVerificationPolicy};

#[cfg(feature = "api-types")]
use proxmox_schema::{api, Updater, UpdaterType};

use super::IsExpired;

#[derive(Clone, Deserialize)]
/// Origin URL for WebauthnConfig
pub struct OriginUrl(Url);

impl serde::Serialize for OriginUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "api-types")]
impl UpdaterType for OriginUrl {
    type Updater = Option<Self>;
}

impl std::str::FromStr for OriginUrl {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(Self(s.parse()?))
    }
}

impl From<OriginUrl> for String {
    fn from(url: OriginUrl) -> String {
        url.to_string()
    }
}

impl fmt::Display for OriginUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.origin().ascii_serialization())
    }
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        rp: { type: String },
        origin: { type: String, optional: true },
        id: { type: String },
    }
))]
#[cfg_attr(feature = "api-types", derive(Updater))]
/// Server side webauthn server configuration.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct WebauthnConfig {
    /// Relying party name. Any text identifier.
    ///
    /// Changing this *may* break existing credentials.
    pub rp: String,

    /// Site origin. Must be a `https://` URL (or `http://localhost`). Should contain the address
    /// users type in their browsers to access the web interface.
    ///
    /// Changing this *may* break existing credentials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<OriginUrl>,

    /// Relying party ID. Must be the domain name without protocol, port or location.
    ///
    /// Changing this *will* break existing credentials.
    pub id: String,

    /// If an `origin` is specified, this specifies whether subdomains should be considered valid
    /// as well.
    ///
    /// May be changed at any time.
    ///
    /// Defaults to `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_subdomains: Option<bool>,
}

impl WebauthnConfig {
    pub fn digest(&self) -> [u8; 32] {
        let mut data = format!("rp={:?}\nid={:?}\n", self.rp, self.id,);
        if let Some(origin) = &self.origin {
            data.push_str(&format!("origin={}\n", origin));
        }
        openssl::sha::sha256(data.as_bytes())
    }

    /// Instantiate a usable webauthn configuration instance.
    pub(super) fn instantiate<'a, 'this: 'a, 'origin: 'a>(
        &'this self,
        origin: Option<&'origin Url>,
    ) -> Result<WebauthnConfigInstance<'a>, Error> {
        Ok(WebauthnConfigInstance {
            origin: origin
                .or_else(|| self.origin.as_ref().map(|u| &u.0))
                .ok_or_else(|| format_err!("missing webauthn origin"))?,
            rp: &self.rp,
            id: &self.id,
            allow_subdomains: self.allow_subdomains.unwrap_or(true),
        })
    }
}

pub(super) struct WebauthnConfigInstance<'a> {
    pub rp: &'a str,
    pub origin: &'a Url,
    pub id: &'a str,
    pub allow_subdomains: bool,
}

/// A webauthn registration challenge.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebauthnRegistrationChallenge {
    /// Server side registration state data.
    pub(super) state: SecurityKeyRegistration,

    /// While this is basically the content of a `SecurityKeyRegistration`, the webauthn-rs crate doesn't
    /// make this public.
    pub(super) challenge: String,

    /// The description chosen by the user for this registration.
    pub(super) description: String,

    /// When the challenge was created as unix epoch. They are supposed to be short-lived.
    created: i64,
}

impl WebauthnRegistrationChallenge {
    pub fn new(state: SecurityKeyRegistration, challenge: String, description: String) -> Self {
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
    pub(super) state: SecurityKeyAuthentication,

    /// While this is basically the content of a `SecurityKeyAuthentication`, the webauthn-rs crate
    /// doesn't make this public.
    pub(super) challenge: String,

    /// When the challenge was created as unix epoch. They are supposed to be short-lived.
    created: i64,
}

impl WebauthnAuthChallenge {
    pub fn new(mut state: SecurityKeyAuthentication, challenge: String) -> Result<Self, Error> {
        state = force_allow_backup_eligibility(state)?;

        Ok(Self {
            state,
            challenge,
            created: proxmox_time::epoch_i64(),
        })
    }
}

impl IsExpired for WebauthnAuthChallenge {
    fn is_expired(&self, at_epoch: i64) -> bool {
        self.created < at_epoch
    }
}

fn force_allow_backup_eligibility(
    state: SecurityKeyAuthentication,
) -> Result<SecurityKeyAuthentication, Error> {
    let mut state =
        serde_json::to_value(&state).context("failed to convert authentication state to json")?;
    if let Some(obj) = state.get_mut("ast") {
        if let Some(allow) = obj.get_mut("allow_backup_eligible_upgrade") {
            *allow = serde_json::Value::Bool(true);
        }
    }

    serde_json::from_value(state).context("failed to convert json back to authentication state")
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
    /// The eligibility for createing backups.
    #[serde(default, skip_serializing_if = "is_false")]
    pub backup_eligible: bool,
    /// The backup state.
    #[serde(default, skip_serializing_if = "is_false")]
    pub backup_state: bool,
}

fn is_false(b: &bool) -> bool {
    !b
}

impl From<webauthn_rs::prelude::SecurityKey> for WebauthnCredential {
    fn from(cred: webauthn_rs::prelude::SecurityKey) -> Self {
        Self::from(webauthn_rs_core::proto::Credential::from(cred))
    }
}

impl From<WebauthnCredential> for webauthn_rs::prelude::SecurityKey {
    fn from(cred: WebauthnCredential) -> Self {
        Self::from(webauthn_rs_core::proto::Credential::from(cred))
    }
}

/// ignores verified and registration_policy fields for now
impl From<webauthn_rs_core::proto::Credential> for WebauthnCredential {
    fn from(cred: webauthn_rs_core::proto::Credential) -> Self {
        Self {
            cred_id: cred.cred_id.into(),
            cred: cred.cred,
            counter: cred.counter,
            backup_eligible: cred.backup_eligible,
            backup_state: cred.backup_state,
        }
    }
}

/// always sets verified to false and registration_policy to Discouraged for now
impl From<WebauthnCredential> for webauthn_rs_core::proto::Credential {
    fn from(cred: WebauthnCredential) -> Self {
        Self {
            cred_id: cred.cred_id.into(),
            cred: cred.cred,
            counter: cred.counter,
            transports: None,
            user_verified: false,
            backup_eligible: cred.backup_eligible,
            backup_state: cred.backup_state,
            registration_policy: UserVerificationPolicy::Discouraged_DO_NOT_USE,
            extensions: webauthn_rs_core::proto::RegisteredExtensions::none(),
            attestation: webauthn_rs_core::proto::ParsedAttestation {
                data: webauthn_rs_core::proto::ParsedAttestationData::None,
                metadata: webauthn_rs_core::proto::AttestationMetadata::None,
            },
            attestation_format: webauthn_rs_core::proto::AttestationFormat::None,
        }
    }
}
