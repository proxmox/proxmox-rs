#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod verifier;

pub use verifier::{WeakCryptoConfig, WeakCryptoConfigUpdater, verify_signature};
