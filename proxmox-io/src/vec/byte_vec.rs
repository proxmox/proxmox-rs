//! This module provides additional operations for `Vec<u8>`.
//!
//! Example:
//! ```
//! # use std::io::Read;
//! use proxmox_io::vec::{self, ByteVecExt};
//!
//! fn append_1024_to_vec<T: Read>(mut input: T, buffer: &mut Vec<u8>) -> std::io::Result<()> {
//!     input.read_exact(unsafe { buffer.grow_uninitialized(1024) })
//! }
//! ```

/// Some additional byte vector operations useful for I/O code.
/// Example:
/// ```
/// # use std::io::Read;
/// # use proxmox_io::ReadExt;
/// use proxmox_io::vec::{self, ByteVecExt};
///
/// # fn code(mut file: std::fs::File, mut data: Vec<u8>) -> std::io::Result<()> {
/// file.read_exact(unsafe {
///     data.grow_uninitialized(1024)
/// })?;
/// # Ok(())
/// # }
/// ```
///
/// Note that this module also provides a safe alternative for the case where
/// `grow_uninitialized()` is directly followed by a `read_exact()` call via the [`ReadExt`]
/// trait:
/// ```ignore
/// file.append_to_vec(&mut data, 1024)?;
/// ```
///
/// [`ReadExt`]: crate::ReadExt
pub trait ByteVecExt {
    /// Grow a vector without initializing its elements. The difference to simply using `reserve`
    /// is that it also updates the actual length, making the newly allocated data part of the
    /// slice.
    ///
    /// This is a shortcut for:
    /// ```ignore
    /// vec.reserve(more);
    /// let total = vec.len() + more;
    /// unsafe {
    ///     vec.set_len(total);
    /// }
    /// ```
    ///
    /// This returns a mutable slice to the newly allocated space, so it can be used inline:
    /// ```
    /// # use std::io::Read;
    /// # use proxmox_io::vec::ByteVecExt;
    /// # fn test(mut file: std::fs::File, buffer: &mut Vec<u8>) -> std::io::Result<()> {
    /// file.read_exact(unsafe { buffer.grow_uninitialized(1024) })?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Although for the above case it is recommended to use the even shorter version from the
    /// [`ReadExt`] trait:
    /// ```ignore
    /// // use crate::tools::vec::ByteVecExt;
    /// file.append_to_vec(&mut buffer, 1024)?;
    /// ```
    ///
    /// # Safety
    ///
    /// When increasing the size, the new contents are uninitialized and have nothing to do with
    /// the previously contained content. Since we cannot track this state through the type system,
    /// this method is marked as an unsafe API for good measure.
    ///
    /// [`ReadExt`]: crate::ReadExt
    unsafe fn grow_uninitialized(&mut self, more: usize) -> &mut [u8];

    /// Resize a vector to a specific size without initializing its data. This is a shortcut for:
    /// ```ignore
    /// if new_size <= vec.len() {
    ///     vec.truncate(new_size);
    /// } else {
    ///     unsafe {
    ///         vec.grow_uninitialized(new_size - vec.len());
    ///     }
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// When increasing the size, the new contents are uninitialized and have nothing to do with
    /// the previously contained content. Since we cannot track this state through the type system,
    /// this method is marked as an unsafe API for good measure.
    unsafe fn resize_uninitialized(&mut self, total: usize);
}

impl ByteVecExt for Vec<u8> {
    unsafe fn grow_uninitialized(&mut self, more: usize) -> &mut [u8] {
        let old_len = self.len();
        self.reserve(more);
        let total = old_len + more;
        unsafe {
            self.set_len(total);
        }
        &mut self[old_len..]
    }

    unsafe fn resize_uninitialized(&mut self, new_size: usize) {
        if new_size <= self.len() {
            self.truncate(new_size);
        } else {
            unsafe {
                self.grow_uninitialized(new_size - self.len());
            }
        }
    }
}
