//! HTTP related utilities used by various Proxmox products.

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(any(feature = "http-helpers"))]
pub mod tls;

#[cfg(any(feature = "http-helpers"))]
pub mod uri;

#[cfg(any(feature = "http-helpers"))]
pub mod proxy_config;
#[cfg(any(feature = "http-helpers"))]
pub use proxy_config::ProxyConfig;

#[cfg(feature = "client")]
pub mod client;
