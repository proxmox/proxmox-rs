//! HTTP proxy configuration.
//!
//! This can be used with the async [`Client`](crate::client::Client) or sync [`Client`](crate::client::sync::Client).

use anyhow::{bail, format_err, Error};

use http::Uri;

use crate::uri::build_authority;

/// HTTP Proxy Configuration
#[derive(Clone)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub authorization: Option<String>, // user:pass
    pub force_connect: bool,
}

impl ProxyConfig {
    /// Parse proxy config from ALL_PROXY environment var
    pub fn from_proxy_env() -> Result<Option<ProxyConfig>, Error> {
        // We only support/use ALL_PROXY environment

        match std::env::var_os("ALL_PROXY") {
            None => Ok(None),
            Some(all_proxy) => {
                let all_proxy = match all_proxy.to_str() {
                    Some(s) => String::from(s),
                    None => bail!("non UTF-8 content in env ALL_PROXY"),
                };
                if all_proxy.is_empty() {
                    return Ok(None);
                }
                let config = Self::parse_proxy_url(&all_proxy)?;
                Ok(Some(config))
            }
        }
    }

    /// Parse proxy configuration string `[http://]<host>[:port]`
    ///
    /// Default port is 1080 (like curl)
    pub fn parse_proxy_url(http_proxy: &str) -> Result<ProxyConfig, Error> {
        Self::parse_proxy_url_do(http_proxy)
            .map_err(|err| format_err!("parse_proxy_url failed: {}", err))
    }

    fn parse_proxy_url_do(http_proxy: &str) -> Result<ProxyConfig, Error> {
        let proxy_uri: Uri = http_proxy.parse()?;
        let proxy_authority = match proxy_uri.authority() {
            Some(authority) => authority,
            None => bail!("missing proxy authority"),
        };
        let host = proxy_authority.host().to_owned();
        let port = match proxy_uri.port() {
            Some(port) => port.as_u16(),
            None => 1080, // CURL default port
        };

        match proxy_uri.scheme_str() {
            Some("http") => { /* Ok */ }
            Some(scheme) => bail!("unsupported proxy scheme '{}'", scheme),
            None => { /* assume HTTP */ }
        }

        let authority_vec: Vec<&str> = proxy_authority.as_str().rsplitn(2, '@').collect();
        let authorization = if authority_vec.len() == 2 {
            Some(authority_vec[1].to_string())
        } else {
            None
        };

        Ok(ProxyConfig {
            host,
            port,
            authorization,
            force_connect: false,
        })
    }

    /// Assemble canonical proxy string (including scheme and port)
    pub fn to_proxy_string(&self) -> Result<String, Error> {
        let authority = build_authority(&self.host, self.port)?;
        Ok(match self.authorization {
            None => format!("http://{authority}"),
            Some(ref authorization) => format!("http://{authorization}@{authority}"),
        })
    }
}
