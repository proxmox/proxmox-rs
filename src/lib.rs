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

mod b64u;
mod json;
mod jws;
mod key;
mod request;

pub mod account;
pub mod authorization;
pub mod directory;
pub mod error;
pub mod order;
pub mod util;

#[doc(inline)]
pub use account::Account;

#[doc(inline)]
pub use authorization::{Authorization, Challenge};

#[doc(inline)]
pub use directory::Directory;

#[doc(inline)]
pub use error::Error;

#[doc(inline)]
pub use order::Order;

#[doc(inline)]
pub use request::Request;

// we don't inline these:
pub use order::NewOrder;
pub use request::ErrorResponse;

/// Header name for nonces.
pub const REPLAY_NONCE: &str = "Replay-Nonce";

/// Header name for locations.
pub const LOCATION: &str = "Location";

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub use client::Client;
