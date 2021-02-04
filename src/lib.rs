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

pub use account::Account;
pub use authorization::{Authorization, Challenge};
pub use directory::Directory;
pub use error::Error;
pub use order::{NewOrder, Order};
pub use request::{ErrorResponse, Request};

/// Header name for nonces.
pub const REPLAY_NONCE: &str = "Replay-Nonce";

/// Header name for locations.
pub const LOCATION: &str = "Location";

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub use client::Client;
