//! Module providing I/O helpers (sync and async).
//!
//! The [`ops`](io::ops) module provides helper traits for types implementing [`Read`](std::io::Read).
//!
//! The top level functions in of this module here are used for standalone implementations of
//! various functionality which is actually intended to be available as methods to types
//! implementing `AsyncRead`, which, however, without async/await cannot be methods due to them
//! having non-static lifetimes in that case.

pub mod ops;
