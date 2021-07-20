use std::path::{Path, PathBuf};

use anyhow::{bail, Error};
use serde_json::{json, Value};

use proxmox::tools::{
    time::epoch_i64,
    fs::{
        replace_file,
        open_file_locked,
        file_get_json,
        CreateOptions,
    },
};

use super::{PublicAuthState, PrivateAuthState};

fn load_auth_state_locked(
    state_dir: &Path,
    realm: &str,
    default: Option<Value>,
) -> Result<(PathBuf, std::fs::File, Vec<Value>), Error> {

    let mut lock_path = state_dir.to_owned();
    lock_path.push(format!("proxmox-openid-auth-state-{}.lck", realm));

    let lock = open_file_locked(
        lock_path,
        std::time::Duration::new(10, 0),
        true,
        CreateOptions::new()
    )?;

    let mut path = state_dir.to_owned();
    path.push(format!("proxmox-openid-auth-state-{}", realm));

    let now = epoch_i64();

    let old_data = file_get_json(&path, default)?;

    let mut data: Vec<Value> = Vec::new();

    let timeout = 10*60; // 10 minutes

    for v in old_data.as_array().unwrap() {
        let ctime = v["ctime"].as_i64().unwrap_or(0);
        if (ctime + timeout) < now {
            continue;
        }
        data.push(v.clone());
    }

    Ok((path, lock, data))
}

fn replace_auth_state(
    path: &Path,
    data: &Vec<Value>,
) -> Result<(), Error> {

    let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
    let options = CreateOptions::new().perm(mode);
    let raw = serde_json::to_string_pretty(data)?;

    replace_file(path, raw.as_bytes(), options)?;

    Ok(())
}

pub fn verify_public_auth_state(
    state_dir: &Path,
    state: &str,
) -> Result<(String, PrivateAuthState), Error> {

    let public_auth_state: PublicAuthState = serde_json::from_str(state)?;

    let (path, _lock, old_data) = load_auth_state_locked(state_dir, &public_auth_state.realm, None)?;

    let mut data: Vec<Value> = Vec::new();

    let mut entry: Option<PrivateAuthState> = None;
    let find_csrf_token = public_auth_state.csrf_token.secret();
    for v in old_data {
        if v["csrf_token"].as_str() == Some(find_csrf_token) {
            entry = Some(serde_json::from_value(v)?);
        } else {
            data.push(v);
        }
    }

    let entry = match entry {
        None => bail!("no openid auth state found (possible timeout)"),
        Some(entry) => entry,
    };

    replace_auth_state(&path, &data)?;

    Ok((public_auth_state.realm, entry))
}

pub fn store_auth_state(
    state_dir: &Path,
    realm: &str,
    auth_state: &PrivateAuthState,
) -> Result<(), Error> {

    let (path, _lock, mut data) = load_auth_state_locked(state_dir, realm, Some(json!([])))?;

    if data.len() > 100 {
        bail!("too many pending openid auth request for realm {}", realm);
    }

    data.push(serde_json::to_value(&auth_state)?);

    replace_auth_state(&path, &data)?;

    Ok(())
}
