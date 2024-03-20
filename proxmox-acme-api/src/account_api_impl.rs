//! ACME account configuration API implementation

use std::ops::ControlFlow;

use anyhow::Error;
use serde_json::json;

use proxmox_acme::async_client::AcmeClient;
use proxmox_acme::types::AccountData as AcmeAccountData;

use proxmox_rest_server::WorkerTask;
use proxmox_sys::task_warn;

use crate::types::{AccountEntry, AccountInfo, AcmeAccountName};
use crate::config::{AcmeApiConfig, DEFAULT_ACME_DIRECTORY_ENTRY};
use crate::account_config::AccountData;

fn account_contact_from_string(s: &str) -> Vec<String> {
    s.split(&[' ', ';', ',', '\0'][..])
        .map(|s| format!("mailto:{}", s))
        .collect()
}

impl AcmeApiConfig {
    pub fn list_accounts(&self) -> Result<Vec<AccountEntry>, Error> {
        let mut entries = Vec::new();
        self.foreach_acme_account(|name| {
            entries.push(AccountEntry { name });
            ControlFlow::Continue(())
        })?;
        Ok(entries)
    }

    pub async fn get_account(
        &self,
        account_name: AcmeAccountName,
    ) -> Result<AccountInfo, Error> {
        let account_data = self.load_account_config(&account_name).await?;
        Ok(AccountInfo {
            location: account_data.location.clone(),
            tos: account_data.tos.clone(),
            directory: account_data.directory_url.clone(),
            account: AcmeAccountData {
                only_return_existing: false, // don't actually write this out in case it's set
                ..account_data.account.clone()
            },
        })
    }

    pub async fn get_tos(&self, directory: Option<String>) -> Result<Option<String>, Error> {
        let directory = directory.unwrap_or_else(|| DEFAULT_ACME_DIRECTORY_ENTRY.url.to_string());
        Ok(AcmeClient::new(directory)
            .terms_of_service_url()
            .await?
            .map(str::to_owned))
    }

    pub async fn register_account(
        &self,
        name: &AcmeAccountName,
        contact: String,
        tos_url: Option<String>,
        directory_url: String,
        eab_creds: Option<(String, String)>,
    ) -> Result<String, Error> {
        let mut client = AcmeClient::new(directory_url.clone());

        let contact = account_contact_from_string(&contact);
        let account = client
            .new_account(tos_url.is_some(), contact, None, eab_creds)
            .await?;

        let account = AccountData::from_account_dir_tos(account, directory_url, tos_url);

        self.create_account_config(&name, &account)?;

        Ok(account.location)
    }

    pub async fn deactivate_account(
        &self,
        worker: &WorkerTask,
        name: &AcmeAccountName,
        force: bool,
    ) -> Result<(), Error> {
        let mut account_data = self.load_account_config(name).await?;
        let mut client = account_data.client();

        match client
            .update_account(&json!({"status": "deactivated"}))
            .await
        {
            Ok(account) => {
                account_data.account = account.data.clone();
                self.save_account_config(&name, &account_data)?;
            }
            Err(err) if !force => return Err(err),
            Err(err) => {
                task_warn!(
                    worker,
                    "error deactivating account {}, proceedeing anyway - {}",
                    name,
                    err,
                );
            }
        }

        self.mark_account_deactivated(&name)?;

        Ok(())
    }

    pub async fn update_account(
        &self,
        name: &AcmeAccountName,
        contact: Option<String>,
    ) -> Result<(), Error> {
        let mut account_data = self.load_account_config(name).await?;
        let mut client = account_data.client();

        let data = match contact {
            Some(contact) => json!({
                "contact": account_contact_from_string(&contact),
            }),
            None => json!({}),
        };

        let account = client.update_account(&data).await?;
        account_data.account = account.data.clone();
        self.save_account_config(&name, &account_data)?;

        Ok(())
    }
}
