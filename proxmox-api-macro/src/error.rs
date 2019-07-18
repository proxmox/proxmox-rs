#[derive(Debug)]
pub struct CompileError {
    pub tokens: proc_macro::TokenStream,
}

unsafe impl Send for CompileError {}
unsafe impl Sync for CompileError {}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "generic compile error")
    }
}

impl std::error::Error for CompileError {}

macro_rules! c_format_err {
    ($span:expr => $($msg:tt)*) => {
        crate::error::CompileError {
            tokens: ::quote::quote_spanned! { $span => compile_error!($($msg)*); }.into()
        }
    };
    ($span:expr, $($msg:tt)*) => { c_format_err!($span => $($msg)*) }
}

macro_rules! c_bail {
    ($span:expr => $($msg:tt)*) => {
        return Err(c_format_err!($span => $($msg)*).into());
    };
    ($span:expr, $($msg:tt)*) => { c_bail!($span => $($msg)*) }
}
