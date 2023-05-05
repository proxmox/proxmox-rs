//! TFA configuration and user data.
//!
//! This is the same as used in PBS but without the `#[api]` type.
//!
//! We may want to move this into a shared crate making the `#[api]` macro feature-gated!

use std::collections::HashMap;
use std::fmt;

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use webauthn_rs::{proto::UserVerificationPolicy, Webauthn};

use crate::totp::Totp;
use proxmox_uuid::Uuid;

mod serde_tools;

mod recovery;
mod u2f;
mod webauthn;

pub mod methods;

pub use recovery::RecoveryState;
pub use u2f::U2fConfig;
use webauthn::WebauthnConfigInstance;
pub use webauthn::{WebauthnConfig, WebauthnCredential};

#[cfg(feature = "api-types")]
pub use webauthn::WebauthnConfigUpdater;

pub use crate::types::TfaInfo;

use recovery::Recovery;
use u2f::{U2fChallenge, U2fChallengeEntry, U2fRegistrationChallenge};
use webauthn::{WebauthnAuthChallenge, WebauthnRegistrationChallenge};

trait IsExpired {
    fn is_expired(&self, at_epoch: i64) -> bool;
}

pub trait OpenUserChallengeData {
    fn open(&self, userid: &str) -> Result<Box<dyn UserChallengeAccess>, Error>;

    fn open_no_create(&self, userid: &str) -> Result<Option<Box<dyn UserChallengeAccess>>, Error>;

    /// Should return `true` if something was removed, `false` if no data existed for the user.
    fn remove(&self, userid: &str) -> Result<bool, Error>;

    /// This allows overriding the number of TOTP failures allowed before locking a user out of
    /// TOTP.
    fn totp_failure_limit(&self) -> u32 {
        8
    }

    /// This allows overriding the number of consecutive TFA failures before an account gets rate
    /// limited.
    fn tfa_failure_limit(&self) -> u32 {
        100
    }

    /// This allows overriding the time users are locked out when reaching the tfa failure limit.
    fn tfa_failure_lock_time(&self) -> i64 {
        3600 * 12
    }

    /// Since PVE needs cluster-wide package upgrades for new entries in [`TfaUserData`], TOTP code
    /// reuse checks can be configured here.
    fn enable_lockout(&self) -> bool {
        true
    }
}

#[test]
fn ensure_open_user_challenge_data_is_dyn_safe() {
    let _: Option<&dyn OpenUserChallengeData> = None;
}

pub trait UserChallengeAccess {
    fn get_mut(&mut self) -> &mut TfaUserChallenges;
    fn save(&mut self) -> Result<(), Error>;
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
fn get_webauthn<'a, 'config: 'a, 'origin: 'a>(
    waconfig: &'config Option<WebauthnConfig>,
    origin: Option<&'origin Url>,
) -> Option<Webauthn<WebauthnConfigInstance<'a>>> {
    match waconfig.as_ref()?.instantiate(origin) {
        Ok(wa) => Some(Webauthn::new(wa)),
        Err(err) => {
            log::error!("webauthn error: {err}");
            None
        }
    }
}

/// Helper to get a `WebauthnConfigInstance` from a `WebauthnConfig`
///
/// This is outside of `TfaConfig` to not borrow its `&self`.
fn check_webauthn<'a, 'config: 'a, 'origin: 'a>(
    waconfig: &'config Option<WebauthnConfig>,
    origin: Option<&'origin Url>,
) -> Result<Webauthn<WebauthnConfigInstance<'a>>, Error> {
    get_webauthn(waconfig, origin).ok_or_else(|| format_err!("no webauthn configuration available"))
}

impl TfaConfig {
    /// Unlock a user's 2nd factor authentication (including TOTP).
    pub fn unlock_tfa(&mut self, userid: &str) -> Result<(), Error> {
        match self.users.get_mut(userid) {
            Some(user) => {
                user.totp_locked = false;
                user.tfa_locked_until = None;
                Ok(())
            }
            None => bail!("no such challenge"),
        }
    }

