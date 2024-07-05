mod compression;

pub use compression::{DeflateEncoder, Level};

const BUFFER_SIZE: usize = 8192;
