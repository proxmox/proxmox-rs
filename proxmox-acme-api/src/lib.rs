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

#[cfg(feature = "impl")]
pub mod completion {

    use std::collections::HashMap;
    use std::ops::ControlFlow;

    use crate::account_config::foreach_acme_account;
    use crate::challenge_schemas::load_dns_challenge_schema;
    use crate::plugin_config::plugin_config;

    pub fn complete_acme_account(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
        let mut out = Vec::new();
        let _ = foreach_acme_account(|name| {
            out.push(name.into_string());
            ControlFlow::Continue(())
        });
        out
    }

    pub fn complete_acme_plugin(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
        match plugin_config() {
            Ok((config, _digest)) => config
                .iter()
                .map(|(id, (_type, _cfg))| id.clone())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn complete_acme_plugin_type(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
        vec![
            "dns".to_string(),
            //"http".to_string(), // makes currently not really sense to create or the like
        ]
    }

    pub fn complete_acme_api_challenge_type(
        _arg: &str,
        param: &HashMap<String, String>,
    ) -> Vec<String> {
        if param.get("type") == Some(&"dns".to_string()) {
            match load_dns_challenge_schema() {
                Ok(schema) => schema.into_iter().map(|s| s.id).collect(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }
}

#[cfg(feature = "impl")]
pub use completion::{
    complete_acme_account, complete_acme_api_challenge_type, complete_acme_plugin,
    complete_acme_plugin_type,
};