    /// Unlock a user's TOTP challenges.
    pub fn unlock_totp(&mut self, userid: &str) -> Result<(), Error> {
        match self.users.get_mut(userid) {
            Some(user) => {
                user.totp_locked = false;
                Ok(())
            }
            None => bail!("no such challenge"),
        }
    }

    /// Get a u2f registration challenge.
    pub fn u2f_registration_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
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
    pub fn u2f_registration_finish<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
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
    pub fn webauthn_registration_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        user: &str,
        description: String,
        origin: Option<&Url>,
    ) -> Result<String, Error> {
        let webauthn = check_webauthn(&self.webauthn, origin)?;

        self.users
            .entry(user.to_owned())
            .or_default()
            .webauthn_registration_challenge(access, webauthn, user, description)
    }

    /// Finish a webauthn registration challenge.
    pub fn webauthn_registration_finish<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        challenge: &str,
        response: &str,
        origin: Option<&Url>,
    ) -> Result<String, Error> {
        let webauthn = check_webauthn(&self.webauthn, origin)?;

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
    pub fn authentication_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        origin: Option<&Url>,
    ) -> Result<Option<TfaChallenge>, Error> {
        match self.users.get_mut(userid) {
            Some(udata) => udata.challenge(
                access,
                userid,
                get_webauthn(&self.webauthn, origin),
                get_u2f(&self.u2f).as_ref(),
            ),
            None => Ok(None),
        }
    }

    /// Verify a TFA challenge.
    pub fn verify<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        challenge: &TfaChallenge,
        response: TfaResponse,
        origin: Option<&Url>,
    ) -> TfaResult {
        let user = match self.users.get_mut(userid) {
            Some(user) => user,
            None => {
                // This should not be reachable, as an API should not try to verify a 2nd factor
                // of a user that doesn't have any 2nd factors.
                log::error!("no 2nd factor available for user '{userid}'");
                return TfaResult::failure(false);
            }
        };

        if user.tfa_is_locked() {
            log::error!("refusing 2nd factor for user '{userid}'");
            return TfaResult::Locked;
        }

        let mut was_totp = false;
        let result = match response {
            TfaResponse::Totp(value) => {
                was_totp = true;
                if user.totp_locked {
                    log::error!("TOTP of user '{userid}' is locked");
                    return TfaResult::Locked;
                }
                user.verify_totp(access, userid, &value)
                    .map(|needs_saving| TfaResult::Success { needs_saving })
            }
            TfaResponse::U2f(value) => match &challenge.u2f {
                Some(challenge) => user
                    .verify_u2f(access, userid, &self.u2f, &challenge.challenge, value)
                    .map(|()| TfaResult::Success {
                        needs_saving: false,
                    }),
                None => Err(format_err!("no u2f factor available for user '{}'", userid)),
            },
            TfaResponse::Webauthn(value) => user
                .verify_webauthn(access, userid, &self.webauthn, origin, value)
                .map(|()| TfaResult::Success {
                    needs_saving: false,
                }),
            TfaResponse::Recovery(value) => {
                // recovery keys get used up so they always persist data:
                user.verify_recovery(access, userid, &value)
                    .map(|()| TfaResult::Success { needs_saving: true })
            }
        };

        match result {
            Ok(r @ TfaResult::Success { .. }) => {
                // reset tfa failure count on success:
                let mut data = match access.open(userid) {
                    Ok(data) => data,
                    Err(err) => {
                        log::error!("failed to access user challenge data for '{userid}': {err}");
                        return r;
                    }
                };

                let access = data.get_mut();
                let mut save = false;
                if was_totp && access.totp_failures != 0 {
                    access.totp_failures = 0;
                    save = true;
                }

                if access.tfa_failures != 0 {
                    access.tfa_failures = 0;
                    save = true;
                }

                if save {
                    if let Err(err) = data.save() {
                        log::error!("failed to store user challenge data: {err}");
                    }
                }
                r
            }
            Ok(r) => r,
            Err(err) => {
                log::error!("error in 2nd factor authentication for user '{userid}': {err}");
                let mut data = match access.open(userid) {
                    Ok(data) => data,
                    Err(err) => {
                        log::error!("failed to access user challenge data for '{userid}': {err}");
                        return TfaResult::failure(false);
                    }
                };

                let data_mut = data.get_mut();
                data_mut.tfa_failures += 1;
                // totp failures are counted in `verify_totp`

                let tfa_limit_reached = data_mut.tfa_failures >= access.tfa_failure_limit();
                let totp_limit_reached =
                    was_totp && data_mut.totp_failures >= access.totp_failure_limit();

                if !tfa_limit_reached && !totp_limit_reached {
                    if let Err(err) = data.save() {
                        log::error!("failed to store user challenge data: {err}");
                    }
                    return TfaResult::failure(false);
                }

                if let Err(err) = data.save() {
                    log::error!("failed to store user challenge data: {err}");
                }
                drop(data);

                if totp_limit_reached {
                    user.totp_locked = access.enable_lockout();
                }

                if tfa_limit_reached && access.enable_lockout() {
                    user.tfa_locked_until =
                        Some(proxmox_time::epoch_i64() + access.tfa_failure_lock_time());
                }

                return TfaResult::Failure {
                    needs_saving: true,
                    tfa_limit_reached,
                    totp_limit_reached,
                };
            }
        }
    }

    pub fn remove_user<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
    ) -> Result<NeedsSaving, Error> {
        let mut save = access.remove(userid)?;
        if self.users.remove(userid).is_some() {
            save = true;
        }
        Ok(if save {
            NeedsSaving::Yes
        } else {
            NeedsSaving::No
        })
    }
}

