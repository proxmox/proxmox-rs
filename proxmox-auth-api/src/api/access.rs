//! Provides the "/access/ticket" API call.

use anyhow::{bail, format_err, Error};
use serde_json::{json, Value};

use proxmox_router::{http_err, Permission, RpcEnvironment};
use proxmox_schema::{api, api_types::PASSWORD_SCHEMA};
use proxmox_tfa::api::TfaChallenge;

use super::auth_context;
use super::ApiTicket;
use crate::ticket::Ticket;
use crate::types::{Authid, Userid};

#[allow(clippy::large_enum_variant)]
enum AuthResult {
    /// Successful authentication which does not require a new ticket.
    Success,

    /// Successful authentication which requires a ticket to be created.
    CreateTicket,

    /// A partial ticket which requires a 2nd factor will be created.
    Partial(Box<TfaChallenge>),
}

#[api(
    input: {
        properties: {
            username: {
                type: Userid,
            },
            password: {
                schema: PASSWORD_SCHEMA,
            },
            path: {
                type: String,
                description: "Path for verifying terminal tickets.",
                optional: true,
            },
            privs: {
                type: String,
                description: "Privilege for verifying terminal tickets.",
                optional: true,
            },
            port: {
                type: Integer,
                description: "Port for verifying terminal tickets.",
                optional: true,
            },
            "tfa-challenge": {
                type: String,
                description: "The signed TFA challenge string the user wants to respond to.",
                optional: true,
            },
        },
    },
    returns: {
        properties: {
            username: {
                type: String,
                description: "User name.",
            },
            ticket: {
                type: String,
                description: "Auth ticket.",
            },
            CSRFPreventionToken: {
                type: String,
                description:
                    "Cross Site Request Forgery Prevention Token. \
                     For partial tickets this is the string \"invalid\".",
            },
        },
    },
    protected: true,
    access: {
        permission: &Permission::World,
    },
)]
/// Create or verify authentication ticket.
///
/// Returns: An authentication ticket with additional infos.
pub async fn create_ticket(
    username: Userid,
    password: String,
    path: Option<String>,
    privs: Option<String>,
    port: Option<u16>,
    tfa_challenge: Option<String>,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    use proxmox_rest_server::RestEnvironment;

    let env: &RestEnvironment = rpcenv
        .as_any()
        .downcast_ref::<RestEnvironment>()
        .ok_or_else(|| format_err!("detected wrong RpcEnvironment type"))?;

    match authenticate_user(&username, &password, path, privs, port, tfa_challenge).await {
        Ok(AuthResult::Success) => Ok(json!({ "username": username })),
        Ok(AuthResult::CreateTicket) => {
            let auth_context = auth_context()?;
            let api_ticket = ApiTicket::Full(username.clone());
            let ticket = Ticket::new(auth_context.auth_prefix(), &api_ticket)?
                .sign(auth_context.keyring(), None)?;
            let token = assemble_csrf_prevention_token(auth_context.csrf_secret(), &username);

            env.log_auth(username.as_str());

            Ok(json!({
                "username": username,
                "ticket": ticket,
                "CSRFPreventionToken": token,
            }))
        }
        Ok(AuthResult::Partial(challenge)) => {
            let auth_context = auth_context()?;
            let api_ticket = ApiTicket::Partial(challenge);
            let ticket = Ticket::new(auth_context.auth_prefix(), &api_ticket)?
                .sign(auth_context.keyring(), Some(username.as_str()))?;
            Ok(json!({
                "username": username,
                "ticket": ticket,
                "CSRFPreventionToken": "invalid",
            }))
        }
        Err(err) => {
            env.log_failed_auth(Some(username.to_string()), &err.to_string());
            Err(http_err!(UNAUTHORIZED, "permission check failed."))
        }
    }
}

