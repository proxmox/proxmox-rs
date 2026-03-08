#![allow(clippy::manual_range_contains)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! Utilities for parsing and manipulating time-related types.
//!
//! This crate provides several time-related abstractions:
//!
//! - [`TimeSpan`] — durations with human-readable parsing and display
//! - [`CalendarEvent`] — recurring time specifications inspired by systemd.time
//! - [`DailyDuration`] — time-of-day windows with optional weekday constraints
//! - [`WeekDays`] — bitflag set representing days of the week
//!
//! On non-WASM targets, additional POSIX time helpers are available:
//!
//! - [`TmEditor`] — safe wrapper around `libc::tm` for date/time manipulation
//! - [`epoch_i64`], [`epoch_f64`] — current Unix epoch
//! - [`epoch_to_rfc3339`], [`epoch_to_rfc3339_utc`], [`epoch_to_rfc2822`] — epoch formatting
//! - [`parse_rfc3339`] — RFC 3339 string to epoch
//! - [`strftime`], [`strftime_l`] — safe `strftime` bindings

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
