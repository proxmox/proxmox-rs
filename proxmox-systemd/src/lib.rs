//! Systemd communication.

pub(crate) mod sys;

mod escape;
pub use escape::{UnescapeError, escape_unit, unescape_unit, unescape_unit_path};

pub mod journal;
pub mod notify;

pub mod sd_id128;
