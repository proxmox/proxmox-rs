mod api_types;
pub use api_types::*;

#[cfg(feature = "dns-impl")]
mod resolv_conf;
#[cfg(feature = "dns-impl")]
pub use resolv_conf::*;
