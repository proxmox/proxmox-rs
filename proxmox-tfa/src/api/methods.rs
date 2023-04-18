//! API interaction module.
//!
//! This defines the methods & types used in the authentication and TFA configuration API between
//! PBS, PVE, PMG.

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::api;

use super::{OpenUserChallengeData, TfaConfig, TfaInfo, TfaUserData};
use crate::totp::Totp;

#[cfg_attr(feature = "api-types", api)]
/// A TFA entry type.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TfaType {
    /// A TOTP entry type.
    Totp,
    /// A U2F token entry.
    U2f,
    /// A Webauthn token entry.
    Webauthn,
    /// Recovery tokens.
    Recovery,
    /// Yubico authentication entry.
    Yubico,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        type: { type: TfaType },
        info: { type: TfaInfo },
    },
))]
/// A TFA entry for a user.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedTfaInfo {
    #[serde(rename = "type")]
    pub ty: TfaType,

    #[serde(flatten)]
    pub info: TfaInfo,
}

fn to_data(data: &TfaUserData) -> Vec<TypedTfaInfo> {
    let mut out = Vec::with_capacity(
        data.totp.len()
            + data.u2f.len()
            + data.webauthn.len()
            + data.yubico.len()
            + if data.recovery.is_some() { 1 } else { 0 },
    );
    if let Some(recovery) = &data.recovery {
        out.push(TypedTfaInfo {
            ty: TfaType::Recovery,
            info: TfaInfo::recovery(recovery.created),
        })
    }
    for entry in &data.totp {
        out.push(TypedTfaInfo {
            ty: TfaType::Totp,
            info: entry.info.clone(),
        });
    }
    for entry in &data.webauthn {
        out.push(TypedTfaInfo {
            ty: TfaType::Webauthn,
            info: entry.info.clone(),
        });
    }
    for entry in &data.u2f {
        out.push(TypedTfaInfo {
            ty: TfaType::U2f,
            info: entry.info.clone(),
        });
    }
    for entry in &data.yubico {
        out.push(TypedTfaInfo {
            ty: TfaType::Yubico,
            info: entry.info.clone(),
        });
    }
    out
}

/// Iterate through tuples of `(type, index, id)`.
fn tfa_id_iter(data: &TfaUserData) -> impl Iterator<Item = (TfaType, usize, &str)> {
    data.totp
        .iter()
        .enumerate()
        .map(|(i, entry)| (TfaType::Totp, i, entry.info.id.as_str()))
        .chain(
            data.webauthn
                .iter()
                .enumerate()
                .map(|(i, entry)| (TfaType::Webauthn, i, entry.info.id.as_str())),
        )
        .chain(
            data.u2f
                .iter()
                .enumerate()
                .map(|(i, entry)| (TfaType::U2f, i, entry.info.id.as_str())),
        )
        .chain(
            data.yubico
                .iter()
                .enumerate()
                .map(|(i, entry)| (TfaType::Yubico, i, entry.info.id.as_str())),
        )
        .chain(
            data.recovery
                .iter()
                .map(|_| (TfaType::Recovery, 0, "recovery")),
        )
}

/// API call implementation for `GET /access/tfa/{userid}`
///
/// Permissions for accessing `userid` must have been verified by the caller.
pub fn list_user_tfa(config: &TfaConfig, userid: &str) -> Result<Vec<TypedTfaInfo>, Error> {
    Ok(match config.users.get(userid) {
        Some(data) => to_data(data),
        None => Vec::new(),
    })
}

/// API call implementation for `GET /access/tfa/{userid}/{ID}`.
///
/// Permissions for accessing `userid` must have been verified by the caller.
///
/// In case this returns `None` a `NOT_FOUND` http error should be returned.
pub fn get_tfa_entry(config: &TfaConfig, userid: &str, id: &str) -> Option<TypedTfaInfo> {
    let user_data = match config.users.get(userid) {
        Some(u) => u,
        None => return None,
    };

    Some(
        match {
            // scope to prevent the temporary iter from borrowing across the whole match
            let entry = tfa_id_iter(user_data).find(|(_ty, _index, entry_id)| id == *entry_id);
            entry.map(|(ty, index, _)| (ty, index))
        } {
            Some((TfaType::Recovery, _)) => match &user_data.recovery {
                Some(recovery) => TypedTfaInfo {
                    ty: TfaType::Recovery,
                    info: TfaInfo::recovery(recovery.created),
                },
                None => return None,
            },
            Some((TfaType::Totp, index)) => TypedTfaInfo {
                ty: TfaType::Totp,
                info: user_data.totp.get(index).unwrap().info.clone(),
            },
            Some((TfaType::Webauthn, index)) => TypedTfaInfo {
                ty: TfaType::Webauthn,
                info: user_data.webauthn.get(index).unwrap().info.clone(),
            },
            Some((TfaType::U2f, index)) => TypedTfaInfo {
                ty: TfaType::U2f,
                info: user_data.u2f.get(index).unwrap().info.clone(),
            },
            Some((TfaType::Yubico, index)) => TypedTfaInfo {
                ty: TfaType::Yubico,
                info: user_data.yubico.get(index).unwrap().info.clone(),
            },
            None => return None,
        },
    )
}

