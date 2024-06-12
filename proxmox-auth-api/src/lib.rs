//! Authentication API crate.
//!
//! This contains the API types for `Userid`/`Realm`/`Authid` etc., the PAM authenticator and the
//! authentication API calls.
//!
//! Each can be enabled via a feature:
//!
//! The `pam-authenticator` feature enables the `Pam` type.

pub const TICKET_LIFETIME: i64 = 3600 * 2; // 2 hours

#[cfg(feature = "ticket")]
mod time;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "api")]
pub use api::set_auth_context;

#[cfg(any(feature = "api", feature = "ticket"))]
mod auth_key;

#[cfg(any(feature = "api", feature = "ticket"))]
pub use auth_key::{HMACKey, Keyring, PrivateKey, PublicKey};

#[cfg(feature = "ticket")]
pub mod ticket;

#[cfg(feature = "api-types")]
pub mod types;

#[cfg(feature = "pam-authenticator")]
mod pam_authenticator;
#[cfg(feature = "pam-authenticator")]
pub use pam_authenticator::Pam;

#[cfg(feature = "password-authenticator")]
mod password_authenticator;
#[cfg(feature = "password-authenticator")]
pub use password_authenticator::PasswordAuthenticator;
