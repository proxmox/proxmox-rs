//! ByteBuffer
//!
//! a simple buffer for u8 with a practical api for reading and appending
//! and consuming from the front
//! Example:
//! ```
//! # use std::io::Read;
//! # use proxmox::tools::byte_buffer::ByteBuffer;
//! fn code<T: Read + ?Sized>(input: &mut T) -> std::io::Result<Box<[u8]>> {
//!     let mut buffer = ByteBuffer::new();
//!     let amount = buffer.read_from(input)?;
//!     let data = buffer.consume(amount);
//!     assert_eq!(data.len(), amount);
//!     Ok(data)
//! }
//! # code(&mut &b"testdata"[..]).expect("byte buffer test failed");
//! ```

use std::cmp::min;
use std::io::{Read, Result};

use crate::tools::vec;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncReadExt};

/// A Buffer that holds bytes (u8)
/// has convenience methods for reading (with any reader that
/// implements std::io::Read or tokio::io::AsyncRead) onto the back
/// and consuming from the front
pub struct ByteBuffer {
    buf: Box<[u8]>,
    data_size: usize,
    capacity: usize,
}

impl ByteBuffer {

    /// Creates a new instance with a default capacity of 4096
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

    /// Returns the length of the data in the Buffer
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

    /// Sets the length of the data. Useful if data was manually added
    /// with a mutable slice (e.g. from [get_free_mut_slice](#method.get_free_mut_slice)).
    ///
    /// Panics when new size would be greater than capacity.
    ///
    /// Example:
    /// ```
    /// # use proxmox::tools::byte_buffer::ByteBuffer;
    /// let mut buf = ByteBuffer::new();
    /// buf.get_free_mut_slice()[..1].copy_from_slice(&[1u8]);
    /// buf.add_size(1);
    /// ```
    ///
    /// This code will panic:
    /// ```should_panic
    /// # use proxmox::tools::byte_buffer::ByteBuffer;
    /// let mut buf = ByteBuffer::with_capacity(128);
    /// buf.add_size(256);
    /// ```
    pub fn add_size(&mut self, size: usize) {
        if self.data_size + size > self.capacity {
            panic!("size bigger than capacity!");
        }
        self.data_size += size;
    }

    /// Gets an immutable reference to the data in the buffer
    /// Example:
    /// ```
    /// # use proxmox::tools::byte_buffer::ByteBuffer;
    /// let mut buf = ByteBuffer::new();
    /// buf.get_free_mut_slice()[..2].copy_from_slice(&[1u8, 2u8]);
    /// buf.add_size(2);
    /// assert_eq!(buf.get_data_slice(), &[1u8, 2u8]);
    /// ```
    pub fn get_data_slice(&self) -> &[u8] {
        &self.buf[..self.data_size]
    }

    /// Returns a mutable reference to the free section of the
    /// Buffer. There are no guarantees about the content of the
    /// free part of the Buffer (may even be uninitialized).
    /// see [add_size](#method.add_size) for a usage example.
    pub fn get_free_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf[self.data_size..self.capacity]
    }

    /// Consumes up to max_amount of data from the front
    /// of the buffer. If there was less than max_amount present,
    /// it will return as much data as there was in the buffer.
    /// The rest of the data will be moved to the front, and
    /// the data size will be updated accordingly.
    ///
    /// Example:
    /// ```
    /// # use proxmox::tools::byte_buffer::ByteBuffer;
    /// let mut buf = ByteBuffer::new();
    /// buf.get_free_mut_slice()[..2].copy_from_slice(&[1u8, 2u8]);
    /// buf.add_size(2);
    /// assert_eq!(buf.data_size(), 2);
    ///
    /// let data = buf.consume(100);
    /// assert_eq!(&data[..], &[1u8, 2u8]);
    /// assert!(buf.is_empty());
    /// ```
    pub fn consume(&mut self, max_amount: usize) -> Box<[u8]> {
        let size = min(max_amount, self.data_size);
        let tmp: Box<[u8]> = self.buf[..size].into();
        self.buf.copy_within(size..self.capacity, 0);
        self.data_size -= size;
        tmp
    }

    /// Takes a reader and reads into the back of the buffer (up to the
    /// free space in the buffer) and updates its size accordingly.
    ///
    /// Example:
    /// ```norun
    /// // create some reader
    /// let reader = ...;
    ///
    /// let mut buf = ByteBuffer::new();
    /// let res = buf.read_from(reader);
    /// // do something with the buffer
    /// ...
    /// ```
    pub fn read_from<T: Read + ?Sized>(&mut self, input: &mut T) -> Result<usize> {
        let amount = input.read(self.get_free_mut_slice())?;
        self.add_size(amount);
        Ok(amount)
    }

    /// Same as read_from, but for reader that implement tokio::io::AsyncRead.
    /// See [read_from](#method.read_from) for an example
    #[cfg(feature = "tokio")]
    pub async fn read_from_async<T: AsyncRead + Unpin>(&mut self, input: &mut T) -> Result<usize> {
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
