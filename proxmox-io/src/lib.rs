//! Module providing I/O helpers (sync and async).
//!
//! The [`ReadExt`] trait provides additional operations for handling byte buffers for types
//! implementing [`Read`](std::io::Read).

#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod range_reader;
pub use range_reader::RangeReader;

mod read;
pub use read::ReadExt;

mod write;
pub use write::WriteExt;

mod sparse_copy;
pub use sparse_copy::{buffer_is_zero, sparse_copy, SparseCopyResult};

#[cfg(feature = "tokio")]
pub use sparse_copy::sparse_copy_async;

mod std_channel_writer;
pub use std_channel_writer::StdChannelWriter;

mod byte_buffer;
pub use byte_buffer::ByteBuffer;

pub mod boxed;
pub mod vec;
