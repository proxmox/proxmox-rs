#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod api;
pub use api::{add_repository_handle, change_repository, get_changelog, list_repositories};

#[cfg(feature = "cache")]
pub mod cache;
#[cfg(feature = "cache")]
mod cache_api;
#[cfg(feature = "cache")]
pub use cache_api::{get_package_versions, list_available_apt_update, update_database};

pub mod deb822;
pub mod repositories;
