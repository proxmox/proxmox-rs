//! This is a general utility crate used by all our rust projects.

pub mod io;
pub mod vec;

/// Evaluates to the offset (in bytes) of a given member within a struct
#[macro_export]
macro_rules! offsetof {
    ($ty:ty, $field:ident) => {
        unsafe { &(*(0 as *const $ty)).$field as *const _ as usize }
    }
}
