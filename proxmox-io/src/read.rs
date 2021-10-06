//! Helpers for `Read`.

use std::io;
use std::mem;

use endian_trait::Endian;

use crate::vec::{self, ByteVecExt};

/// Adds some additional related functionality for types implementing [`Read`](std::io::Read).
///
/// Particularly for reading into a newly allocated buffer, appending to a `Vec<u8>` or reading
/// values of a specific endianess (types implementing [`Endian`]).
///
/// Examples:
/// ```no_run
/// use proxmox_io::ReadExt;
///
/// # fn code() -> std::io::Result<()> {
/// let mut file = std::fs::File::open("some.data")?;
///
/// // read some bytes into a newly allocated Vec<u8>:
/// let mut data = file.read_exact_allocated(64)?;
///
/// // appending data to a vector:
/// let actually_appended = file.append_to_vec(&mut data, 64)?; // .read() version
/// file.append_exact_to_vec(&mut data, 64)?; // .read_exact() version
/// # Ok(())
/// # }
/// ```
///
/// Or for reading values of a defined representation and endianess:
///
/// ```no_run
/// # use endian_trait::Endian;
/// # use proxmox_io::ReadExt;
///
/// #[derive(Endian)]
/// #[repr(C)]
/// struct Header {
///     version: u16,
///     data_size: u16,
/// }
///
/// # fn code(mut file: std::fs::File) -> std::io::Result<()> {
/// // We have given `Header` a proper binary representation via `#[repr]`, so this is safe:
/// let header: Header = unsafe { file.read_le_value()? };
/// let mut blob = file.read_exact_allocated(header.data_size as usize)?;
/// # Ok(())
/// # }
/// ```
///
/// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
pub trait ReadExt {
    /// Read data into a newly allocated vector. This is a shortcut for:
    /// ```ignore
    /// let mut data = Vec::with_capacity(len);
    /// unsafe {
    ///     data.set_len(len);
    /// }
    /// reader.read_exact(&mut data)?;
    /// ```
    ///
    /// With this trait, we just use:
    /// ```no_run
    /// use proxmox_io::ReadExt;
    /// # fn code(mut reader: std::fs::File, len: usize) -> std::io::Result<()> {
    /// let data = reader.read_exact_allocated(len)?;
    /// # Ok(())
    /// # }
    /// ```
    fn read_exact_allocated(&mut self, size: usize) -> io::Result<Vec<u8>>;

    /// Append data to a vector, growing it as necessary. Returns the amount of data appended.
    fn append_to_vec(&mut self, out: &mut Vec<u8>, size: usize) -> io::Result<usize>;

    /// Append an exact amount of data to a vector, growing it as necessary.
    fn append_exact_to_vec(&mut self, out: &mut Vec<u8>, size: usize) -> io::Result<()>;

    /// Read a value with host endianess.
    ///
    /// This is limited to types implementing the [`Endian`] trait under the assumption that
    /// this is only done for types which are supposed to be read/writable directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox_io::ReadExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::open("my-raw.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// let data: Data = unsafe { file.read_host_value()? };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Safety
    ///
    /// This should only used for types with a defined storage representation, usually
    /// `#[repr(C)]`, otherwise the results may be inconsistent.
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn read_host_value<T: Endian>(&mut self) -> io::Result<T>;

    /// Read a little endian value.
    ///
    /// The return type is required to implement the [`Endian`] trait, and we make the
    /// assumption that this is only done for types which are supposed to be read/writable
    /// directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox_io::ReadExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::open("my-little-endian.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// let data: Data = unsafe { file.read_le_value()? };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Safety
    ///
    /// This should only used for types with a defined storage representation, usually
    /// `#[repr(C)]`, otherwise the results may be inconsistent.
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn read_le_value<T: Endian>(&mut self) -> io::Result<T>;

    /// Read a big endian value.
    ///
    /// The return type is required to implement the [`Endian`] trait, and we make the
    /// assumption that this is only done for types which are supposed to be read/writable
    /// directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox_io::ReadExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::open("my-big-endian.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// let data: Data = unsafe { file.read_be_value()? };
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Safety
    ///
    /// This should only used for types with a defined storage representation, usually
    /// `#[repr(C)]`, otherwise the results may be inconsistent.
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn read_be_value<T: Endian>(&mut self) -> io::Result<T>;

