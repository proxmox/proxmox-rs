use std::collections::HashMap;

use anyhow::{bail, format_err, Error};
use serde_json::{from_value, Value};

use proxmox_auth_api::types::Authid;
use proxmox_product_config::{open_api_lockfile, replace_config, ApiLockGuard};

use crate::init::impl_feature::{token_shadow, token_shadow_lock};

// Get exclusive lock
fn lock_config() -> Result<ApiLockGuard, Error> {
    open_api_lockfile(token_shadow_lock(), None, true)
}

fn read_file() -> Result<HashMap<Authid, String>, Error> {
    let json = proxmox_sys::fs::file_get_json(token_shadow(), Some(Value::Null))?;

    if json == Value::Null {
        Ok(HashMap::new())
    } else {
        // swallow serde error which might contain sensitive data
        from_value(json)
            .map_err(|_err| format_err!("unable to parse '{}'", token_shadow().display()))
    }
}

fn write_file(data: HashMap<Authid, String>) -> Result<(), Error> {
    let json = serde_json::to_vec(&data)?;
    replace_config(token_shadow(), &json)
}

/// Verifies that an entry for given tokenid / API token secret exists
pub fn verify_secret(tokenid: &Authid, secret: &str) -> Result<(), Error> {
    if !tokenid.is_token() {
        bail!("not an API token ID");
    }

    let data = read_file()?;
    match data.get(tokenid) {
        Some(hashed_secret) => proxmox_sys::crypt::verify_crypt_pw(secret, hashed_secret),
        None => bail!("invalid API token"),
    }
}

/// Adds a new entry for the given tokenid / API token secret. The secret is stored as salted hash.
pub fn set_secret(tokenid: &Authid, secret: &str) -> Result<(), Error> {
    if !tokenid.is_token() {
        bail!("not an API token ID");
    }

    let _guard = lock_config()?;

    let mut data = read_file()?;
    let hashed_secret = proxmox_sys::crypt::encrypt_pw(secret)?;
    data.insert(tokenid.clone(), hashed_secret);
    write_file(data)?;

    Ok(())
}

/// Deletes the entry for the given tokenid.
pub fn delete_secret(tokenid: &Authid) -> Result<(), Error> {
    if !tokenid.is_token() {
        bail!("not an API token ID");
    }

    let _guard = lock_config()?;

    let mut data = read_file()?;
    data.remove(tokenid);
    write_file(data)?;

    Ok(())
}

/// Generates a new secret for the given tokenid / API token, sets it then returns it.
/// The secret is stored as salted hash.
pub fn generate_and_set_secret(tokenid: &Authid) -> Result<String, Error> {
    let secret = format!("{:x}", proxmox_uuid::Uuid::generate());
    set_secret(tokenid, &secret)?;
    Ok(secret)
}
