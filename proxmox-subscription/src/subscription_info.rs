use std::path::Path;

use anyhow::{bail, format_err, Error};
use openssl::hash::{hash, DigestBytes, MessageDigest};
use proxmox_sys::fs::file_get_contents;
use proxmox_time::TmEditor;
use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::{api, Updater};

use crate::sign::Verifier;

pub(crate) const SHARED_KEY_DATA: &str = "kjfdlskfhiuewhfk947368";

/// How long the local key is valid for in between remote checks
pub(crate) const SUBSCRIPTION_MAX_LOCAL_KEY_AGE: i64 = 15 * 24 * 3600;
pub(crate) const SUBSCRIPTION_MAX_LOCAL_SIGNED_KEY_AGE: i64 = 365 * 24 * 3600;
pub(crate) const SUBSCRIPTION_MAX_KEY_CHECK_FAILURE_AGE: i64 = 5 * 24 * 3600;

// Aliases are needed for PVE compat!
#[cfg_attr(feature = "api-types", api())]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Subscription status
pub enum SubscriptionStatus {
    // FIXME: remove?
    /// newly set subscription, not yet checked
    #[serde(alias = "New")]
    New,
    /// no subscription set
    #[serde(alias = "NotFound")]
    NotFound,
    /// subscription set and active
    #[serde(alias = "Active")]
    Active,
    /// subscription set but invalid for this server
    #[serde(alias = "Invalid")]
    Invalid,
    /// subscription set but expired for this server
    #[serde(alias = "Expired")]
    Expired,
    /// subscription got (recently) suspended
    #[serde(alias = "Suspended")]
    Suspended,
}
impl Default for SubscriptionStatus {
    fn default() -> Self {
        SubscriptionStatus::NotFound
    }
}
impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionStatus::New => write!(f, "new"),
            SubscriptionStatus::NotFound => write!(f, "notfound"),
            SubscriptionStatus::Active => write!(f, "active"),
            SubscriptionStatus::Invalid => write!(f, "invalid"),
            SubscriptionStatus::Expired => write!(f, "expired"),
            SubscriptionStatus::Suspended => write!(f, "suspended"),
        }
    }
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        status: {
            type: SubscriptionStatus,
        },
    },
))]
#[cfg_attr(feature = "api-types", derive(Updater))]
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Proxmox subscription information
pub struct SubscriptionInfo {
    /// Subscription status from the last check
    pub status: SubscriptionStatus,
    /// the server ID, if permitted to access
    #[serde(skip_serializing_if = "Option::is_none", alias = "validdirectory")]
    pub serverid: Option<String>,
    /// timestamp of the last check done
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checktime: Option<i64>,
    /// the subscription key, if set and permitted to access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// a more human readable status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// human readable productname of the set subscription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub productname: Option<String>,
    /// register date of the set subscription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regdate: Option<String>,
    /// next due date of the set subscription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nextduedate: Option<String>,
    /// URL to the web shop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Signature for offline keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl SubscriptionInfo {
    /// Returns the canonicalized signed data and, if available, signature contained in `self`.
    pub fn signed_data(&self) -> Result<(Vec<u8>, Option<String>), Error> {
        let mut data = serde_json::to_value(&self)?;
        let signature = data
            .as_object_mut()
            .ok_or_else(|| format_err!("subscription info not a JSON object"))?
            .remove("signature")
            .and_then(|v| v.as_str().map(|v| v.to_owned()));

        if self.is_signed() && signature.is_none() {
            bail!("Failed to extract signature value!");
        }

        let data = proxmox_serde::json::to_canonical_json(&data)?;
        Ok((data, signature))
    }

