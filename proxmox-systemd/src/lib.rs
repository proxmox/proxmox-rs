//! Systemd communication.

pub(crate) mod sys;

mod escape;
pub use escape::{escape_unit, unescape_unit, unescape_unit_path, UnescapeError};

pub mod journal;
pub mod notify;
