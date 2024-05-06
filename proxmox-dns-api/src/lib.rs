mod api_types;
pub use api_types::{DeletableResolvConfProperty, ResolvConf, ResolvConfWithDigest};
pub use api_types::{
    FIRST_DNS_SERVER_SCHEMA, SEARCH_DOMAIN_SCHEMA, SECOND_DNS_SERVER_SCHEMA,
    THIRD_DNS_SERVER_SCHEMA,
};

#[cfg(feature = "impl")]
mod resolv_conf;
#[cfg(feature = "impl")]
pub use resolv_conf::*;
