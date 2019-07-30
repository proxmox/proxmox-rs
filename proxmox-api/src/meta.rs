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

pub trait AsOptionStr {
    fn as_option_str(&self) -> Option<&str>;
}

impl AsOptionStr for String {
    fn as_option_str(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl AsOptionStr for str {
    fn as_option_str(&self) -> Option<&str> {
        Some(self)
    }
}

impl AsOptionStr for Option<String> {
    fn as_option_str(&self) -> Option<&str> {
        self.as_ref().map(String::as_str)
    }
}

impl AsOptionStr for Option<&str> {
    fn as_option_str(&self) -> Option<&str> {
        *self
    }
}