    /// Whether a signature exists - *this does not check the signature's validity!*
    ///
    /// Use [SubscriptionInfo::check_signature()] to verify the
    /// signature.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Checks whether a [SubscriptionInfo]'s `checktime` matches the age criteria:
    ///
    /// - Instances generated (more than 1.5h) in the future are invalid
    /// - Signed instances are valid for up to a year, clamped by the next due date
    /// - Unsigned instances are valid for 30+5 days
    /// - If `recheck` is set to `true`, unsigned instances are only treated as valid for 5 days
    ///  (this mode is used to decide whether to refresh the subscription information)
    ///
    /// If the criteria are not met, `status` is set to [SubscriptionStatus::Invalid] and `message`
    /// to a human-readable error message.
    pub fn check_age(&mut self, recheck: bool) {
        let now = proxmox_time::epoch_i64();
        let age = now - self.checktime.unwrap_or(0);

        let cutoff = if self.is_signed() {
            SUBSCRIPTION_MAX_LOCAL_SIGNED_KEY_AGE
        } else if recheck {
            SUBSCRIPTION_MAX_KEY_CHECK_FAILURE_AGE
        } else {
            SUBSCRIPTION_MAX_LOCAL_KEY_AGE + SUBSCRIPTION_MAX_KEY_CHECK_FAILURE_AGE
        };

        // allow some delta for DST changes or time syncs, 1.5h
        if age < -5400 {
            self.status = SubscriptionStatus::Invalid;
            self.message = Some("last check date too far in the future".to_string());
            self.signature = None;
        } else if age > cutoff {
            if let SubscriptionStatus::Active = self.status {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some("subscription information too old".to_string());
                self.signature = None;
            }
        }

        if self.is_signed() && self.status == SubscriptionStatus::Active {
            if let Some(next_due) = self.nextduedate.as_ref() {
                match parse_next_due(next_due.as_str()) {
                    Ok(next_due) if now > next_due => {
                        self.status = SubscriptionStatus::Invalid;
                        self.message = Some("subscription information too old".to_string());
                        self.signature = None;
                    }
                    Ok(_) => {}
                    Err(err) => {
                        self.status = SubscriptionStatus::Invalid;
                        self.message = Some(format!("Failed parsing 'nextduedate' - {err}"));
                        self.signature = None;
                    }
                }
            }
        }
    }

    /// Check that server ID contained in [SubscriptionInfo] matches that of current system.
    ///
    /// `status` is set to [SubscriptionStatus::Invalid] and `message` to a human-readable
    ///  message in case it does not.
    pub fn check_server_id(&mut self) {
        match (self.serverid.as_ref(), get_hardware_address()) {
            (_, Err(err)) => {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some(format!("Failed to obtain server ID - {err}."));
                self.signature = None;
            }
            (None, _) => {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some("Missing server ID.".to_string());
                self.signature = None;
            }
            (Some(contained), Ok(expected)) if &expected != contained => {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some("Server ID mismatch.".to_string());
                self.signature = None;
            }
            (Some(_), Ok(_)) => {}
        }
    }

    /// Check a [SubscriptionInfo]'s signature, if one is available.
    ///
    /// `status` is set to [SubscriptionStatus::Invalid] and `message` to a human-readable error
    /// message in case a signature is available but not valid for the given `key`.
    pub fn check_signature<P: AsRef<Path>>(&mut self, keys: &[P]) {
        let verify = |info: &SubscriptionInfo, path: &P| -> Result<(), Error> {
            let raw = file_get_contents(path)?;

            let key = openssl::pkey::PKey::public_key_from_pem(&raw)?;

            let (signed, signature) = info.signed_data()?;
            let signature = match signature {
                None => bail!("Failed to extract signature value."),
                Some(sig) => sig,
            };

            key.verify(&signed, &signature)
                .map_err(|err| format_err!("Signature verification failed - {err}"))
        };

        if self.is_signed() {
            if keys.is_empty() {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some("Signature exists, but no key available.".to_string());
            } else if !keys.iter().any(|key| verify(self, key).is_ok()) {
                self.status = SubscriptionStatus::Invalid;
                self.message = Some("Signature validation failed".to_string());
            }
        }
    }
}

/// Shortcut for md5 sums.
pub(crate) fn md5sum(data: &[u8]) -> Result<DigestBytes, Error> {
    hash(MessageDigest::md5(), data).map_err(Error::from)
}

/// Generate the current system's "server ID".
pub fn get_hardware_address() -> Result<String, Error> {
    static FILENAME: &str = "/etc/ssh/ssh_host_rsa_key.pub";

    let contents = proxmox_sys::fs::file_get_contents(FILENAME)
        .map_err(|e| format_err!("Error getting host key - {}", e))?;
    let digest = md5sum(&contents).map_err(|e| format_err!("Error digesting host key - {}", e))?;

    Ok(hex::encode(&digest).to_uppercase())
}

fn parse_next_due(value: &str) -> Result<i64, Error> {
    let mut components = value.split('-');
    let year = components
        .next()
        .ok_or_else(|| format_err!("missing year component."))?
        .parse::<i32>()?;
    let month = components
        .next()
        .ok_or_else(|| format_err!("missing month component."))?
        .parse::<i32>()?;
    let day = components
        .next()
        .ok_or_else(|| format_err!("missing day component."))?
        .parse::<i32>()?;

    if components.next().is_some() {
        bail!("cannot parse 'nextduedate' value '{value}'");
    }

    let mut tm = TmEditor::new(true);
    tm.set_year(year)?;
    tm.set_mon(month)?;
    tm.set_mday(day)?;

    tm.into_epoch()
}
