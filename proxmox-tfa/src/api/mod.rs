//! TFA configuration and user data.
//!
//! This is the same as used in PBS but without the `#[api]` type.
//!
//! We may want to move this into a shared crate making the `#[api]` macro feature-gated!

use std::collections::HashMap;

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use webauthn_rs::{proto::UserVerificationPolicy, Webauthn};

use crate::totp::Totp;
use proxmox_uuid::Uuid;

#[cfg(feature = "api-types")]
use proxmox_schema::api;

mod serde_tools;

mod recovery;
mod u2f;
mod webauthn;

pub mod methods;

pub use recovery::RecoveryState;
pub use u2f::U2fConfig;
pub use webauthn::{WebauthnConfig, WebauthnCredential};

#[cfg(feature = "api-types")]
pub use webauthn::WebauthnConfigUpdater;

use recovery::Recovery;
use u2f::{U2fChallenge, U2fChallengeEntry, U2fRegistrationChallenge};
use webauthn::{WebauthnAuthChallenge, WebauthnRegistrationChallenge};

trait IsExpired {
    fn is_expired(&self, at_epoch: i64) -> bool;
}

pub trait OpenUserChallengeData: Clone {
    type Data: UserChallengeAccess;

    fn open(&self, userid: &str) -> Result<Self::Data, Error>;

    fn open_no_create(&self, userid: &str) -> Result<Option<Self::Data>, Error>;

    /// Should return `true` if something was removed, `false` if no data existed for the user.
    fn remove(&self, userid: &str) -> Result<bool, Error>;
}

pub trait UserChallengeAccess: Sized {
    //fn open(userid: &str) -> Result<Self, Error>;
    //fn open_no_create(userid: &str) -> Result<Option<Self>, Error>;
    fn get_mut(&mut self) -> &mut TfaUserChallenges;
    fn save(self) -> Result<(), Error>;
}

const CHALLENGE_TIMEOUT_SECS: i64 = 2 * 60;

/// TFA Configuration for this instance.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct TfaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u2f: Option<U2fConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub webauthn: Option<WebauthnConfig>,

    #[serde(skip_serializing_if = "TfaUsers::is_empty", default)]
    pub users: TfaUsers,
}

/// Helper to get a u2f instance from a u2f config, or `None` if there isn't one configured.
fn get_u2f(u2f: &Option<U2fConfig>) -> Option<u2f::U2f> {
    u2f.as_ref().map(|cfg| {
        u2f::U2f::new(
            cfg.appid.clone(),
            cfg.origin.clone().unwrap_or_else(|| cfg.appid.clone()),
        )
    })
}

/// Helper to get a u2f instance from a u2f config.
///
/// This is outside of `TfaConfig` to not borrow its `&self`.
fn check_u2f(u2f: &Option<U2fConfig>) -> Result<u2f::U2f, Error> {
    get_u2f(u2f).ok_or_else(|| format_err!("no u2f configuration available"))
}

/// Helper to get a `Webauthn` instance from a `WebauthnConfig`, or `None` if there isn't one
/// configured.
fn get_webauthn(waconfig: &Option<WebauthnConfig>) -> Option<Webauthn<WebauthnConfig>> {
    waconfig.clone().map(Webauthn::new)
}

/// Helper to get a u2f instance from a u2f config.
///
/// This is outside of `TfaConfig` to not borrow its `&self`.
fn check_webauthn(waconfig: &Option<WebauthnConfig>) -> Result<Webauthn<WebauthnConfig>, Error> {
    get_webauthn(waconfig).ok_or_else(|| format_err!("no webauthn configuration available"))
}

