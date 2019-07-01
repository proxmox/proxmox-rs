//! Helpers for `Write`.

use std::io;

use endian_trait::Endian;

/// Adds some additional related functionality for types implementing [`Write`](std::io::Write).
///
/// Particularly for writing values of a specific endianess (types implementing [`Endian`]).
///
/// Examples:
/// ```no_run
/// # use endian_trait::Endian;
/// # use proxmox::tools::io::WriteExt;
///
/// #[derive(Endian)]
/// #[repr(C)]
/// struct Header {
///     version: u16,
///     data_size: u16,
/// }
///
/// # fn code(mut file: std::fs::File) -> std::io::Result<()> {
/// let header = Header {
///     version: 1,
///     data_size: 16,
/// };
/// // We have given `Header` a proper binary representation via `#[repr]`, so this is safe:
/// unsafe {
///     file.write_le_value(header)?;
/// }
/// # Ok(())
/// # }
/// ```
///
/// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
pub trait WriteExt {
    /// Write a value with host endianess.
    ///
    /// This is limited to types implementing the [`Endian`] trait under the assumption that this
    /// is only done for types which are supposed to be read/writable directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// The underlying write call is `.write_all()`, so there are no partial writes.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox::tools::io::WriteExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::create("my-raw.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// unsafe {
    ///     file.write_host_value(Data {
    ///         value: 1,
    ///         count: 2,
    ///     })?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn write_host_value<T: Endian>(&mut self, value: T) -> io::Result<()>;

    /// Write a little endian value.
    ///
    /// The input type is required to implement the [`Endian`] trait, and we make the assumption
    /// that this is only done for types which are supposed to be read/writable directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// The underlying write call is `.write_all()`, so there are no partial writes.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox::tools::io::WriteExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::create("my-raw.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// unsafe {
    ///     file.write_le_value(Data {
    ///         value: 1,
    ///         count: 2,
    ///     })?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn write_le_value<T: Endian>(&mut self, value: T) -> io::Result<()>;

    /// Read a big endian value.
    ///
    /// The input type is required to implement the [`Endian`] trait, and we make the assumption
    /// that this is only done for types which are supposed to be read/writable directly.
    ///
    /// There's no way to directly depend on a type having a specific `#[repr(...)]`, therefore
    /// this is considered unsafe.
    ///
    /// The underlying write call is `.write_all()`, so there are no partial writes.
    ///
    /// ```no_run
    /// # use endian_trait::Endian;
    /// use proxmox::tools::io::WriteExt;
    ///
    /// #[derive(Endian)]
    /// #[repr(C, packed)]
    /// struct Data {
    ///     value: u16,
    ///     count: u32,
    /// }
    ///
    /// # fn code() -> std::io::Result<()> {
    /// let mut file = std::fs::File::create("my-raw.dat")?;
    /// // We know `Data` has a safe binary representation (#[repr(C, packed)]), so we can
    /// // safely use our helper:
    /// unsafe {
    ///     file.write_be_value(Data {
    ///         value: 1,
    ///         count: 2,
    ///     })?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Endian`]: https://docs.rs/endian_trait/0.6/endian_trait/trait.Endian.html
    unsafe fn write_be_value<T: Endian>(&mut self, value: T) -> io::Result<()>;
}

impl<W: io::Write> WriteExt for W {
    unsafe fn write_host_value<T: Endian>(&mut self, value: T) -> io::Result<()> {
        self.write_all(std::slice::from_raw_parts(
            &value as *const T as *const u8,
            std::mem::size_of::<T>(),
        ))
    }

    unsafe fn write_le_value<T: Endian>(&mut self, value: T) -> io::Result<()> {
        self.write_host_value::<T>(value.to_le())
    }

    unsafe fn write_be_value<T: Endian>(&mut self, value: T) -> io::Result<()> {
        self.write_host_value::<T>(value.to_be())
    }
}
