use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Mutex;

use anyhow::{format_err, Error};
use percent_encoding::percent_decode_str;

use proxmox_rest_server::{extract_cookie, AuthError};
use proxmox_tfa::api::{OpenUserChallengeData, TfaConfig};

use crate::auth_key::{HMACKey, Keyring};
use crate::types::{Authid, RealmRef, Userid, UsernameRef};

mod access;
mod ticket;

use crate::ticket::Ticket;
use access::verify_csrf_prevention_token;

pub use access::{assemble_csrf_prevention_token, create_ticket, API_METHOD_CREATE_TICKET};
pub use ticket::{ApiTicket, PartialTicket};

/// Authentication realms are used to manage users: authenticate, change password or remove.
pub trait Authenticator {
    /// Authenticate a user given a password.
    fn authenticate_user<'a>(
        &'a self,
        username: &'a UsernameRef,
        password: &'a str,
        client_ip: Option<&'a IpAddr>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

    /// Change a user's password.
    fn store_password(
        &self,
        username: &UsernameRef,
        password: &str,
        client_ip: Option<&IpAddr>,
    ) -> Result<(), Error>;

    /// Remove a user.
    fn remove_password(&self, username: &UsernameRef) -> Result<(), Error>;
}

/// This provides access to the available realms and authentication keys.
pub trait AuthContext: Send + Sync {
    /// Lookup a realm by name.
    fn lookup_realm(&self, realm: &RealmRef) -> Option<Box<dyn Authenticator + Send + Sync>>;

    /// Get the current authentication keyring.
    fn keyring(&self) -> &Keyring;

    /// The auth prefix without the separating colon. Eg. `"PBS"`.
    fn auth_prefix(&self) -> &'static str;

    /// API token prefix (without the `'='`).
    fn auth_token_prefix(&self) -> &'static str;

    /// Auth cookie name.
    fn auth_cookie_name(&self) -> &'static str;

    /// Access the TFA config with an exclusive lock.
    fn tfa_config_write_lock(&self) -> Result<Box<dyn LockedTfaConfig>, Error>;

    /// Check if a userid is enabled and return a [`UserInformation`] handle.
    fn auth_id_is_active(&self, auth_id: &Authid) -> Result<bool, Error>;

    /// CSRF prevention token secret data.
    fn csrf_secret(&self) -> &'static HMACKey;

    /// Verify a token secret.
    fn verify_token_secret(&self, token_id: &Authid, token_secret: &str) -> Result<(), Error>;

    /// Check path based tickets. (Used for terminal tickets).
    fn check_path_ticket(
        &self,
        userid: &Userid,
        password: &str,
        path: String,
        privs: String,
        port: u16,
    ) -> Result<Option<bool>, Error> {
        let _ = (userid, password, path, privs, port);
        Ok(None)
    }
}

/// When verifying TFA challenges we need to be able to update the TFA config without interference
/// from other threads. Similarly, to authenticate with recovery keys, we need to be able to
/// atomically mark them as used.
pub trait LockedTfaConfig {
    /// Get mutable access to the [`TfaConfig`] and retain immutable access to `self`.
    fn config_mut(&mut self) -> (&dyn OpenUserChallengeData, &mut TfaConfig);

    // Save the modified [`TfaConfig`].
    //
    // The config will have been modified by accessing the
    // [`config_mut`](LockedTfaConfig::config_mut()) method.
    fn save_config(&mut self) -> Result<(), Error>;
}

static AUTH_CONTEXT: Mutex<Option<&'static dyn AuthContext>> = Mutex::new(None);

/// Configure access to authentication realms and keys.
pub fn set_auth_context(auth_context: &'static dyn AuthContext) {
    *AUTH_CONTEXT.lock().unwrap() = Some(auth_context);
}

fn auth_context() -> Result<&'static dyn AuthContext, Error> {
    (*AUTH_CONTEXT.lock().unwrap()).ok_or_else(|| format_err!("no realm access configured"))
}

struct UserAuthData {
    ticket: String,
    csrf_token: Option<String>,
}

enum AuthData {
    User(UserAuthData),
    ApiToken(String),
}

pub fn http_check_auth(
    headers: &http::HeaderMap,
    method: &http::Method,
) -> Result<String, AuthError> {
    let auth_context = auth_context()?;

    let auth_data = extract_auth_data(auth_context, headers);
    match auth_data {
        Some(AuthData::User(user_auth_data)) => {
            let ticket = user_auth_data.ticket.clone();
            let ticket_lifetime = crate::TICKET_LIFETIME;

            let userid: Userid = Ticket::<ApiTicket>::parse(&ticket)?
                .verify_with_time_frame(
                    auth_context.keyring(),
                    auth_context.auth_prefix(),
                    None,
                    -300..ticket_lifetime,
                )?
                .require_full()?;

            let auth_id = Authid::from(userid.clone());
            if !auth_context.auth_id_is_active(&auth_id)? {
                return Err(format_err!("user account disabled or expired.").into());
            }

            if method != http::Method::GET {
                if let Some(csrf_token) = &user_auth_data.csrf_token {
                    verify_csrf_prevention_token(
                        auth_context.csrf_secret(),
                        &userid,
                        csrf_token,
                        -300,
                        ticket_lifetime,
                    )?;
                } else {
                    return Err(format_err!("missing CSRF prevention token").into());
                }
            }

            Ok(auth_id.to_string())
        }
        Some(AuthData::ApiToken(api_token)) => {
            let mut parts = api_token.splitn(2, ':');
            let tokenid = parts
                .next()
                .ok_or_else(|| format_err!("failed to split API token header"))?;
            let tokenid: Authid = tokenid.parse()?;

            if !auth_context.auth_id_is_active(&tokenid)? {
                return Err(format_err!("user account or token disabled or expired.").into());
            }

            let tokensecret = parts
                .next()
                .ok_or_else(|| format_err!("failed to split API token header"))?;
            let tokensecret = percent_decode_str(tokensecret)
                .decode_utf8()
                .map_err(|_| format_err!("failed to decode API token header"))?;

            auth_context.verify_token_secret(&tokenid, &tokensecret)?;

            Ok(tokenid.to_string())
        }
        None => Err(AuthError::NoData),
    }
}

fn extract_auth_data(
    auth_context: &dyn AuthContext,
    headers: &http::HeaderMap,
) -> Option<AuthData> {
    if let Some(raw_cookie) = headers.get(http::header::COOKIE) {
        if let Ok(cookie) = raw_cookie.to_str() {
            if let Some(ticket) = extract_cookie(cookie, auth_context.auth_cookie_name()) {
                let csrf_token = match headers.get("CSRFPreventionToken").map(|v| v.to_str()) {
                    Some(Ok(v)) => Some(v.to_owned()),
                    _ => None,
                };
                return Some(AuthData::User(UserAuthData { ticket, csrf_token }));
            }
        }
    }

    let token_prefix = auth_context.auth_token_prefix();
    match headers.get(http::header::AUTHORIZATION).map(|v| v.to_str()) {
        Some(Ok(v)) => {
            if !v.starts_with(token_prefix) {
                return None;
            }
            match v.as_bytes().get(token_prefix.len()).copied() {
                Some(b' ') | Some(b'=') => {
                    Some(AuthData::ApiToken(v[(token_prefix.len() + 1)..].to_owned()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}