impl TfaConfig {
    // Get a u2f registration challenge.
    pub fn u2f_registration_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        description: String,
    ) -> Result<String, Error> {
        let u2f = check_u2f(&self.u2f)?;

        self.users
            .entry(userid.to_owned())
            .or_default()
            .u2f_registration_challenge(access, userid, &u2f, description)
    }

    /// Finish a u2f registration challenge.
    pub fn u2f_registration_finish<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        challenge: &str,
        response: &str,
    ) -> Result<String, Error> {
        let u2f = check_u2f(&self.u2f)?;

        match self.users.get_mut(userid) {
            Some(user) => user.u2f_registration_finish(access, userid, &u2f, challenge, response),
            None => bail!("no such challenge"),
        }
    }

    /// Get a webauthn registration challenge.
    pub fn webauthn_registration_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        user: &str,
        description: String,
    ) -> Result<String, Error> {
        let webauthn = check_webauthn(&self.webauthn)?;

        self.users
            .entry(user.to_owned())
            .or_default()
            .webauthn_registration_challenge(access, webauthn, user, description)
    }

    /// Finish a webauthn registration challenge.
    pub fn webauthn_registration_finish<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        challenge: &str,
        response: &str,
    ) -> Result<String, Error> {
        let webauthn = check_webauthn(&self.webauthn)?;

        let response: webauthn_rs::proto::RegisterPublicKeyCredential =
            serde_json::from_str(response)
                .map_err(|err| format_err!("error parsing challenge response: {}", err))?;

        match self.users.get_mut(userid) {
            Some(user) => {
                user.webauthn_registration_finish(access, webauthn, userid, challenge, response)
            }
            None => bail!("no such challenge"),
        }
    }

    /// Add a TOTP entry for a user.
    ///
    /// Unlike U2F/WA, this does not require a challenge/response. The user can choose their secret
    /// themselves.
    pub fn add_totp(&mut self, userid: &str, description: String, value: Totp) -> String {
        self.users
            .entry(userid.to_owned())
            .or_default()
            .add_totp(description, value)
    }

    /// Add a Yubico key to a user.
    ///
    /// Unlike U2F/WA, this does not require a challenge/response. The user can choose their secret
    /// themselves.
    pub fn add_yubico(&mut self, userid: &str, description: String, key: String) -> String {
        self.users
            .entry(userid.to_owned())
            .or_default()
            .add_yubico(description, key)
    }

    /// Add a new set of recovery keys. There can only be 1 set of keys at a time.
    pub fn add_recovery(&mut self, userid: &str) -> Result<Vec<String>, Error> {
        self.users
            .entry(userid.to_owned())
            .or_default()
            .add_recovery()
    }

    /// Get a two factor authentication challenge for a user, if the user has TFA set up.
    pub fn authentication_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
    ) -> Result<Option<TfaChallenge>, Error> {
        match self.users.get_mut(userid) {
            Some(udata) => udata.challenge(
                access,
                userid,
                get_webauthn(&self.webauthn),
                get_u2f(&self.u2f).as_ref(),
            ),
            None => Ok(None),
        }
    }

    /// Verify a TFA challenge.
    pub fn verify<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        challenge: &TfaChallenge,
        response: TfaResponse,
    ) -> Result<NeedsSaving, Error> {
        match self.users.get_mut(userid) {
            Some(user) => match response {
                TfaResponse::Totp(value) => user.verify_totp(&value),
                TfaResponse::U2f(value) => match &challenge.u2f {
                    Some(challenge) => {
                        let u2f = check_u2f(&self.u2f)?;
                        user.verify_u2f(access.clone(), userid, u2f, &challenge.challenge, value)
                    }
                    None => bail!("no u2f factor available for user '{}'", userid),
                },
                TfaResponse::Webauthn(value) => {
                    let webauthn = check_webauthn(&self.webauthn)?;
                    user.verify_webauthn(access.clone(), userid, webauthn, value)
                }
                TfaResponse::Recovery(value) => {
                    user.verify_recovery(&value)?;
                    return Ok(NeedsSaving::Yes);
                }
            },
            None => bail!("no 2nd factor available for user '{}'", userid),
        }?;

        Ok(NeedsSaving::No)
    }

    pub fn remove_user<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
    ) -> Result<NeedsSaving, Error> {
        let mut save = access.remove(userid)?;
        if self.users.remove(userid).is_some() {
            save = true;
        }
        Ok(save.into())
    }
}

#[must_use = "must save the config in order to ensure one-time use of recovery keys"]
#[derive(Clone, Copy)]
pub enum NeedsSaving {
    No,
    Yes,
}

impl NeedsSaving {
    /// Convenience method so we don't need to import the type name.
    pub fn needs_saving(self) -> bool {
        matches!(self, NeedsSaving::Yes)
    }
}

impl From<bool> for NeedsSaving {
    fn from(v: bool) -> Self {
        if v {
            NeedsSaving::Yes
        } else {
            NeedsSaving::No
        }
    }
}

/// Mapping of userid to TFA entry.
pub type TfaUsers = HashMap<String, TfaUserData>;

