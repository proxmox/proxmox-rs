//! ACME protocol helper.
//!
//! This is supposed to implement the low level parts of the ACME protocol, providing an [`Account`]
//! and some other helper types which allow interacting with an ACME server by implementing methods
//! which create [`Request`]s the user can then combine with a nonce and send to the the ACME
//! server using whatever http client they choose.
//!
//! This is a rather low level crate, and while it provides an optional synchronous client using
//! curl (for simplicity), users should have basic understanding of the ACME API in order to
//! implement a client using this.
//!
//! The [`Account`] helper supports RSA and ECC keys and provides most of the API methods.

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "api-types")]
pub mod types;

#[cfg(feature = "impl")]
mod b64u;
#[cfg(feature = "impl")]
mod eab;
#[cfg(feature = "impl")]
mod json;
#[cfg(feature = "impl")]
mod jws;
#[cfg(feature = "impl")]
mod key;
#[cfg(feature = "impl")]
mod request;

#[cfg(feature = "impl")]
pub mod account;

#[cfg(feature = "impl")]
pub mod authorization;
#[cfg(feature = "impl")]
pub mod directory;
#[cfg(feature = "impl")]
pub mod error;
#[cfg(feature = "impl")]
pub mod order;

#[cfg(feature = "impl")]
pub mod util;

#[cfg(feature = "impl")]
#[doc(inline)]
pub use account::Account;

#[cfg(feature = "impl")]
#[doc(inline)]
pub use authorization::{Authorization, Challenge};

#[cfg(feature = "impl")]
#[doc(inline)]
pub use directory::Directory;

#[cfg(feature = "impl")]
#[doc(inline)]
pub use error::Error;

#[cfg(feature = "impl")]
#[doc(inline)]
pub use order::Order;

#[cfg(feature = "impl")]
#[doc(inline)]
pub use request::Request;

// we don't inline these:
#[cfg(feature = "impl")]
pub use order::NewOrder;
#[cfg(feature = "impl")]
pub use request::ErrorResponse;

/// Header name for nonces.
pub const REPLAY_NONCE: &str = "Replay-Nonce";

/// Header name for locations.
pub const LOCATION: &str = "Location";

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub use client::Client;

#[cfg(feature = "async-client")]
pub mod async_client;
