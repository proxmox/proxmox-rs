//! Module providing I/O helpers (sync and async).
//!
//! The [`ReadExt`] trait provides additional operations for handling byte buffers for types
//! implementing [`Read`](std::io::Read).

mod read;
pub use read::ReadExt;

mod write;
pub use write::WriteExt;

mod sparse_copy;
pub use sparse_copy::{buffer_is_zero, sparse_copy, SparseCopyResult};

#[cfg(feature = "tokio")]
pub use sparse_copy::sparse_copy_async;

mod byte_buffer;
pub use byte_buffer::ByteBuffer;

pub mod vec;