/// TFA data for a user.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct TfaUserData {
    /// Totp keys for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub totp: Vec<TfaEntry<Totp>>,

    /// Registered u2f tokens for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub u2f: Vec<TfaEntry<u2f::Registration>>,

    /// Registered webauthn tokens for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub webauthn: Vec<TfaEntry<WebauthnCredential>>,

    /// Recovery keys. (Unordered OTP values).
    #[serde(skip_serializing_if = "Recovery::option_is_empty", default)]
    pub recovery: Option<Recovery>,

    /// Yubico keys for a user. NOTE: This is not directly supported currently, we just need this
    /// available for PVE, where the yubico API server configuration is part if the realm.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub yubico: Vec<TfaEntry<String>>,
}

impl TfaUserData {
    /// Shortcut to get the recovery entry only if it is not empty!
    pub fn recovery(&self) -> Option<&Recovery> {
        if Recovery::option_is_empty(&self.recovery) {
            None
        } else {
            self.recovery.as_ref()
        }
    }

    /// `true` if no second factors exist
    pub fn is_empty(&self) -> bool {
        self.totp.is_empty()
            && self.u2f.is_empty()
            && self.webauthn.is_empty()
            && self.yubico.is_empty()
            && self.recovery().is_none()
    }

    /// Find an entry by id, except for the "recovery" entry which we're currently treating
    /// specially.
    pub fn find_entry_mut<'a>(&'a mut self, id: &str) -> Option<&'a mut TfaInfo> {
        for entry in &mut self.totp {
            if entry.info.id == id {
                return Some(&mut entry.info);
            }
        }

        for entry in &mut self.webauthn {
            if entry.info.id == id {
                return Some(&mut entry.info);
            }
        }

        for entry in &mut self.u2f {
            if entry.info.id == id {
                return Some(&mut entry.info);
            }
        }

        for entry in &mut self.yubico {
            if entry.info.id == id {
                return Some(&mut entry.info);
            }
        }

        None
    }

    /// Create a u2f registration challenge.
    ///
    /// The description is required at this point already mostly to better be able to identify such
    /// challenges in the tfa config file if necessary. The user otherwise has no access to this
    /// information at this point, as the challenge is identified by its actual challenge data
    /// instead.
    fn u2f_registration_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        u2f: &u2f::U2f,
        description: String,
    ) -> Result<String, Error> {
        let challenge = serde_json::to_string(&u2f.registration_challenge()?)?;

        let mut data = access.open(userid)?;
        data.get_mut()
            .u2f_registrations
            .push(U2fRegistrationChallenge::new(
                challenge.clone(),
                description,
            ));
        data.save()?;

        Ok(challenge)
    }

    fn u2f_registration_finish<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        u2f: &u2f::U2f,
        challenge: &str,
        response: &str,
    ) -> Result<String, Error> {
        let mut data = access.open(userid)?;
        let entry = data
            .get_mut()
            .u2f_registration_finish(u2f, challenge, response)?;
        data.save()?;

        let id = entry.info.id.clone();
        self.u2f.push(entry);
        Ok(id)
    }

    /// Create a webauthn registration challenge.
    ///
    /// The description is required at this point already mostly to better be able to identify such
    /// challenges in the tfa config file if necessary. The user otherwise has no access to this
    /// information at this point, as the challenge is identified by its actual challenge data
    /// instead.
    fn webauthn_registration_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        webauthn: Webauthn<WebauthnConfig>,
        userid: &str,
        description: String,
    ) -> Result<String, Error> {
        let cred_ids: Vec<_> = self
            .enabled_webauthn_entries()
            .map(|cred| cred.cred_id.clone())
            .collect();

        let (challenge, state) = webauthn.generate_challenge_register_options(
            userid.as_bytes().to_vec(),
            userid.to_owned(),
            userid.to_owned(),
            Some(cred_ids),
            Some(UserVerificationPolicy::Discouraged),
            None,
        )?;

        let challenge_string = challenge.public_key.challenge.to_string();
        let challenge = serde_json::to_string(&challenge)?;

        let mut data = access.open(userid)?;
        data.get_mut()
            .webauthn_registrations
            .push(WebauthnRegistrationChallenge::new(
                state,
                challenge_string,
                description,
            ));
        data.save()?;

        Ok(challenge)
    }

    /// Finish a webauthn registration. The challenge should correspond to an output of
    /// `webauthn_registration_challenge`. The response should come directly from the client.
    fn webauthn_registration_finish<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        webauthn: Webauthn<WebauthnConfig>,
        userid: &str,
        challenge: &str,
        response: webauthn_rs::proto::RegisterPublicKeyCredential,
    ) -> Result<String, Error> {
        let mut data = access.open(userid)?;
        let entry = data.get_mut().webauthn_registration_finish(
            webauthn,
            challenge,
            response,
            &self.webauthn,
        )?;
        data.save()?;

        let id = entry.info.id.clone();
        self.webauthn.push(entry);
        Ok(id)
    }

    fn add_totp(&mut self, description: String, totp: Totp) -> String {
        let entry = TfaEntry::new(description, totp);
        let id = entry.info.id.clone();
        self.totp.push(entry);
        id
    }

    fn add_yubico(&mut self, description: String, key: String) -> String {
        let entry = TfaEntry::new(description, key);
        let id = entry.info.id.clone();
        self.yubico.push(entry);
        id
    }

    /// Add a new set of recovery keys. There can only be 1 set of keys at a time.
    fn add_recovery(&mut self) -> Result<Vec<String>, Error> {
        if self.recovery.is_some() {
            bail!("user already has recovery keys");
        }

        let (recovery, original) = Recovery::generate()?;

        self.recovery = Some(recovery);

        Ok(original)
    }

    /// Helper to iterate over enabled totp entries.
    fn enabled_totp_entries(&self) -> impl Iterator<Item = &Totp> {
        self.totp
            .iter()
            .filter_map(|e| if e.info.enable { Some(&e.entry) } else { None })
    }

    /// Helper to iterate over enabled u2f entries.
    fn enabled_u2f_entries(&self) -> impl Iterator<Item = &u2f::Registration> {
        self.u2f
            .iter()
            .filter_map(|e| if e.info.enable { Some(&e.entry) } else { None })
    }

    /// Helper to iterate over enabled u2f entries.
    fn enabled_webauthn_entries(&self) -> impl Iterator<Item = &WebauthnCredential> {
        self.webauthn
            .iter()
            .filter_map(|e| if e.info.enable { Some(&e.entry) } else { None })
    }

    /// Helper to iterate over enabled yubico entries.
    pub fn enabled_yubico_entries(&self) -> impl Iterator<Item = &str> {
        self.yubico.iter().filter_map(|e| {
            if e.info.enable {
                Some(e.entry.as_str())
            } else {
                None
            }
        })
    }

    /// Verify a totp challenge. The `value` should be the totp digits as plain text.
    fn verify_totp(&self, value: &str) -> Result<(), Error> {
        let now = std::time::SystemTime::now();

        for entry in self.enabled_totp_entries() {
            if entry.verify(value, now, -1..=1)?.is_some() {
                return Ok(());
            }
        }

        bail!("totp verification failed");
    }

    /// Generate a generic TFA challenge. See the [`TfaChallenge`] description for details.
    pub fn challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        webauthn: Option<Webauthn<WebauthnConfig>>,
        u2f: Option<&u2f::U2f>,
    ) -> Result<Option<TfaChallenge>, Error> {
        if self.is_empty() {
            return Ok(None);
        }

        Ok(Some(TfaChallenge {
            totp: self.totp.iter().any(|e| e.info.enable),
            recovery: RecoveryState::from(&self.recovery),
            webauthn: match webauthn {
                Some(webauthn) => self.webauthn_challenge(access.clone(), userid, webauthn)?,
                None => None,
            },
            u2f: match u2f {
                Some(u2f) => self.u2f_challenge(access.clone(), userid, u2f)?,
                None => None,
            },
            yubico: self.yubico.iter().any(|e| e.info.enable),
        }))
    }

    /// Get the recovery state.
    pub fn recovery_state(&self) -> RecoveryState {
        RecoveryState::from(&self.recovery)
    }

    /// Generate an optional webauthn challenge.
    fn webauthn_challenge<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        webauthn: Webauthn<WebauthnConfig>,
    ) -> Result<Option<webauthn_rs::proto::RequestChallengeResponse>, Error> {
        if self.webauthn.is_empty() {
            return Ok(None);
        }

        let creds: Vec<_> = self
            .enabled_webauthn_entries()
            .map(|cred| cred.clone().into())
            .collect();

        if creds.is_empty() {
            return Ok(None);
        }

        let (challenge, state) = webauthn.generate_challenge_authenticate(creds)?;

        let challenge_string = challenge.public_key.challenge.to_string();
        let mut data = access.open(userid)?;
        data.get_mut()
            .webauthn_auths
            .push(WebauthnAuthChallenge::new(state, challenge_string));
        data.save()?;

        Ok(Some(challenge))
    }

    /// Generate an optional u2f challenge.
    fn u2f_challenge<A: OpenUserChallengeData>(
        &self,
        access: A,
        userid: &str,
        u2f: &u2f::U2f,
    ) -> Result<Option<U2fChallenge>, Error> {
        if self.u2f.is_empty() {
            return Ok(None);
        }

        let keys: Vec<crate::u2f::RegisteredKey> = self
            .enabled_u2f_entries()
            .map(|registration| registration.key.clone())
            .collect();

        if keys.is_empty() {
            return Ok(None);
        }

        let challenge = U2fChallenge {
            challenge: u2f.auth_challenge()?,
            keys,
        };

        let mut data = access.open(userid)?;
        data.get_mut()
            .u2f_auths
            .push(U2fChallengeEntry::new(&challenge));
        data.save()?;

        Ok(Some(challenge))
    }

    /// Verify a u2f response.
    fn verify_u2f<A: OpenUserChallengeData>(
        &self,
        access: A,
        userid: &str,
        u2f: u2f::U2f,
        challenge: &crate::u2f::AuthChallenge,
        response: Value,
    ) -> Result<(), Error> {
        let expire_before = proxmox_time::epoch_i64() - CHALLENGE_TIMEOUT_SECS;

        let response: crate::u2f::AuthResponse = serde_json::from_value(response)
            .map_err(|err| format_err!("invalid u2f response: {}", err))?;

        if let Some(entry) = self
            .enabled_u2f_entries()
            .find(|e| e.key.key_handle == response.key_handle())
        {
            if u2f
                .auth_verify_obj(&entry.public_key, &challenge.challenge, response)?
                .is_some()
            {
                let mut data = match access.open_no_create(userid)? {
                    Some(data) => data,
                    None => bail!("no such challenge"),
                };
                let index = data
                    .get_mut()
                    .u2f_auths
                    .iter()
                    .position(|r| r == challenge)
                    .ok_or_else(|| format_err!("no such challenge"))?;
                let entry = data.get_mut().u2f_auths.remove(index);
                if entry.is_expired(expire_before) {
                    bail!("no such challenge");
                }
                data.save()
                    .map_err(|err| format_err!("failed to save challenge file: {}", err))?;

                return Ok(());
            }
        }

        bail!("u2f verification failed");
    }

    /// Verify a webauthn response.
    fn verify_webauthn<A: OpenUserChallengeData>(
        &mut self,
        access: A,
        userid: &str,
        webauthn: Webauthn<WebauthnConfig>,
        mut response: Value,
    ) -> Result<(), Error> {
        let expire_before = proxmox_time::epoch_i64() - CHALLENGE_TIMEOUT_SECS;

        let challenge = match response
            .as_object_mut()
            .ok_or_else(|| format_err!("invalid response, must be a json object"))?
            .remove("challenge")
            .ok_or_else(|| format_err!("missing challenge data in response"))?
        {
            Value::String(s) => s,
            _ => bail!("invalid challenge data in response"),
        };

        let response: webauthn_rs::proto::PublicKeyCredential = serde_json::from_value(response)
            .map_err(|err| format_err!("invalid webauthn response: {}", err))?;

        let mut data = match access.open_no_create(userid)? {
            Some(data) => data,
            None => bail!("no such challenge"),
        };

        let index = data
            .get_mut()
            .webauthn_auths
            .iter()
            .position(|r| r.challenge == challenge)
            .ok_or_else(|| format_err!("no such challenge"))?;

        let challenge = data.get_mut().webauthn_auths.remove(index);
        if challenge.is_expired(expire_before) {
            bail!("no such challenge");
        }

        // we don't allow re-trying the challenge, so make the removal persistent now:
        data.save()
            .map_err(|err| format_err!("failed to save challenge file: {}", err))?;

        webauthn.authenticate_credential(&response, &challenge.state)?;

        Ok(())
    }

    /// Verify a recovery key.
    ///
    /// NOTE: If successful, the key will automatically be removed from the list of available
    /// recovery keys, so the configuration needs to be saved afterwards!
    fn verify_recovery(&mut self, value: &str) -> Result<(), Error> {
        if let Some(r) = &mut self.recovery {
            if r.verify(value)? {
                return Ok(());
            }
        }
        bail!("recovery verification failed");
    }
}

