mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod resolv_conf;
#[cfg(feature = "impl")]
pub use resolv_conf::*;