pub struct EntryNotFound;

/// API call implementation for `DELETE /access/tfa/{userid}/{ID}`.
///
/// The caller must have already verified the user's password.
///
/// The TFA config must be WRITE locked.
///
/// The caller must *save* the config afterwards!
///
/// Errors only if the entry was not found.
///
/// Returns `true` if the user still has other TFA entries left, `false` if the user has *no* more
/// tfa entries.
pub fn delete_tfa(config: &mut TfaConfig, userid: &str, id: &str) -> Result<bool, EntryNotFound> {
    let user_data = config.users.get_mut(userid).ok_or(EntryNotFound)?;

    match {
        // scope to prevent the temporary iter from borrowing across the whole match
        let entry = tfa_id_iter(user_data).find(|(_, _, entry_id)| id == *entry_id);
        entry.map(|(ty, index, _)| (ty, index))
    } {
        Some((TfaType::Recovery, _)) => user_data.recovery = None,
        Some((TfaType::Totp, index)) => drop(user_data.totp.remove(index)),
        Some((TfaType::Webauthn, index)) => drop(user_data.webauthn.remove(index)),
        Some((TfaType::U2f, index)) => drop(user_data.u2f.remove(index)),
        Some((TfaType::Yubico, index)) => drop(user_data.yubico.remove(index)),
        None => return Err(EntryNotFound),
    }

    if user_data.is_empty() {
        config.users.remove(userid);
        Ok(false)
    } else {
        Ok(true)
    }
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        "entries": {
            type: Array,
            items: { type: TypedTfaInfo },
        },
    },
))]
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Over the API we only provide the descriptions for TFA data.
pub struct TfaUser {
    /// The user this entry belongs to.
    userid: String,

    /// TFA entries.
    entries: Vec<TypedTfaInfo>,
}

/// API call implementation for `GET /access/tfa`.
///
/// Caller needs to have performed the required privilege checks already.
pub fn list_tfa(
    config: &TfaConfig,
    authid: &str,
    top_level_allowed: bool,
) -> Result<Vec<TfaUser>, Error> {
    let tfa_data = &config.users;

    let mut out = Vec::<TfaUser>::new();
    if top_level_allowed {
        for (user, data) in tfa_data {
            out.push(TfaUser {
                userid: user.clone(),
                entries: to_data(data),
            });
        }
    } else if let Some(data) = { tfa_data }.get(authid) {
        out.push(TfaUser {
            userid: authid.into(),
            entries: to_data(data),
        });
    }

    Ok(out)
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        recovery: {
            description: "A list of recovery codes as integers.",
            type: Array,
            items: {
                type: Integer,
                description: "A one-time usable recovery code entry.",
            },
        },
    },
))]
/// The result returned when adding TFA entries to a user.
#[derive(Default, Serialize)]
pub struct TfaUpdateInfo {
    /// The id if a newly added TFA entry.
    id: Option<String>,

    /// When adding u2f entries, this contains a challenge the user must respond to in order to
    /// finish the registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<String>,

    /// When adding recovery codes, this contains the list of codes to be displayed to the user
    /// this one time.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    recovery: Vec<String>,
}

impl TfaUpdateInfo {
    fn id(id: String) -> Self {
        Self {
            id: Some(id),
            ..Default::default()
        }
    }
}

fn need_description(description: Option<String>) -> Result<String, Error> {
    description.ok_or_else(|| format_err!("'description' is required for new entries"))
}

