use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use anyhow::format_err;
use openssl::x509;

use proxmox_client::Client as ProxmoxClient;
use proxmox_client::{ApiResponse, Environment, Error, HttpClient};

use crate::types::*;

use super::{add_query_arg, add_query_bool};

pub struct Client<C, E: Environment> {
    client: ProxmoxClient<C, E>,
}

impl<C, E: Environment> Client<C, E> {
    /// Get the underlying client object.
    pub fn inner(&self) -> &ProxmoxClient<C, E> {
        &self.client
    }

    /// Get a mutable reference to the underlying client object.
    pub fn inner_mut(&mut self) -> &mut ProxmoxClient<C, E> {
        &mut self.client
    }
}

include!("../generated/code.rs");

#[derive(Default)]
// TODO: Merge this with pbs-client's stuff
pub struct Options {
    /// Set a TLS verification callback.
    callback:
        Option<Box<dyn Fn(bool, &mut x509::X509StoreContextRef) -> bool + Send + Sync + 'static>>,

    fingerprint: Option<Vec<u8>>,

    /// `proxmox_http` based options.
    http_options: proxmox_http::HttpOptions,
}

impl Options {
    /// New default instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a TLS verification callback.
    pub fn tls_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(bool, &mut x509::X509StoreContextRef) -> bool + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(cb));
        self
    }

    /// Expect a specific tls fingerprint. Does not take effect if `tls_callback` is used.
    pub fn tls_fingerprint_str(mut self, fingerprint: &str) -> Result<Self, BadFingerprint> {
        self.fingerprint = Some(parse_fingerprint(fingerprint)?.to_vec());
        Ok(self)
    }

    /// Set the HTTP related options.
    pub fn http_options(mut self, http_options: proxmox_http::HttpOptions) -> Self {
        self.http_options = http_options;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BadFingerprint;

impl fmt::Display for BadFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("failed to parse fingerprint")
    }
}

impl StdError for BadFingerprint {}

fn parse_fingerprint(s: &str) -> Result<[u8; 32], BadFingerprint> {
    use hex::FromHex;

    let hex: Vec<u8> = s
        .as_bytes()
        .iter()
        .copied()
        .filter(|&b| b != b':')
        .collect();

    <[u8; 32]>::from_hex(&hex).map_err(|_| BadFingerprint)
}

pub type HyperClient<E> = Client<Arc<proxmox_http::client::Client>, E>;

impl<E> HyperClient<E>
where
    E: Environment,
    E::Error: From<anyhow::Error>,
    anyhow::Error: From<E::Error>,
{
    pub fn new(env: E, server: &str, options: Options) -> Result<Self, E::Error> {
        use proxmox_client::TlsOptions;

        let tls_options = match options.callback {
            Some(cb) => TlsOptions::Callback(cb),
            None => match options.fingerprint {
                Some(fp) => TlsOptions::Fingerprint(fp.to_vec()),
                None => TlsOptions::default(),
            },
        };

        let client = proxmox_client::HyperClient::with_options(
            format!("https://{server}:8006")
                .parse()
                .map_err(|err| format_err!("bad address: {server:?} - {err}"))?,
            env,
            tls_options,
            options.http_options,
        )?;

        Ok(Self { client })
    }

    pub async fn login(&self) -> Result<(), E::Error> {
        self.client.login().await?;
        Ok(())
    }
}
