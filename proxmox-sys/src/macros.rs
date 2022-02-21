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
