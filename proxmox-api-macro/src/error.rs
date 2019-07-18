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

macro_rules! cbail {
    ($span:expr => $($msg:tt)*) => {
        return Err(::failure::Error::from(crate::error::CompileError {
            tokens: ::quote::quote_spanned! { $span => compile_error!($($msg)*); }.into()
        }))
    };
    ($span:expr, $($msg:tt)*) => { cbail!($span => $($msg)*) }
}
