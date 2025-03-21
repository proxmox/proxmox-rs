//! # Proxmox REST server
//!
//! This module provides convenient building blocks to implement a
//! REST server.
//!
//! ## Features
//!
//! * highly threaded code, uses Rust async
//! * static API definitions using schemas
//! * support for long running worker tasks (threads or async tokio tasks)
//! * supports separate access and authentication log files
//! * extra control socket to trigger management operations
//!   - logfile rotation
//!   - worker task management
//! * generic interface to authenticate user

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::fmt;
use std::sync::LazyLock;

use anyhow::{format_err, Error};
use nix::unistd::Pid;

use proxmox_sys::fs::CreateOptions;
use proxmox_sys::linux::procfs::PidStat;

mod compression;
pub use compression::*;

pub mod formatter;

mod environment;
pub use environment::*;

mod api_config;
pub use api_config::{ApiConfig, AuthError, AuthHandler, IndexHandler};

mod rest;
pub use rest::{Redirector, RestServer};

pub mod connection;

mod worker_task;
pub use worker_task::*;

mod h2service;
pub use h2service::*;

static PID: LazyLock<i32> = LazyLock::new(|| unsafe { libc::getpid() });
static PSTART: LazyLock<u64> = LazyLock::new(|| {
    PidStat::read_from_pid(Pid::from_raw(*PID))
        .unwrap()
        .starttime
});

/// Returns the current process ID (see [libc::getpid])
///
/// The value is cached at startup (so it is invalid after a fork)
pub(crate) fn pid() -> i32 {
    *PID
}

/// Returns the starttime of the process (see [PidStat])
///
/// The value is cached at startup (so it is invalid after a fork)
pub(crate) fn pstart() -> u64 {
    *PSTART
}

/// Helper to write the PID into a file
pub fn write_pid(pid_fn: &str) -> Result<(), Error> {
    let pid_str = format!("{}\n", *PID);
    proxmox_sys::fs::replace_file(pid_fn, pid_str.as_bytes(), CreateOptions::new(), false)
}

/// Helper to read the PID from a file
pub fn read_pid(pid_fn: &str) -> Result<i32, Error> {
    let pid = proxmox_sys::fs::file_get_contents(pid_fn)?;
    let pid = std::str::from_utf8(&pid)?.trim();
    pid.parse()
        .map_err(|err| format_err!("could not parse pid - {}", err))
}

/// Extract a specific cookie from cookie header.
/// We assume cookie_name is already url encoded.
pub fn extract_cookie(cookie: &str, cookie_name: &str) -> Option<String> {
    for pair in cookie.split(';') {
        let (name, value) = match pair.find('=') {
            Some(i) => (pair[..i].trim(), pair[(i + 1)..].trim()),
            None => return None, // Cookie format error
        };

        if name == cookie_name {
            use percent_encoding::percent_decode;
            if let Ok(value) = percent_decode(value.as_bytes()).decode_utf8() {
                return Some(value.into());
            } else {
                return None; // Cookie format error
            }
        }
    }

    None
}

/// Extract a specific cookie from a HeaderMap's "COOKIE" entry.
/// We assume cookie_name is already url encoded.
pub fn cookie_from_header(headers: &http::HeaderMap, cookie_name: &str) -> Option<String> {
    if let Some(Ok(cookie)) = headers.get("COOKIE").map(|v| v.to_str()) {
        extract_cookie(cookie, cookie_name)
    } else {
        None
    }
}

/// normalize uri path
///
/// Do not allow ".", "..", or hidden files ".XXXX"
/// Also remove empty path components
pub fn normalize_path_with_components(
    path: &str,
) -> Result<(String, Vec<&str>), IllegalPathComponents> {
    let items = path.split('/');

    let mut path = String::new();
    let mut components = vec![];

    for name in items {
        if name.is_empty() {
            continue;
        }
        if name.starts_with('.') {
            return Err(IllegalPathComponents);
        }
        path.push('/');
        path.push_str(name);
        components.push(name);
    }

    Ok((path, components))
}

#[derive(Debug)]
pub struct IllegalPathComponents;

impl std::error::Error for IllegalPathComponents {}

impl fmt::Display for IllegalPathComponents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("path contains illegal components")
    }
}

/// Normalize a uri path by stripping empty components.
/// Components starting with a '.' are illegal.
pub fn normalize_path(path: &str) -> Result<String, IllegalPathComponents> {
    let mut output = String::with_capacity(path.len());
    for item in path.split('/') {
        if item.is_empty() {
            continue;
        }

        if item.starts_with('.') {
            return Err(IllegalPathComponents);
        }

        output.push('/');
        output.push_str(item);
    }
    Ok(output)
}