async fn authenticate_user(
    userid: &Userid,
    password: &str,
    path: Option<String>,
    privs: Option<String>,
    port: Option<u16>,
    tfa_challenge: Option<String>,
) -> Result<AuthResult, Error> {
    let auth_context = auth_context()?;
    let prefix = auth_context.auth_prefix();

    let auth_id = Authid::from(userid.clone());
    if !auth_context.auth_id_is_active(&auth_id)? {
        bail!("user account disabled or expired.");
    }

    if let Some(tfa_challenge) = tfa_challenge {
        return authenticate_2nd(userid, &tfa_challenge, password);
    }

    if password.starts_with(prefix) && password.as_bytes().get(prefix.len()).copied() == Some(b':')
    {
        if let Ok(ticket_userid) = Ticket::<Userid>::parse(password)
            .and_then(|ticket| ticket.verify(auth_context.keyring(), prefix, None))
        {
            if *userid == ticket_userid {
                return Ok(AuthResult::CreateTicket);
            }
            bail!("ticket login failed - wrong userid");
        }
    } else if let Some(((path, privs), port)) = path.zip(privs).zip(port) {
        match auth_context.check_path_ticket(userid, password, path, privs, port)? {
            None => (), // no path based tickets supported, just fall through.
            Some(true) => return Ok(AuthResult::Success),
            Some(false) => bail!("No such privilege"),
        }
    }

    #[allow(clippy::let_unit_value)]
    {
        let _: () = auth_context
            .lookup_realm(userid.realm())
            .ok_or_else(|| format_err!("unknown realm {:?}", userid.realm().as_str()))?
            .authenticate_user(userid.name(), password)
            .await?;
    }

    Ok(match login_challenge(userid)? {
        None => AuthResult::CreateTicket,
        Some(challenge) => AuthResult::Partial(Box::new(challenge)),
    })
}

fn authenticate_2nd(
    userid: &Userid,
    challenge_ticket: &str,
    response: &str,
) -> Result<AuthResult, Error> {
    let auth_context = auth_context()?;
    let challenge: Box<TfaChallenge> = Ticket::<ApiTicket>::parse(challenge_ticket)?
        .verify_with_time_frame(
            auth_context.keyring(),
            auth_context.auth_prefix(),
            Some(userid.as_str()),
            -60..600,
        )?
        .require_partial()?;

    #[allow(clippy::let_unit_value)]
    {
        let mut tfa_config_lock = auth_context.tfa_config_write_lock()?;
        let (locked_config, tfa_config) = tfa_config_lock.config_mut();
        if tfa_config
            .verify(
                locked_config,
                userid.as_str(),
                &challenge,
                response.parse()?,
                None,
            )?
            .needs_saving()
        {
            tfa_config_lock.save_config()?;
        }
    }

    Ok(AuthResult::CreateTicket)
}

fn login_challenge(userid: &Userid) -> Result<Option<TfaChallenge>, Error> {
    let auth_context = auth_context()?;
    let mut tfa_config_lock = auth_context.tfa_config_write_lock()?;
    let (locked_config, tfa_config) = tfa_config_lock.config_mut();
    tfa_config.authentication_challenge(locked_config, userid.as_str(), None)
}

fn assemble_csrf_prevention_token(secret: &[u8], userid: &Userid) -> String {
    let epoch = crate::time::epoch_i64();

    let digest = compute_csrf_secret_digest(epoch, secret, userid);

    format!("{:08X}:{}", epoch, digest)
}

fn compute_csrf_secret_digest(timestamp: i64, secret: &[u8], userid: &Userid) -> String {
    let mut hasher = openssl::sha::Sha256::new();
    let data = format!("{:08X}:{}:", timestamp, userid);
    hasher.update(data.as_bytes());
    hasher.update(secret);

    base64::encode_config(hasher.finish(), base64::STANDARD_NO_PAD)
}

pub(crate) fn verify_csrf_prevention_token(
    secret: &[u8],
    userid: &Userid,
    token: &str,
    min_age: i64,
    max_age: i64,
) -> Result<i64, Error> {
    verify_csrf_prevention_token_do(secret, userid, token, min_age, max_age)
        .map_err(|err| format_err!("invalid csrf token - {}", err))
}

fn verify_csrf_prevention_token_do(
    secret: &[u8],
    userid: &Userid,
    token: &str,
    min_age: i64,
    max_age: i64,
) -> Result<i64, Error> {
    use std::collections::VecDeque;

    let mut parts: VecDeque<&str> = token.split(':').collect();

    if parts.len() != 2 {
        bail!("format error - wrong number of parts.");
    }

    let timestamp = parts.pop_front().unwrap();
    let sig = parts.pop_front().unwrap();

    let ttime = i64::from_str_radix(timestamp, 16)
        .map_err(|err| format_err!("timestamp format error - {}", err))?;

    let digest = compute_csrf_secret_digest(ttime, secret, userid);

    if digest != sig {
        bail!("invalid signature.");
    }

    let now = crate::time::epoch_i64();

    let age = now - ttime;
    if age < min_age {
        bail!("timestamp newer than expected.");
    }

    if age > max_age {
        bail!("timestamp too old.");
    }

    Ok(age)
}
