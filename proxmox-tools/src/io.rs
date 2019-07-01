//! Module providing I/O helpers (sync and async).
//!
//! The [`ReadExt`] trait provides additional operations for handling byte buffers for types
//! implementing [`Read`](std::io::Read).

// DEPRECATED:
pub mod ops {
    pub use super::ReadExt as ReadExtOps;
}

mod read;
pub use read::*;
