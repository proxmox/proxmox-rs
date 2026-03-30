#[macro_use]
pub mod topsis;

pub mod node;
pub mod resource;
pub mod usage;

pub mod scheduler;

// pve_static exists only for backwards compatibility to not break builds
// The allow(deprecated) is to not report its own use of deprecated items
#[allow(deprecated)]
pub mod pve_static;
