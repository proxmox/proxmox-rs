mod compression;
mod decompression;

pub use compression::{DeflateEncoder, Level};
pub use decompression::DeflateDecoder;

const BUFFER_SIZE: usize = 8192;