/// A TFA entry for a user.
///
/// This simply connects a raw registration to a non optional descriptive text chosen by the user.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TfaEntry<T> {
    #[serde(flatten)]
    pub info: TfaInfo,

    /// The actual entry.
    pub entry: T,
}

impl<T> TfaEntry<T> {
    /// Create an entry with a description. The id will be autogenerated.
    fn new(description: String, entry: T) -> Self {
        Self {
            info: TfaInfo {
                id: Uuid::generate().to_string(),
                enable: true,
                description,
                created: proxmox_time::epoch_i64(),
            },
            entry,
        }
    }

    /// Create a raw entry from a `TfaInfo` and the corresponding entry data.
    pub fn from_parts(info: TfaInfo, entry: T) -> Self {
        Self { info, entry }
    }
}

#[cfg_attr(feature = "api-types", api)]
/// Over the API we only provide this part when querying a user's second factor list.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TfaInfo {
    /// The id used to reference this entry.
    pub id: String,

    /// User chosen description for this entry.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,

    /// Creation time of this entry as unix epoch.
    pub created: i64,

    /// Whether this TFA entry is currently enabled.
    #[serde(skip_serializing_if = "is_default_tfa_enable")]
    #[serde(default = "default_tfa_enable")]
    pub enable: bool,
}