#[must_use = "must save the config in order to ensure one-time use of recovery keys"]
#[derive(Debug)]
pub enum TfaResult {
    /// Login succeeded. The user file might need updating.
    Success { needs_saving: bool },
    /// Login failed. The user file might need updating.
    Failure {
        needs_saving: bool,
        totp_limit_reached: bool,
        tfa_limit_reached: bool,
    },
    /// The current method is blocked.
    Locked,
}

impl TfaResult {
    const fn failure(needs_saving: bool) -> Self {
        Self::Failure {
            needs_saving,
            totp_limit_reached: false,
            tfa_limit_reached: false,
        }
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

/// Mapping of userid to TFA entry.
pub type TfaUsers = HashMap<String, TfaUserData>;

/// TFA data for a user.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct TfaUserData {
    /// Totp keys for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub totp: Vec<TfaEntry<TotpEntry>>,

    /// Registered u2f tokens for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub u2f: Vec<TfaEntry<u2f::Registration>>,

    /// Registered webauthn tokens for a user.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub webauthn: Vec<TfaEntry<WebauthnCredential>>,

    /// Recovery keys. (Unordered OTP values).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub recovery: Option<Recovery>,

    /// Yubico keys for a user. NOTE: This is not directly supported currently, we just need this
    /// available for PVE, where the yubico API server configuration is part if the realm.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub yubico: Vec<TfaEntry<String>>,

    /// Once a user runs into a TOTP limit they get locked out of TOTP until they successfully use
    /// a recovery key.
    #[serde(skip_serializing_if = "bool_is_false", default)]
    pub totp_locked: bool,

    /// If a user hits too many 2nd factor failures, they get completely blocked for a while.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(deserialize_with = "filter_expired_timestamp")]
    pub tfa_locked_until: Option<i64>,
}

/// Serde helper to filter out an optional timestamp that should be removed.
fn filter_expired_timestamp<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<i64>::deserialize(deserializer)? {
        Some(t) if t < proxmox_time::epoch_i64() => Ok(None),
        other => Ok(other),
    }
}

