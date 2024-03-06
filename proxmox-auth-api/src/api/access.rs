//! Provides the "/access/ticket" API call.

use anyhow::{bail, format_err, Error};
use openssl::hash::MessageDigest;
use serde_json::{json, Value};

use proxmox_rest_server::RestEnvironment;
use proxmox_router::{http_err, Permission, RpcEnvironment};
use proxmox_schema::{api, api_types::PASSWORD_SCHEMA};
use proxmox_tfa::api::TfaChallenge;

use super::ApiTicket;
use super::{auth_context, HMACKey};
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
    let env: &RestEnvironment = rpcenv
        .as_any()
        .downcast_ref::<RestEnvironment>()
        .ok_or_else(|| format_err!("detected wrong RpcEnvironment type"))?;

    match authenticate_user(&username, &password, path, privs, port, tfa_challenge, env).await {
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
    rpcenv: &RestEnvironment,
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

    let client_ip = rpcenv.get_client_ip().map(|sa| sa.ip());

    #[allow(clippy::let_unit_value)]
    {
        let _: () = auth_context
            .lookup_realm(userid.realm())
            .ok_or_else(|| format_err!("unknown realm {:?}", userid.realm().as_str()))?
            .authenticate_user(userid.name(), password, client_ip.as_ref())
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
        use proxmox_tfa::api::TfaResult;

        let mut tfa_config_lock = auth_context.tfa_config_write_lock()?;
        let (locked_config, tfa_config) = tfa_config_lock.config_mut();
        let result = tfa_config.verify(
            locked_config,
            userid.as_str(),
            &challenge,
            response.parse()?,
            None,
        );

        let (success, needs_saving) = match result {
            TfaResult::Locked => (false, false),
            TfaResult::Failure { needs_saving, .. } => {
                // TODO: Implement notifications for totp/tfa limits!
                (false, needs_saving)
            }
            TfaResult::Success { needs_saving } => (true, needs_saving),
        };
        if needs_saving {
            tfa_config_lock.save_config()?;
        }
        if !success {
            bail!("authentication failed");
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

pub fn assemble_csrf_prevention_token(secret: &HMACKey, userid: &Userid) -> String {
    let epoch = crate::time::epoch_i64();

    let data = csrf_token_data(epoch, userid);
    let digest = base64::encode_config(
        secret.sign(MessageDigest::sha3_256(), &data).unwrap(),
        base64::STANDARD_NO_PAD,
    );
    format!("{:08X}:{}", epoch, digest)
}

fn csrf_token_data(timestamp: i64, userid: &Userid) -> Vec<u8> {
    format!("{:08X}:{}:", timestamp, userid).as_bytes().to_vec()
}

pub(crate) fn verify_csrf_prevention_token(
    secret: &HMACKey,
    userid: &Userid,
    token: &str,
    min_age: i64,
    max_age: i64,
) -> Result<i64, Error> {
    verify_csrf_prevention_token_do(secret, userid, token, min_age, max_age)
        .map_err(|err| format_err!("invalid csrf token - {}", err))
}

fn verify_csrf_prevention_token_do(
    secret: &HMACKey,
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
    let sig = base64::decode_config(sig, base64::STANDARD_NO_PAD)
        .map_err(|e| format_err!("could not base64 decode csrf signature - {e}"))?;

    let ttime = i64::from_str_radix(timestamp, 16)
        .map_err(|err| format_err!("timestamp format error - {}", err))?;

    let verify = secret.verify(
        MessageDigest::sha3_256(),
        &sig,
        &csrf_token_data(ttime, userid),
    );

    if verify.is_err() || !verify? {
        // legacy token verification code
        // TODO: remove once all dependent products had a major version release (PBS)
        let mut hasher = openssl::sha::Sha256::new();
        let data = format!("{:08X}:{}:", ttime, userid);
        hasher.update(data.as_bytes());
        hasher.update(&secret.as_bytes()?);
        let old_digest = hasher.finish();

        if old_digest.len() != sig.len() || !openssl::memcmp::eq(&old_digest, &sig) {
            bail!("invalid signature.");
        }
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

#[test]
fn test_assemble_and_verify_csrf_token() {
    let secret = HMACKey::generate().expect("failed to generate HMAC key for testing");

    let userid: Userid = "name@realm"
        .parse()
        .expect("could not parse user id for HMAC testing");
    let token = assemble_csrf_prevention_token(&secret, &userid);

    verify_csrf_prevention_token(&secret, &userid, &token, -300, 300)
        .expect("could not verify csrf for testing");
}

#[test]
fn test_verify_legacy_csrf_tokens() {
    use openssl::rsa::Rsa;

    // assemble legacy key and token
    let key = Rsa::generate(2048)
        .expect("could not generate RSA key for testing")
        .private_key_to_pem()
        .expect("could not create private PEM for testing");
    let userid: Userid = "name@realm"
        .parse()
        .expect("could not parse the user id for legacy csrf testing");
    let epoch = crate::time::epoch_i64();

    let mut hasher = openssl::sha::Sha256::new();
    let data = format!("{:08X}:{}:", epoch, userid);
    hasher.update(data.as_bytes());
    hasher.update(&key);
    let old_digest = base64::encode_config(hasher.finish(), base64::STANDARD_NO_PAD);

    let token = format!("{:08X}:{}", epoch, old_digest);

    // load key into new hmackey wrapper and verify
    let string = base64::encode_config(key.clone(), base64::STANDARD_NO_PAD);
    let secret =
        HMACKey::from_base64(&string).expect("could not create HMAC key from base64 for testing");
    verify_csrf_prevention_token(&secret, &userid, &token, -300, 300).unwrap();
}