/// API call implementation for `POST /access/tfa/{userid}`.
///
/// Permissions for accessing `userid` must have been verified by the caller.
///
/// The caller must have already verified the user's password!
#[allow(clippy::too_many_arguments)]
pub fn add_tfa_entry<A: OpenUserChallengeData>(
    config: &mut TfaConfig,
    access: &A,
    userid: &str,
    description: Option<String>,
    totp: Option<String>,
    value: Option<String>,
    challenge: Option<String>,
    r#type: TfaType,
    origin: Option<&url::Url>,
) -> Result<TfaUpdateInfo, Error> {
    match r#type {
        TfaType::Totp => {
            if challenge.is_some() {
                bail!("'challenge' parameter is invalid for 'totp' entries");
            }

            add_totp(config, userid, need_description(description)?, totp, value)
        }
        TfaType::Webauthn => {
            if totp.is_some() {
                bail!("'totp' parameter is invalid for 'webauthn' entries");
            }

            add_webauthn(
                config,
                access,
                userid,
                description,
                challenge,
                value,
                origin,
            )
        }
        TfaType::U2f => {
            if totp.is_some() {
                bail!("'totp' parameter is invalid for 'u2f' entries");
            }

            add_u2f(config, access, userid, description, challenge, value)
        }
        TfaType::Recovery => {
            if totp.or(value).or(challenge).is_some() {
                bail!("generating recovery tokens does not allow additional parameters");
            }

            let recovery = config.add_recovery(userid)?;

            Ok(TfaUpdateInfo {
                id: Some("recovery".to_string()),
                recovery,
                ..Default::default()
            })
        }
        TfaType::Yubico => {
            if totp.or(challenge).is_some() {
                bail!("'totp' and 'challenge' parameters are invalid for 'yubico' entries");
            }

            add_yubico(config, userid, need_description(description)?, value)
        }
    }
}

fn add_totp(
    config: &mut TfaConfig,
    userid: &str,
    description: String,
    totp: Option<String>,
    value: Option<String>,
) -> Result<TfaUpdateInfo, Error> {
    let (totp, value) = match (totp, value) {
        (Some(totp), Some(value)) => (totp, value),
        _ => bail!("'totp' type requires both 'totp' and 'value' parameters"),
    };

    let totp: Totp = totp.parse()?;
    if totp
        .verify(&value, std::time::SystemTime::now(), -1..=1)?
        .is_none()
    {
        bail!("failed to verify TOTP challenge");
    }
    Ok(TfaUpdateInfo::id(config.add_totp(
        userid,
        description,
        totp,
    )))
}

fn add_yubico(
    config: &mut TfaConfig,
    userid: &str,
    description: String,
    value: Option<String>,
) -> Result<TfaUpdateInfo, Error> {
    let key = value.ok_or_else(|| format_err!("missing 'value' parameter for 'yubico' entry"))?;
    Ok(TfaUpdateInfo::id(config.add_yubico(
        userid,
        description,
        key,
    )))
}

fn add_u2f<A: ?Sized + OpenUserChallengeData>(
    config: &mut TfaConfig,
    access: &A,
    userid: &str,
    description: Option<String>,
    challenge: Option<String>,
    value: Option<String>,
) -> Result<TfaUpdateInfo, Error> {
    match challenge {
        None => config
            .u2f_registration_challenge(access, userid, need_description(description)?)
            .map(|c| TfaUpdateInfo {
                challenge: Some(c),
                ..Default::default()
            }),
        Some(challenge) => {
            let value = value.ok_or_else(|| {
                format_err!("missing 'value' parameter (u2f challenge response missing)")
            })?;
            config
                .u2f_registration_finish(access, userid, &challenge, &value)
                .map(TfaUpdateInfo::id)
        }
    }
}

fn add_webauthn<A: ?Sized + OpenUserChallengeData>(
    config: &mut TfaConfig,
    access: &A,
    userid: &str,
    description: Option<String>,
    challenge: Option<String>,
    value: Option<String>,
    origin: Option<&url::Url>,
) -> Result<TfaUpdateInfo, Error> {
    match challenge {
        None => config
            .webauthn_registration_challenge(access, userid, need_description(description)?, origin)
            .map(|c| TfaUpdateInfo {
                challenge: Some(c),
                ..Default::default()
            }),
        Some(challenge) => {
            let value = value.ok_or_else(|| {
                format_err!("missing 'value' parameter (webauthn challenge response missing)")
            })?;
            config
                .webauthn_registration_finish(access, userid, &challenge, &value, origin)
                .map(TfaUpdateInfo::id)
        }
    }
}

/// API call implementation for `PUT /access/tfa/{userid}/{id}`.
///
/// The caller must have already verified the user's password.
///
/// Errors only if the entry was not found.
pub fn update_tfa_entry(
    config: &mut TfaConfig,
    userid: &str,
    id: &str,
    description: Option<String>,
    enable: Option<bool>,
) -> Result<(), EntryNotFound> {
    let mut entry = config
        .users
        .get_mut(userid)
        .and_then(|user| user.find_entry_mut(id))
        .ok_or(EntryNotFound)?;

    if let Some(description) = description {
        entry.description = description;
    }

    if let Some(enable) = enable {
        entry.enable = enable;
    }

    Ok(())
}