impl TfaUserData {
    /// `true` if no second factors exist
    pub fn is_empty(&self) -> bool {
        self.totp.is_empty()
            && self.u2f.is_empty()
            && self.webauthn.is_empty()
            && self.yubico.is_empty()
            && self.recovery.is_none()
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
    fn u2f_registration_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
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

    fn u2f_registration_finish<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
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
    fn webauthn_registration_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        webauthn: Webauthn<WebauthnConfigInstance>,
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
    fn webauthn_registration_finish<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        webauthn: Webauthn<WebauthnConfigInstance>,
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
        let entry = TfaEntry::new(description, TotpEntry::new(totp));
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
    /// Here we also need access to the ID.
    fn enabled_totp_entries_mut(&mut self) -> impl Iterator<Item = &mut TfaEntry<TotpEntry>> {
        self.totp.iter_mut().filter(|e| e.info.enable)
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
    ///
    /// TOTP keys are stored in the user data, so we always need to save afterwards.
    fn verify_totp<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        value: &str,
    ) -> Result<bool, Error> {
        let now = std::time::SystemTime::now();

        let needs_saving = access.enable_lockout();
        for entry in self.enabled_totp_entries_mut() {
            if let Some(current) = entry.entry.verify(value, now, -1..=1)? {
                if needs_saving {
                    if current <= entry.entry.last_count {
                        let mut data = access.open(userid)?;
                        let data_access = data.get_mut();
                        data_access.totp_failures += 1;
                        data.save()?;
                        bail!("rejecting reused TOTP value");
                    }

                    entry.entry.last_count = current;
                }

                let mut data = access.open(userid)?;
                let data_access = data.get_mut();
                data_access.totp_failures = 0;
                data.save()?;
                return Ok(needs_saving);
            }
        }

        let mut data = access.open(userid)?;
        let data_access = data.get_mut();
        data_access.totp_failures += 1;
        data.save()?;

        bail!("totp verification failed");
    }

    /// Generate a generic TFA challenge. See the [`TfaChallenge`] description for details.
    fn challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        webauthn: Option<Webauthn<WebauthnConfigInstance>>,
        u2f: Option<&u2f::U2f>,
    ) -> Result<Option<TfaChallenge>, Error> {
        if self.is_empty() {
            return Ok(None);
        }

        // Since we don't bail out when failing to generate WA or U2F challenges, we keep track of
        // whether we tried here, otherwise `challenge.check()` would consider these to be not
        // configured by the user and might allow logging in without them on error.
        let mut not_empty = false;

        let challenge = TfaChallenge {
            totp: self.totp.iter().any(|e| e.info.enable),
            recovery: self.recovery_state(),
            webauthn: match webauthn {
                Some(webauthn) => match self.webauthn_challenge(access, userid, webauthn) {
                    Ok(wa) => wa,
                    Err(err) => {
                        not_empty = true;
                        log::error!("failed to generate webauthn challenge: {err}");
                        None
                    }
                },
                None => None,
            },
            u2f: match u2f {
                Some(u2f) => match self.u2f_challenge(access, userid, u2f) {
                    Ok(u2f) => u2f,
                    Err(err) => {
                        not_empty = true;
                        log::error!("failed to generate u2f challenge: {err}");
                        None
                    }
                },
                None => None,
            },
            yubico: self.yubico.iter().any(|e| e.info.enable),
        };

        // This happens if 2nd factors exist but are all disabled.
        if challenge.is_empty() && !not_empty {
            return Ok(None);
        }

        Ok(Some(challenge))
    }

    /// Get the recovery state.
    pub fn recovery_state(&self) -> Option<RecoveryState> {
        self.recovery.as_ref().map(RecoveryState::from)
    }

