#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod subscription_info;
#[cfg(feature = "impl")]
pub use subscription_info::{
    get_hardware_address_candidates, UpdateSubscription, ProductType, ServerId, SetSubscription,
    SubscriptionInfo, SubscriptionStatus,
};

#[cfg(not(feature = "impl"))]
pub use subscription_info::{
    UpdateSubscription, ProductType, SetSubscription, SubscriptionInfo, SubscriptionStatus,
};

#[cfg(feature = "impl")]
pub mod check;
#[cfg(feature = "impl")]
pub mod files;
#[cfg(feature = "impl")]
pub mod sign;
