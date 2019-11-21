use std::any::Any;

/// An easy way to convert types to Any
///
/// Mostly useful to downcast trait objects (see RpcEnvironment).
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
