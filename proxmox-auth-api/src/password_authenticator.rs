use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;

use anyhow::{bail, Error};
use serde_json::json;

use proxmox_product_config::open_secret_lockfile;

use crate::types::UsernameRef;

/// A simple password authenticator with a configurable path for a shadow json and lock file.
pub struct PasswordAuthenticator {
    pub config_filename: &'static str,
    pub lock_filename: &'static str,
}

impl crate::api::Authenticator for PasswordAuthenticator {
    fn authenticate_user<'a>(
        &'a self,
        username: &'a UsernameRef,
        password: &'a str,
        client_ip: Option<&'a IpAddr>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            let data = proxmox_sys::fs::file_get_json(self.config_filename, Some(json!({})))?;
            match data[username.as_str()].as_str() {
                None => bail!("no password set"),
                Some(enc_password) => {
                    proxmox_sys::crypt::verify_crypt_pw(password, enc_password)?;

                    // if the password hash is not based on the current hashing function (as
                    // identified by its prefix), rehash the password.
                    if !enc_password.starts_with(proxmox_sys::crypt::HASH_PREFIX) {
                        // only log that we could not upgrade a password, we already know that the
                        // user has a valid password, no reason the deny to log in attempt.
                        if let Err(e) = self.store_password(username, password, client_ip) {
                            log::warn!("could not upgrade a users password! - {e}");
                        }
                    }
                }
            }
            Ok(())
        })
    }

    fn store_password(
        &self,
        username: &UsernameRef,
        password: &str,
        _client_ip: Option<&IpAddr>,
    ) -> Result<(), Error> {
        let enc_password = proxmox_sys::crypt::encrypt_pw(password)?;

        let _guard = open_secret_lockfile(self.lock_filename, None, true);
        let mut data = proxmox_sys::fs::file_get_json(self.config_filename, Some(json!({})))?;
        data[username.as_str()] = enc_password.into();

        let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
        let options = proxmox_sys::fs::CreateOptions::new()
            .perm(mode)
            .owner(nix::unistd::ROOT)
            .group(nix::unistd::Gid::from_raw(0));

        let data = serde_json::to_vec_pretty(&data)?;
        proxmox_sys::fs::replace_file(self.config_filename, &data, options, true)?;

        Ok(())
    }

    fn remove_password(&self, username: &UsernameRef) -> Result<(), Error> {
        let _guard = open_secret_lockfile(self.lock_filename, None, true);

        let mut data = proxmox_sys::fs::file_get_json(self.config_filename, Some(json!({})))?;
        if let Some(map) = data.as_object_mut() {
            map.remove(username.as_str());
        }

        let mode = nix::sys::stat::Mode::from_bits_truncate(0o0600);
        let options = proxmox_sys::fs::CreateOptions::new()
            .perm(mode)
            .owner(nix::unistd::ROOT)
            .group(nix::unistd::Gid::from_raw(0));

        let data = serde_json::to_vec_pretty(&data)?;
        proxmox_sys::fs::replace_file(self.config_filename, &data, options, true)?;

        Ok(())
    }
}
