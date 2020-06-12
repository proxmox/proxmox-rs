//! Memory mapping helpers.

use std::convert::TryFrom;
use std::os::unix::io::RawFd;
use std::{io, mem, ptr};

use nix::sys::mman;

use crate::sys::error::{io_err_other, SysError};

pub struct Mmap<T> {
    data: *mut T,
    len: usize,
}

unsafe impl<T> Send for Mmap<T> where T: Send {}
unsafe impl<T> Sync for Mmap<T> where T: Sync {}

impl<T> Mmap<T> {
    pub unsafe fn map_fd(
        fd: RawFd,
        ofs: u64,
        count: usize,
        prot: mman::ProtFlags,
        flags: mman::MapFlags,
    ) -> io::Result<Self> {
        let byte_len = count * mem::size_of::<T>();
        let data = mman::mmap(
            ptr::null_mut(),
            libc::size_t::try_from(byte_len).map_err(io_err_other)?,
            prot,
            flags,
            fd,
            libc::off_t::try_from(ofs).map_err(io_err_other)?,
        )
        .map_err(SysError::into_io_error)?;

        Ok(Self {
            data: data as *mut T,
            len: count,
        })
    }
}

impl<T> std::ops::Deref for Mmap<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.data, self.len) }
    }
}

impl<T> std::ops::DerefMut for Mmap<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.data, self.len) }
    }
}

impl<T> Drop for Mmap<T> {
    fn drop(&mut self) {
        unsafe {
            // In theory this can fail if too many memory mappings are already present and
            // unmapping a smaller region inside a bigger one, causing it to become split into 2
            // regions. But then we have bigger problems already anyway, so we'll just ignore this.
            let _ = mman::munmap(self.data as *mut libc::c_void, self.len * mem::size_of::<T>());
        }
    }
}

impl<'a, T> IntoIterator for &'a Mmap<T> {
    type Item = &'a T;
    type IntoIter = <&'a [T] as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        <&'a [T] as IntoIterator>::into_iter(self)
    }
}
