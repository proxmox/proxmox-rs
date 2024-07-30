//! Byte vector helpers.
//!
//! We have a lot of I/O code such as:
//! ```ignore
//! let mut buffer = vec![0u8; header_size];
//! file.read_exact(&mut buffer)?;
//! ```
//! (We even have this case with a 4M buffer!)
//!
//! This needlessly initializes the buffer to zero (which not only wastes time (an insane amount of
//! time on debug builds, actually) but also prevents tools such as valgrind from pointing out
//! access to actually uninitialized data, which may hide bugs...)
//!
//! This module provides some helpers for this kind of code. Many of these are supposed to stay on
//! a lower level, with I/O helpers for types implementing [`Read`](std::io::Read) being available
//! in this module.
//!
//! Examples:
//! ```no_run
//! use proxmox_io::vec::{self, ByteVecExt};
//!
//! # let size = 64usize;
//! # let more = 64usize;
//! let mut buffer = vec::undefined(size); // A zero-initialized buffer
//!
//! let mut buffer = unsafe { vec::uninitialized(size) }; // an actually uninitialized buffer
//! vec::clear(&mut buffer); // zero out an &mut [u8]
//!
//! vec::clear(unsafe {
//!     buffer.grow_uninitialized(more) // grow the buffer with uninitialized bytes
//! });
//! ```

mod byte_vec;
pub use byte_vec::ByteVecExt;

/// Create an uninitialized byte vector of a specific size.
///
/// This is just a shortcut for:
/// ```no_run
/// # let len = 64usize;
/// let mut v = Vec::<u8>::with_capacity(len);
/// unsafe {
///     v.set_len(len);
/// }
/// ```
///
/// # Safety
///
/// It's generally not unsafe to use this method, but the contents are uninitialized, and since
/// this does not return a `MaybeUninit` type to track the initialization state, this is simply
/// marked as unsafe for good measure.
#[inline]
pub unsafe fn uninitialized(len: usize) -> Vec<u8> {
    unsafe {
        let data = std::alloc::alloc(std::alloc::Layout::array::<u8>(len).unwrap());
        Vec::from_raw_parts(data, len, len)
    }
}

/// Shortcut to zero out a slice of bytes.
#[inline]
pub fn clear(data: &mut [u8]) {
    unsafe {
        std::ptr::write_bytes(data.as_mut_ptr(), 0, data.len());
    }
}

/// Create a newly allocated, zero initialized byte vector.
#[inline]
pub fn zeroed(len: usize) -> Vec<u8> {
    unsafe {
        let mut out = uninitialized(len);
        clear(&mut out);
        out
    }
}

/// Create a newly allocated byte vector of a specific size with "undefined" content.
///
/// The data will be zero initialized, but this function is meant to at some point gain support for
/// marking the data as uninitialized for tools such as `valgrind` at some point.
#[inline]
pub fn undefined(len: usize) -> Vec<u8> {
    zeroed(len)
}
