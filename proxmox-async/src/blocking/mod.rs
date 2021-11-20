//! Async wrappers for blocking I/O (adding `block_in_place` around
//! channels/readers)

mod std_channel_stream;
pub use std_channel_stream::StdChannelStream;

mod wrapped_reader_stream;
pub use wrapped_reader_stream::WrappedReaderStream;
