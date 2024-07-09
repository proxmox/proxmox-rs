#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod api_types;
pub use api_types::*;

#[cfg(feature = "impl")]
mod journal;
#[cfg(feature = "impl")]
pub use journal::{dump_journal, dump_syslog};
