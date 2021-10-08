//! HTTP related utilities used by various Proxmox products.

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(feature = "http-helpers")]
pub mod tls;

#[cfg(feature = "http-helpers")]
pub mod uri;

#[cfg(feature = "http-helpers")]
pub mod proxy_config;
#[cfg(feature = "http-helpers")]
pub use proxy_config::ProxyConfig;

#[cfg(feature = "client")]
pub mod client;
