//! std::ops extensions

/// Modeled after the nightly `std::ops::ControlFlow`.
///
/// Will be removed with crate version 2.0.
#[derive(Clone, Copy, Debug, PartialEq)]
#[deprecated(since = "1.1", note = "use std::ops::ControlFlow")]
pub enum ControlFlow<B, C = ()> {
    Continue(C),
    Break(B),
}

#[allow(deprecated)]
impl<B> ControlFlow<B> {
    #[deprecated(since = "1.1", note = "use std::ops::ControlFlow")]
    pub const CONTINUE: ControlFlow<B, ()> = ControlFlow::Continue(());
}
