//! This provides an API-stable wrapper for the `base64` crate with serde helpers.
//!
//! Since the base64 create's API has changed quite a bit and has been unwieldy at times as well as
//! having changed its behavior with respect to whether padding when decoding is required or
//! optional, this should provide a "stable" API abstraction with these things made explicit.

mod error;
use error::ConvertError;
pub use error::{DecodeError, EncodeError};

#[macro_use]
mod implementation;

implement_kind!(
    &base64::alphabet::STANDARD,
    #[doc = "base64"]
    ///use proxmox_base64::Display;
    ///    "MX5+Mg=="
    ///use proxmox_base64::DisplayNoPad;
    ///    "MX5+Mg"
    ///    #[serde(serialize_with = "proxmox_base64::serialize_as_base64")]
    ///let encoded = r#"{"data":"MX5+Mg=="}"#;
    ///    #[serde(serialize_with = "proxmox_base64::serialize_as_base64_no_pad")]
    ///let encoded = r#"{"data":"MX5+Mg"}"#;
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_from_base64")]
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_from_base64_pad")]
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_from_base64_no_pad")]
    ///    #[serde(with = "proxmox_base64::as_base64")]
    ///    #[serde(with = "proxmox_base64::as_base64_no_pad_indifferent")]
    ///    #[serde(with = "proxmox_base64::as_base64_must_pad")]
    ///    #[serde(with = "proxmox_base64::as_base64_must_not_pad")]
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_string_from_base64")]
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_string_from_base64_pad")]
    ///    #[serde(deserialize_with = "proxmox_base64::deserialize_string_from_base64_no_pad")]
    ///    #[serde(with = "proxmox_base64::string_as_base64")]
    ///    #[serde(with = "proxmox_base64::string_as_base64_no_pad_indifferent")]
    ///    #[serde(with = "proxmox_base64::string_as_base64_must_pad")]
    ///    #[serde(with = "proxmox_base64::string_as_base64_must_not_pad")]
);

pub mod url {
    //! This provides the same API as the top level module but with URL safe encoding.

    use crate::{ConvertError, DecodeError};

    implement_kind!(
        &base64::alphabet::URL_SAFE,
        #[doc = "base64url"]
        ///use proxmox_base64::url::Display;
        ///    "MX5-Mg=="
        ///use proxmox_base64::url::DisplayNoPad;
        ///    "MX5-Mg"
        ///    #[serde(serialize_with = "proxmox_base64::url::serialize_as_base64")]
        ///let encoded = r#"{"data":"MX5-Mg=="}"#;
        ///    #[serde(serialize_with = "proxmox_base64::url::serialize_as_base64_no_pad")]
        ///let encoded = r#"{"data":"MX5-Mg"}"#;
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_from_base64")]
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_from_base64_pad")]
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_from_base64_no_pad")]
        ///    #[serde(with = "proxmox_base64::url::as_base64")]
        ///    #[serde(with = "proxmox_base64::url::as_base64_no_pad_indifferent")]
        ///    #[serde(with = "proxmox_base64::url::as_base64_must_pad")]
        ///    #[serde(with = "proxmox_base64::url::as_base64_must_not_pad")]
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_string_from_base64")]
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_string_from_base64_pad")]
        ///    #[serde(deserialize_with = "proxmox_base64::url::deserialize_string_from_base64_no_pad")]
        ///    #[serde(with = "proxmox_base64::url::string_as_base64")]
        ///    #[serde(with = "proxmox_base64::url::string_as_base64_no_pad_indifferent")]
        ///    #[serde(with = "proxmox_base64::url::string_as_base64_must_pad")]
        ///    #[serde(with = "proxmox_base64::url::string_as_base64_must_not_pad")]
    );
}
