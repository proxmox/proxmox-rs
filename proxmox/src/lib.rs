//! Proxmox "tools" package containing some generic tools along with the schema, API and CLI
//! helpers.

pub mod api;
pub mod sys;
pub mod tools;

/// An identity (nop) macro. Used by the `#[sortable]` proc macro.
#[cfg(feature = "sortable-macro")]
#[macro_export]
macro_rules! identity {
    ($($any:tt)*) => ($($any)*)
}

#[cfg(feature = "sortable-macro")]
pub use proxmox_sortable_macro::sortable;
