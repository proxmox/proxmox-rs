#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod subscription_info;
#[cfg(feature = "impl")]
pub use subscription_info::{
    ProductType, ServerId, SetSubscription, SubscriptionInfo, SubscriptionStatus,
    UpdateSubscription, get_hardware_address_candidates,
};

#[cfg(not(feature = "impl"))]
pub use subscription_info::{
    ProductType, SetSubscription, SubscriptionInfo, SubscriptionStatus, UpdateSubscription,
};

#[cfg(feature = "impl")]
pub mod check;
#[cfg(feature = "impl")]
pub mod files;
#[cfg(feature = "impl")]
pub mod sign;
