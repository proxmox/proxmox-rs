//! ACME account configuration helpers (load/save config)

use std::fs::OpenOptions;
use std::ops::ControlFlow;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};

use proxmox_sys::error::SysError;
use proxmox_sys::fs::{replace_file, CreateOptions};

use proxmox_schema::api_types::SAFE_ID_REGEX;

use proxmox_acme::async_client::AcmeClient;
use proxmox_acme::types::AccountData as AcmeAccountData;
use proxmox_acme::Account;

use crate::types::AcmeAccountName;

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

// Our on-disk format inherited from PVE's proxmox-acme code.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    /// The account's location URL.
    pub location: String,

    /// The account data.
    pub account: AcmeAccountData,

    /// The private key as PEM formatted string.
    pub key: String,

    /// ToS URL the user agreed to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tos: Option<String>,

    #[serde(skip_serializing_if = "is_false", default)]
    pub debug: bool,

    /// The directory's URL.
    pub directory_url: String,
}

impl AccountData {
    pub fn from_account_dir_tos(
        account: &Account,
        directory_url: String,
        tos: Option<String>,
    ) -> Self {
        AccountData {
            location: account.location.clone(),
            key: account.private_key.clone(),
            account: AcmeAccountData {
                only_return_existing: false, // don't actually write this out in case it's set
                ..account.data.clone()
            },
            debug: false,
            tos,
            directory_url,
        }
    }

    pub fn client(&self) -> AcmeClient {
        let mut client = AcmeClient::new(self.directory_url.clone());
        client.set_account(Account {
            location: self.location.clone(),
            private_key: self.key.clone(),
            data: self.account.clone(),
        });
        client
    }
}

fn acme_account_dir() -> PathBuf {
    super::config::acme_config_dir().join("accounts")
}

/// Returns the path to the account configuration file (`$config_dir/accounts/$name`).
pub fn account_cfg_filename(name: &str) -> PathBuf {
    acme_account_dir().join(name)
}

fn make_acme_account_dir() -> nix::Result<()> {
    super::config::make_acme_dir()?;
    super::config::create_secret_subdir(acme_account_dir())
}

pub(crate) fn foreach_acme_account<F>(mut func: F) -> Result<(), Error>
where
    F: FnMut(AcmeAccountName) -> ControlFlow<Result<(), Error>>,
{
    match proxmox_sys::fs::scan_subdir(-1, acme_account_dir().as_path(), &SAFE_ID_REGEX) {
        Ok(files) => {
            for file in files {
                let file = file?;
                let file_name = unsafe { file.file_name_utf8_unchecked() };

                if file_name.starts_with('_') {
                    continue;
                }

                let account_name = match AcmeAccountName::from_string(file_name.to_owned()) {
                    Ok(account_name) => account_name,
                    Err(_) => continue,
                };

                if let ControlFlow::Break(result) = func(account_name) {
                    return result;
                }
            }
            Ok(())
        }
        Err(err) if err.not_found() => Ok(()),
        Err(err) => Err(err.into()),
    }
}

// Mark account as deactivated
pub(crate) fn mark_account_deactivated(account_name: &str) -> Result<(), Error> {
    let from = account_cfg_filename(account_name);
    for i in 0..100 {
        let to = account_cfg_filename(&format!("_deactivated_{}_{}", account_name, i));
        if !Path::new(&to).exists() {
            return std::fs::rename(&from, &to).map_err(|err| {
                format_err!(
                    "failed to move account path {:?} to {:?} - {}",
                    from,
                    to,
                    err
                )
            });
        }
    }
    bail!(
        "No free slot to rename deactivated account {:?}, please cleanup {:?}",
        from,
        acme_account_dir()
    );
}

// Load an existing ACME account by name.
pub(crate) async fn load_account_config(account_name: &str) -> Result<AccountData, Error> {
    let account_cfg_filename = account_cfg_filename(account_name);
    let data = match tokio::fs::read(&account_cfg_filename).await {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            bail!("acme account '{}' does not exist", account_name)
        }
        Err(err) => bail!(
            "failed to load acme account from {:?} - {}",
            account_cfg_filename,
            err
        ),
    };
    let data: AccountData = serde_json::from_slice(&data).map_err(|err| {
        format_err!(
            "failed to parse acme account from {:?} - {}",
            account_cfg_filename,
            err
        )
    })?;

    Ok(data)
}

// Save an new ACME account (fails if the file already exists).
pub(crate) fn create_account_config(
    account_name: &AcmeAccountName,
    account: &AccountData,
) -> Result<(), Error> {
    make_acme_account_dir()?;

    let account_cfg_filename = account_cfg_filename(account_name.as_ref());
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&account_cfg_filename)
        .map_err(|err| {
            format_err!(
                "failed to open {:?} for writing: {}",
                account_cfg_filename,
                err
            )
        })?;

    serde_json::to_writer_pretty(file, account).map_err(|err| {
        format_err!(
            "failed to write acme account to {:?}: {}",
            account_cfg_filename,
            err
        )
    })?;

    Ok(())
}

// Save ACME account data (overtwrite existing data).
pub(crate) fn save_account_config(
    account_name: &AcmeAccountName,
    account: &AccountData,
) -> Result<(), Error> {
    let account_cfg_filename = account_cfg_filename(account_name.as_ref());

    let mut data = Vec::<u8>::new();
    serde_json::to_writer_pretty(&mut data, account).map_err(|err| {
        format_err!(
            "failed to serialize acme account to {:?}: {}",
            account_cfg_filename,
            err
        )
    })?;

    make_acme_account_dir()?;

    replace_file(
        account_cfg_filename,
        &data,
        CreateOptions::new()
            .perm(nix::sys::stat::Mode::from_bits_truncate(0o600))
            .owner(nix::unistd::ROOT)
            .group(nix::unistd::Gid::from_raw(0)),
        true,
    )
}
