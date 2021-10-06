//! Rust language related helpers.
//!
//! This provides some macros for features which are not yet available in the language, or
//! sometimes also types from nightly `std` which are simple enough to do just haven't been
//! bikeshedded and stabilized in the standard library yet.

mod constnamedbitmap;

pub mod ops;

/// Macro to write error-handling blocks (like perl eval {})
///
/// #### Example:
/// ```
/// # use proxmox_lang::try_block;
/// # macro_rules! format_err {
/// #     ($($msg:tt)+) => { format!($($msg)+) }
/// # }
/// # macro_rules! bail {
/// #     ($($msg:tt)+) => { return Err(format_err!($($msg)+)); }
/// # }
/// # let some_condition = false;
/// let result = try_block!({
///     if (some_condition) {
///         bail!("some error");
///     }
///     Ok(())
/// })
/// .map_err(|e| format_err!("my try block returned an error - {}", e));
/// ```

#[macro_export]
macro_rules! try_block {
    { $($token:tt)* } => {{ (|| -> Result<_,_> { $($token)* })() }}
}

/// Statically assert the size of a type at compile time.
///
/// This should compile:
/// ```
/// # use proxmox_lang::static_assert_size;
/// #[repr(C)]
/// struct Stuff {
///     value: [u8; 32]
/// }
/// static_assert_size!(Stuff, 32);
/// ```
///
/// This should fail to compile:
/// ```compile_fail
/// # use proxmox_lang::static_assert_size;
/// #[repr(C)]
/// struct Stuff {
///     value: [u8; 32]
/// }
/// static_assert_size!(Stuff, 128);
/// ```
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: fn() -> () = || {
            let _ = ::std::mem::transmute::<[u8; $size], $ty>;
        };
    };
}

/// Evaluates to the offset (in bytes) of a given member within a struct
///
/// ```
/// # use proxmox_lang::offsetof;
///
/// #[repr(C)]
/// struct Stuff {
///     first: u32,
///     second: u32,
/// }
///
/// assert_eq!(offsetof!(Stuff, second), 4);
///
/// ```
// FIXME: With 1.56 we get `const transmute` which may help making this usable `const fn` as we can
// avoid dereferencing the raw pointer by transmuting `0usize` into a proper reference instead.
//
// So with 1.56, do this instead:
//
//     unsafe { &(std::mem::transmute::<_, &$ty>(0usize).$field) as *const _ as usize }
#[macro_export]
macro_rules! offsetof {
    ($ty:ty, $field:ident) => {
        unsafe { &(*(std::ptr::null::<$ty>())).$field as *const _ as usize }
    };
}
