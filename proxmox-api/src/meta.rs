//! Type related meta information, mostly used by the macro code.

use crate::ApiType;

/// Helper trait for entries with a `default` value in their api type definition.
pub trait OrDefault {
    type Output;

    fn or_default(&self, def: &'static Self::Output) -> &Self::Output;
    fn set(&mut self, value: Self::Output);
}

impl<T> OrDefault for Option<T>
where
    T: ApiType,
{
    type Output = T;

    #[inline]
    fn or_default(&self, def: &'static Self::Output) -> &Self::Output {
        self.as_ref().unwrap_or(def)
    }

    #[inline]
    fn set(&mut self, value: Self::Output) {
        *self = Some(value);
    }
}
