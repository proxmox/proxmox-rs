#![allow(clippy::manual_range_contains)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(not(target_arch = "wasm32"))]
mod tm_editor;
#[cfg(not(target_arch = "wasm32"))]
pub use tm_editor::*;

pub(crate) mod parse_helpers;

pub(crate) mod date_time_value;

mod calendar_event;
pub use calendar_event::*;

mod time_span;
pub use time_span::*;

mod week_days;
pub use week_days::*;

mod daily_duration;
pub use daily_duration::*;

#[cfg(not(target_arch = "wasm32"))]
mod posix;
#[cfg(not(target_arch = "wasm32"))]
pub use posix::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(test)]
mod test;
