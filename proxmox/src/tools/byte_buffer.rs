//! ByteBuffer
//!
//! a simple buffer for u8 with a practical api for reading and appending
//! and consuming from the front
//! Example:
//! ```
//! # use std::io::Read;
//! # use proxmox::tools::byte_buffer::ByteBuffer;
//!
//! fn code<T: Read>(input: &mut T) -> std::io::Result<Box<[u8]>> {
//!     let mut buffer = ByteBuffer::new();
//!     let amount = buffer.read_from(input)?;
//!     let data = buffer.consume(amount);
//!     assert_eq!(data.len(), amount);
//!     Ok(data)
//! }
//! ```

use std::cmp::min;
use std::io::{Read, Result};

use crate::tools::vec;

use tokio::io::AsyncReadExt;

pub struct ByteBuffer {
    buf: Box<[u8]>,
    data_size: usize,
    capacity: usize,
}

impl ByteBuffer {
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: vec::undefined(capacity).into_boxed_slice(),
            data_size: 0,
            capacity,
        }
    }

    pub fn data_size(&self) -> usize {
        self.data_size
    }

    pub fn free_size(&self) -> usize {
        self.capacity - self.data_size
    }

    pub fn is_empty(&self) -> bool {
        self.data_size == 0
    }

    pub fn is_full(&self) -> bool {
        self.data_size >= self.capacity
    }

    pub fn add_size(&mut self, size: usize) {
        if self.data_size + size > self.capacity {
            panic!("size bigger than capacity!");
        }
        self.data_size += size;
    }

    pub fn get_data_slice(&self) -> &[u8] {
        &self.buf[..self.data_size]
    }

    pub fn get_free_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf[self.data_size..self.capacity]
    }

    pub fn consume(&mut self, max_amount: usize) -> Box<[u8]> {
        let size = min(max_amount, self.data_size);
        let mut tmp = Vec::with_capacity(size);
        tmp.extend_from_slice(&self.buf[..size]);
        self.buf.copy_within(size..self.capacity, 0);
        self.data_size -= size;
        tmp.into_boxed_slice()
    }

    pub fn read_from<T: Read>(&mut self, input: &mut T) -> Result<usize> {
        let amount = input.read(self.get_free_mut_slice())?;
        self.add_size(amount);
        Ok(amount)
    }

    pub async fn read_from_async<T: AsyncReadExt + Unpin>(&mut self, input: &mut T) -> Result<usize> {
        let amount = input.read(self.get_free_mut_slice()).await?;
        self.add_size(amount);
        Ok(amount)
    }
}

#[cfg(test)]
mod test {
    use crate::tools::byte_buffer::ByteBuffer;

    #[test]
    fn test1() {
        let mut buffer = ByteBuffer::new();
        let slice = buffer.get_free_mut_slice();
        for i in 0..slice.len() {
            slice[i] = (i%255) as u8;
        }
        buffer.add_size(5);

        let slice2 = buffer.get_data_slice();

        assert_eq!(slice2, &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn test2() {
        let mut buffer = ByteBuffer::with_capacity(1024);
        let size = buffer.read_from(&mut std::io::repeat(54)).unwrap();
        assert_eq!(buffer.data_size(), size);
        assert_eq!(buffer.get_data_slice()[0], 54);
    }
}
