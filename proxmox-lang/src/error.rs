//! A set of macros/helpers for I/O handling. These provide for `std::io::Error` what `anyhow` provides
//! for `anyhow::Error.`

use std::io;

/// Helper to convert non-system-errors into an `io::Error` or `io::ErrorKind::Other`.
///
/// A more convenient way is to use the `io_format_err!` macro.
pub fn io_err_other<E: ToString>(e: E) -> io::Error {
    io::Error::new(std::io::ErrorKind::Other, e.to_string())
}


/// Like anyhow's `format_err` but producing a `std::io::Error`.
#[macro_export]
macro_rules! io_format_err {
    ($($msg:tt)+) => {
        ::std::io::Error::new(::std::io::ErrorKind::Other, format!($($msg)+))
    };
}

/// Shortcut to return an `io::Error::last_os_error`.
///
/// This is effectively `return Err(::std::io::Error::last_os_error().into());`.
#[macro_export]
macro_rules! io_bail_last {
    () => {{
        return Err(::std::io::Error::last_os_error().into());
    }};
}

/// Like anyhow's `bail` but producing a `std::io::Error`.
#[macro_export]
macro_rules! io_bail {
    ($($msg:tt)+) => {{
        return Err($crate::io_format_err!($($msg)+));
    }};
}

#[doc(hidden)]
/// Non-panicking assertion: shortcut for returning an `io::Error` if the condition is not met.
/// Essentially: `if !expr { io_bail_last!() }`.
///
/// Note that this uses `errno`, care must be taken not to overwrite it with different value as a
/// side effect.
#[macro_export]
macro_rules! io_assert {
    ($value:expr) => {
        if !$value {
            $crate::io_bail_last!();
        }
    };
}
