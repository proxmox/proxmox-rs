//! The Proxmox VE API type crate.

//pub mod api;
mod types;
pub use types::*;

#[cfg(feature = "client-util")]
pub mod client;