    /// Read a boxed value with host endianess.
    ///
    /// This is currently not limited to types implementing the [`Endian`] trait as in our use
    /// cases we use this for types which are too big to want to always perform endian swaps
    /// immediately on all values.
    ///
    /// ```
    /// # use proxmox_io::vec;
    /// use proxmox_io::ReadExt;
    ///
    /// #[repr(C)]
    /// struct Data {
    ///     v1: u64,
    ///     buf: [u8; 4088],
    /// }
    ///
    /// # let mut input = [0u8; 4096];
    /// # use proxmox_io::WriteExt;
    /// # let mut writer = &mut input[..];
    /// # unsafe { writer.write_host_value(32u64).unwrap() };
    /// # let mut file = &input[..];
    ///
    /// # fn code<T: std::io::Read>(mut file: T) -> std::io::Result<()> {
    /// let data: Box<Data> = unsafe { file.read_host_value_boxed()? };
    /// assert_eq!(data.v1, 32);
    /// # Ok(())
    /// # }
    /// # code(&input[..]).unwrap();
    /// ```
    ///
    /// # Safety
    ///
    /// This should only used for types with a defined storage representation, usually
    /// `#[repr(C)]`, otherwise the results may be inconsistent.
    unsafe fn read_host_value_boxed<T>(&mut self) -> io::Result<Box<T>>;

    /// Try to read the exact number of bytes required to fill buf.
    ///
    /// This function reads as many bytes as necessary to completely
    /// fill the specified buffer buf. If this function encounters an
    /// "end of file" before getting any data, it returns Ok(false).
    /// If there is some data, but not enough, it return an error of
    /// the kind ErrorKind::UnexpectedEof. The contents of buf are
    /// unspecified in this case.
    fn read_exact_or_eof(&mut self, buf: &mut [u8]) -> io::Result<bool>;

    /// Read until EOF
    fn skip_to_end(&mut self) -> io::Result<usize>;
}

impl<R: io::Read> ReadExt for R {
    fn read_exact_allocated(&mut self, size: usize) -> io::Result<Vec<u8>> {
        let mut out = unsafe { vec::uninitialized(size) };
        self.read_exact(&mut out)?;
        Ok(out)
    }

    fn append_to_vec(&mut self, out: &mut Vec<u8>, size: usize) -> io::Result<usize> {
        let pos = out.len();
        unsafe {
            out.grow_uninitialized(size);
        }
        let got = self.read(&mut out[pos..])?;
        unsafe {
            out.set_len(pos + got);
        }
        Ok(got)
    }

    fn append_exact_to_vec(&mut self, out: &mut Vec<u8>, size: usize) -> io::Result<()> {
        let pos = out.len();
        unsafe {
            out.grow_uninitialized(size);
        }
        self.read_exact(&mut out[pos..])?;
        Ok(())
    }

    unsafe fn read_host_value<T: Endian>(&mut self) -> io::Result<T> {
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        self.read_exact(std::slice::from_raw_parts_mut(
            value.as_mut_ptr() as *mut u8,
            mem::size_of::<T>(),
        ))?;
        Ok(value.assume_init())
    }

    unsafe fn read_le_value<T: Endian>(&mut self) -> io::Result<T> {
        Ok(self.read_host_value::<T>()?.from_le())
    }

    unsafe fn read_be_value<T: Endian>(&mut self) -> io::Result<T> {
        Ok(self.read_host_value::<T>()?.from_be())
    }

    unsafe fn read_host_value_boxed<T>(&mut self) -> io::Result<Box<T>> {
        // FIXME: Change this once #![feature(new_uninit)] lands for Box<T>!

        let ptr = std::alloc::alloc(std::alloc::Layout::new::<T>()) as *mut T;
        self.read_exact(std::slice::from_raw_parts_mut(
            ptr as *mut u8,
            mem::size_of::<T>(),
        ))?;
        Ok(Box::from_raw(ptr))
    }

    fn read_exact_or_eof(&mut self, mut buf: &mut [u8]) -> io::Result<bool> {
        let mut read_bytes = 0;
        loop {
            match self.read(&mut buf) {
                Ok(0) => {
                    if read_bytes == 0 {
                        return Ok(false);
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "failed to fill whole buffer",
                    ));
                }
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    read_bytes += n;
                    if buf.is_empty() {
                        return Ok(true);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    fn skip_to_end(&mut self) -> io::Result<usize> {
        let mut skipped_bytes = 0;
        let mut buf = unsafe { vec::uninitialized(32 * 1024) };
        loop {
            match self.read(&mut buf) {
                Ok(0) => return Ok(skipped_bytes),
                Ok(n) => skipped_bytes += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}
