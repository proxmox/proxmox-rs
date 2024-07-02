//! ACME API crate (API types and API implementation)
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod types;
pub use types::*;

#[cfg(feature = "impl")]
mod init;
#[cfg(feature = "impl")]
pub use init::*;

#[cfg(feature = "impl")]
mod config;
#[cfg(feature = "impl")]
pub use config::{DEFAULT_ACME_DIRECTORY_ENTRY, KNOWN_ACME_DIRECTORIES};

#[cfg(feature = "impl")]
mod challenge_schemas;
#[cfg(feature = "impl")]
pub use challenge_schemas::{get_cached_challenge_schemas, ChallengeSchemaWrapper};

#[cfg(feature = "impl")]
mod account_config;
#[cfg(feature = "impl")]
pub use account_config::account_config_filename;

#[cfg(feature = "impl")]
mod plugin_config;

#[cfg(feature = "impl")]
mod account_api_impl;
#[cfg(feature = "impl")]
pub use account_api_impl::{
    deactivate_account, get_account, get_tos, list_accounts, register_account, update_account,
};

#[cfg(feature = "impl")]
mod plugin_api_impl;
#[cfg(feature = "impl")]
pub use plugin_api_impl::{add_plugin, delete_plugin, get_plugin, list_plugins, update_plugin};

#[cfg(feature = "impl")]
pub(crate) mod acme_plugin;

#[cfg(feature = "impl")]
mod certificate_helpers;
#[cfg(feature = "impl")]
pub use certificate_helpers::{create_self_signed_cert, order_certificate, revoke_certificate};
