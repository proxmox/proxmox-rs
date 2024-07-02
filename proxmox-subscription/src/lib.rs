#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod subscription_info;
pub use subscription_info::{
    get_hardware_address, ProductType, SubscriptionInfo, SubscriptionStatus,
};

pub mod check;
pub mod files;
pub mod sign;