impl TfaInfo {
    /// For recovery keys we have a fixed entry.
    pub fn recovery(created: i64) -> Self {
        Self {
            id: "recovery".to_string(),
            description: String::new(),
            enable: true,
            created,
        }
    }
}

const fn default_tfa_enable() -> bool {
    true
}

const fn is_default_tfa_enable(v: &bool) -> bool {
    *v
}

/// When sending a TFA challenge to the user, we include information about what kind of challenge
/// the user may perform. If webauthn credentials are available, a webauthn challenge will be
/// included.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TfaChallenge {
    /// True if the user has TOTP devices.
    #[serde(skip_serializing_if = "bool_is_false", default)]
    totp: bool,

    /// Whether there are recovery keys available.
    #[serde(skip_serializing_if = "RecoveryState::is_unavailable", default)]
    recovery: RecoveryState,

    /// If the user has any u2f tokens registered, this will contain the U2F challenge data.
    #[serde(skip_serializing_if = "Option::is_none")]
    u2f: Option<U2fChallenge>,

    /// If the user has any webauthn credentials registered, this will contain the corresponding
    /// challenge data.
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    webauthn: Option<webauthn_rs::proto::RequestChallengeResponse>,

    /// True if the user has yubico keys configured.
    #[serde(skip_serializing_if = "bool_is_false", default)]
    yubico: bool,
}