    /// Generate an optional webauthn challenge.
    fn webauthn_challenge<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        webauthn: Webauthn<WebauthnConfigInstance>,
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
    fn u2f_challenge<A: ?Sized + OpenUserChallengeData>(
        &self,
        access: &A,
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
    fn verify_u2f<A: ?Sized + OpenUserChallengeData>(
        &self,
        access: &A,
        userid: &str,
        u2f: &Option<U2fConfig>,
        challenge: &crate::u2f::AuthChallenge,
        response: Value,
    ) -> Result<(), Error> {
        let u2f = check_u2f(u2f)?;

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
    fn verify_webauthn<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        webauthn: &Option<WebauthnConfig>,
        origin: Option<&Url>,
        mut response: Value,
    ) -> Result<(), Error> {
        let webauthn = check_webauthn(webauthn, origin)?;

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
    fn verify_recovery<A: ?Sized + OpenUserChallengeData>(
        &mut self,
        access: &A,
        userid: &str,
        value: &str,
    ) -> Result<(), Error> {
        if let Some(r) = &mut self.recovery {
            if r.verify(value)? {
                // On success we reset the failure state.
                self.totp_locked = false;
                self.tfa_locked_until = None;

                let mut data = access.open(userid)?;
                let access = data.get_mut();
                if access.totp_failures != 0 {
                    access.totp_failures = 0;
                    data.save()?;
                }
                return Ok(());
            }
        }
        bail!("recovery verification failed");
    }

    fn tfa_is_locked(&self) -> bool {
        match self.tfa_locked_until {
            Some(locked_until) => proxmox_time::epoch_i64() < locked_until,
            None => false,
        }
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

#[derive(Clone)]
pub struct TotpEntry {
    pub totp: Totp,
    pub last_count: i64,
}

impl TotpEntry {
    pub fn new(totp: Totp) -> Self {
        Self {
            totp,
            last_count: i64::MIN,
        }
    }
}

impl std::ops::Deref for TotpEntry {
    type Target = Totp;

    fn deref(&self) -> &Totp {
        &self.totp
    }
}

impl Serialize for TotpEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        if self.last_count == i64::MIN {
            return self.totp.serialize(serializer);
        }

        let mut map = serializer.serialize_struct("TotpEntry", 2)?;
        map.serialize_field("totp", &self.totp)?;
        map.serialize_field("last-count", &self.last_count)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for TotpEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = TotpEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a totp string or a TotpEntry struct")
            }

            fn visit_str<E: Error>(self, s: &str) -> Result<TotpEntry, E> {
                Ok(TotpEntry::new(s.parse().map_err(|err| E::custom(err))?))
            }

            fn visit_map<A>(self, mut map: A) -> Result<TotpEntry, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                use std::borrow::Cow;

                let mut totp = None;
                let mut last_count = None;

                loop {
                    let key: Cow<'de, str> = match map.next_key()? {
                        Some(k) => k,
                        None => break,
                    };

                    match key.as_ref() {
                        "totp" if totp.is_some() => return Err(A::Error::duplicate_field("totp")),
                        "totp" => totp = Some(map.next_value()?),
                        "last-count" if last_count.is_some() => {
                            return Err(A::Error::duplicate_field("last-count"))
                        }
                        "last-count" => last_count = Some(map.next_value()?),
                        other => {
                            return Err(A::Error::unknown_field(other, &["totp", "last-count"]))
                        }
                    }
                }

                Ok(TotpEntry {
                    totp: totp.ok_or_else(|| A::Error::missing_field("totp"))?,
                    last_count: last_count.unwrap_or(i64::MIN),
                })
            }
        }

        deserializer.deserialize_any(V)
    }
}

/// When sending a TFA challenge to the user, we include information about what kind of challenge
/// the user may perform. If webauthn credentials are available, a webauthn challenge will be
/// included.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TfaChallenge {
    /// True if the user has TOTP devices.
    #[serde(skip_serializing_if = "bool_is_false", default)]
    pub totp: bool,

    /// Whether there are recovery keys available.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub recovery: Option<RecoveryState>,

    /// If the user has any u2f tokens registered, this will contain the U2F challenge data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u2f: Option<U2fChallenge>,

    /// If the user has any webauthn credentials registered, this will contain the corresponding
    /// challenge data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webauthn: Option<webauthn_rs::proto::RequestChallengeResponse>,

    /// True if the user has yubico keys configured.
    #[serde(skip_serializing_if = "bool_is_false", default)]
    pub yubico: bool,
}

impl TfaChallenge {
    pub fn is_empty(&self) -> bool {
        !self.totp
            && self.recovery.is_none()
            && self.u2f.is_none()
            && self.webauthn.is_none()
            && !self.yubico
    }
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

    /// Number of consecutive TOTP failures. Too many of those will lock out a user.
    #[serde(skip_serializing_if = "u32_is_zero", default)]
    totp_failures: u32,

    /// Number of consecutive 2nd factor failures. When the limit is reached, the user is locked
    /// out for 12 hours.
    #[serde(skip_serializing_if = "u32_is_zero", default)]
    tfa_failures: u32,
}

fn u32_is_zero(n: &u32) -> bool {
    *n == 0
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
        webauthn: Webauthn<WebauthnConfigInstance>,
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
