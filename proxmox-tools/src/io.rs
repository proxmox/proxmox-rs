//! Module providing I/O helpers (sync and async).
//!
//! The [`ReadExt`] trait provides additional operations for handling byte buffers for types
//! implementing [`Read`](std::io::Read).

mod read;
pub use read::*;

mod write;
pub use write::*;