fn bool_is_false(v: &bool) -> bool {
    !v
}

/// A user's response to a TFA challenge.
pub enum TfaResponse {
    Totp(String),
    U2f(Value),
    Webauthn(Value),
    Recovery(String),
}

/// This is part of the REST API:
impl std::str::FromStr for TfaResponse {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(if let Some(totp) = s.strip_prefix("totp:") {
            TfaResponse::Totp(totp.to_string())
        } else if let Some(u2f) = s.strip_prefix("u2f:") {
            TfaResponse::U2f(serde_json::from_str(u2f)?)
        } else if let Some(webauthn) = s.strip_prefix("webauthn:") {
            TfaResponse::Webauthn(serde_json::from_str(webauthn)?)
        } else if let Some(recovery) = s.strip_prefix("recovery:") {
            TfaResponse::Recovery(recovery.to_string())
        } else {
            bail!("invalid tfa response");
        })
    }
}

/// Active TFA challenges per user, stored in a restricted temporary file on the machine handling
/// the current user's authentication.
#[derive(Default, Deserialize, Serialize)]
pub struct TfaUserChallenges {
    /// Active u2f registration challenges for a user.
    ///
    /// Expired values are automatically filtered out while parsing the tfa configuration file.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[serde(deserialize_with = "filter_expired_challenge")]
    u2f_registrations: Vec<U2fRegistrationChallenge>,

