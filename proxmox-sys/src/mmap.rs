//! Memory mapping helpers.

use std::convert::TryFrom;
use std::mem::MaybeUninit;
use std::num::NonZeroUsize;
use std::os::fd::AsFd;
use std::ptr::NonNull;
use std::{io, mem};

use nix::sys::mman;

use proxmox_lang::io_format_err;

use crate::error::SysError;

pub struct Mmap<T> {
    data: NonNull<T>,
    len: usize,
}

unsafe impl<T> Send for Mmap<T> where T: Send {}
unsafe impl<T> Sync for Mmap<T> where T: Sync {}

impl<T> Mmap<T> {
    /// Map a file into memory.
    ///
    /// # Safety
    ///
    /// `fd` must refer to a valid file descriptor.
    pub unsafe fn map_fd<F: AsFd>(
        fd: F,
        ofs: u64,
        count: usize,
        prot: mman::ProtFlags,
        flags: mman::MapFlags,
    ) -> io::Result<Self> {
        let byte_len = NonZeroUsize::new(count * mem::size_of::<T>())
            .ok_or_else(|| io_format_err!("mapped length must not be zero"))?;

        // libc::size_t vs usize
        #[allow(clippy::useless_conversion)]
        let data = unsafe {
            mman::mmap(
                None,
                byte_len,
                prot,
                flags,
                fd,
                libc::off_t::try_from(ofs).map_err(io::Error::other)?,
            )
        }
        .map_err(SysError::into_io_error)?;

        Ok(Self {
            data: data.cast::<T>(),
            len: count,
        })
    }
}

impl<T> std::ops::Deref for Mmap<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        unsafe { NonNull::slice_from_raw_parts(self.data, self.len).as_ref() }
    }
}

impl<T> std::ops::DerefMut for Mmap<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { NonNull::slice_from_raw_parts(self.data, self.len).as_mut() }
    }
}

impl<T> Drop for Mmap<T> {
    fn drop(&mut self) {
        unsafe {
            // In theory this can fail if too many memory mappings are already present and
            // unmapping a smaller region inside a bigger one, causing it to become split into 2
            // regions. But then we have bigger problems already anyway, so we'll just ignore this.
            let _ = mman::munmap(
                self.data.cast::<core::ffi::c_void>(),
                self.len * mem::size_of::<T>(),
            );
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

impl<T> Mmap<MaybeUninit<T>> {
    /// Converts to `Mmap<T>`.
    ///
    /// # Safety
    ///
    /// It is up to the caller to ensure this is safe, see
    /// [`MaybeUninit::assume_init`](std::mem::MaybeUninit::assume_init).
    pub unsafe fn assume_init(self) -> Mmap<T> {
        let out = Mmap {
            data: self.data.cast::<T>(),
            len: self.len,
        };
        std::mem::forget(self);
        out
    }
}
