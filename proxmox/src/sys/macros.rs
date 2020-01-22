//! A set of macros for I/O handling. These provide for `std::io::Error` what `failure` provides
//! for `failure::Error.`

/// Like failure's `format_err` but producing a `std::io::Error`.
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
    () => {
        return Err(::std::io::Error::last_os_error().into());
    };
}

/// Like failure's `bail` but producing a `std::io::Error`.
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

/// Roughly equivalent to `nix::Errno::result`. Turns a `-1` into an `io::Error`, while passing
/// other values through as `Ok(n)`.
#[macro_export]
macro_rules! c_result {
    ($call:expr) => {{
        let rc = $call;
        if rc == -1 {
            ::std::io::Result::Err(::std::io::Error::last_os_error())
        } else {
            ::std::io::Result::Ok(rc)
        }
    }};
}

/// Like the old `try!` macro, but for return values of extern `C` calls. This is equivalent to
/// doing `c_result!(expr)?` (note the question mark).
#[macro_export]
macro_rules! c_try {
    ($expr:expr) => {{
        $crate::c_result!($expr)?
    }};
}

/// Shortcut for generating an `&'static CStr`.
///
/// This takes a *string* (*not* a *byte-string*), appends a terminating zero, and calls
/// `CStr::from_bytes_with_nul_unchecked`.
///
/// Shortcut for:
/// ```no_run
/// let bytes = concat!("THE TEXT", "\0");
/// unsafe { ::std::ffi::CStr::from_bytes_with_nul_unchecked(bytes.as_bytes()) }
/// # ;
/// ```
#[macro_export]
macro_rules! c_str {
    ($data:expr) => {{
        let bytes = concat!($data, "\0");
        unsafe { ::std::ffi::CStr::from_bytes_with_nul_unchecked(bytes.as_bytes()) }
    }};
}
