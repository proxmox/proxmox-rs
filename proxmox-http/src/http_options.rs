use crate::ProxyConfig;

/// Options for an HTTP client.
#[derive(Default)]
pub struct HttpOptions {
    /// Proxy configuration
    pub proxy_config: Option<ProxyConfig>,
    /// `User-Agent` header value
    pub user_agent: Option<String>,
    /// TCP keepalive time, defaults to 7200
    pub tcp_keepalive: Option<u32>,
}

impl HttpOptions {
    pub fn get_proxy_authorization(&self) -> Option<String> {
        if let Some(ref proxy_config) = self.proxy_config {
            if !proxy_config.force_connect {
                return proxy_config.authorization.clone();
            }
        }

        None
    }
}