    /// Active u2f authentication challenges for a user.
    ///
    /// Expired values are automatically filtered out while parsing the tfa configuration file.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[serde(deserialize_with = "filter_expired_challenge")]
    u2f_auths: Vec<U2fChallengeEntry>,

    /// Active webauthn registration challenges for a user.
    ///
    /// Expired values are automatically filtered out while parsing the tfa configuration file.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[serde(deserialize_with = "filter_expired_challenge")]
    webauthn_registrations: Vec<WebauthnRegistrationChallenge>,

    /// Active webauthn authentication challenges for a user.
    ///
    /// Expired values are automatically filtered out while parsing the tfa configuration file.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[serde(deserialize_with = "filter_expired_challenge")]
    webauthn_auths: Vec<WebauthnAuthChallenge>,
}

/// Serde helper using our `FilteredVecVisitor` to filter out expired entries directly at load
/// time.
fn filter_expired_challenge<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + IsExpired,
{
    let expire_before = proxmox_time::epoch_i64() - CHALLENGE_TIMEOUT_SECS;
    deserializer.deserialize_seq(serde_tools::fold(
        "a challenge entry",
        |cap| cap.map(Vec::with_capacity).unwrap_or_else(Vec::new),
        move |out, reg: T| {
            if !reg.is_expired(expire_before) {
                out.push(reg);
            }
        },
    ))
}

impl TfaUserChallenges {
    /// Finish a u2f registration. The challenge should correspond to an output of
    /// `u2f_registration_challenge` (which is a stringified `RegistrationChallenge`). The response
    /// should come directly from the client.
    fn u2f_registration_finish(
        &mut self,
        u2f: &u2f::U2f,
        challenge: &str,
        response: &str,
    ) -> Result<TfaEntry<u2f::Registration>, Error> {
        let expire_before = proxmox_time::epoch_i64() - CHALLENGE_TIMEOUT_SECS;

        let index = self
            .u2f_registrations
            .iter()
            .position(|r| r.challenge == challenge)
            .ok_or_else(|| format_err!("no such challenge"))?;

        let reg = &self.u2f_registrations[index];
        if reg.is_expired(expire_before) {
            bail!("no such challenge");
        }

        // the verify call only takes the actual challenge string, so we have to extract it
        // (u2f::RegistrationChallenge did not always implement Deserialize...)
        let chobj: Value = serde_json::from_str(challenge)
            .map_err(|err| format_err!("error parsing original registration challenge: {}", err))?;
        let challenge = chobj["challenge"]
            .as_str()
            .ok_or_else(|| format_err!("invalid registration challenge"))?;

        let (mut reg, description) = match u2f.registration_verify(challenge, response)? {
            None => bail!("verification failed"),
            Some(reg) => {
                let entry = self.u2f_registrations.remove(index);
                (reg, entry.description)
            }
        };

        // we do not care about the attestation certificates, so don't store them
        reg.certificate.clear();

        Ok(TfaEntry::new(description, reg))
    }

    /// Finish a webauthn registration. The challenge should correspond to an output of
    /// `webauthn_registration_challenge`. The response should come directly from the client.
    fn webauthn_registration_finish(
        &mut self,
        webauthn: Webauthn<WebauthnConfig>,
        challenge: &str,
        response: webauthn_rs::proto::RegisterPublicKeyCredential,
        existing_registrations: &[TfaEntry<WebauthnCredential>],
    ) -> Result<TfaEntry<WebauthnCredential>, Error> {
        let expire_before = proxmox_time::epoch_i64() - CHALLENGE_TIMEOUT_SECS;

        let index = self
            .webauthn_registrations
            .iter()
            .position(|r| r.challenge == challenge)
            .ok_or_else(|| format_err!("no such challenge"))?;

        let reg = self.webauthn_registrations.remove(index);
        if reg.is_expired(expire_before) {
            bail!("no such challenge");
        }

        let (credential, _authenticator) =
            webauthn.register_credential(&response, &reg.state, |id| -> Result<bool, ()> {
                Ok(existing_registrations
                    .iter()
                    .any(|cred| cred.entry.cred_id == *id))
            })?;

        Ok(TfaEntry::new(reg.description, credential.into()))
    }
}
