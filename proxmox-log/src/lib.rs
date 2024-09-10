#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]

use std::env;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::task::futures::TaskLocalFuture;
use tracing_log::{AsLog, LogTracer};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::prelude::*;

use tasklog_layer::TasklogLayer;

mod file_logger;
pub use file_logger::{FileLogOptions, FileLogger};

mod tasklog_layer;

pub use tracing::debug;
pub use tracing::debug_span;
pub use tracing::enabled;
pub use tracing::error;
pub use tracing::error_span;
pub use tracing::event;
pub use tracing::info;
pub use tracing::info_span;
pub use tracing::span;
pub use tracing::span_enabled;
pub use tracing::trace;
pub use tracing::trace_span;
pub use tracing::warn;
pub use tracing::warn_span;
pub use tracing::Level;
pub use tracing_subscriber::filter::LevelFilter;

tokio::task_local! {
    static LOG_CONTEXT: LogContext;
}

pub fn init_logger(
    env_var_name: &str,
    default_log_level: LevelFilter,
) -> Result<(), anyhow::Error> {
    let mut log_level = default_log_level;
    if let Ok(v) = env::var(env_var_name) {
        match v.parse::<LevelFilter>() {
            Ok(l) => {
                log_level = l;
            }
            Err(e) => {
                eprintln!("env variable {env_var_name} found, but parsing failed: {e:?}");
            }
        }
    }
    let registry = tracing_subscriber::registry()
        .with(
            journald_or_stderr_layer()
                .with_filter(filter_fn(|metadata| {
                    !LogContext::exists() || *metadata.level() >= Level::ERROR
                }))
                .with_filter(log_level),
        )
        .with(TasklogLayer {}.with_filter(log_level));

    tracing::subscriber::set_global_default(registry)?;
    LogTracer::init_with_filter(log_level.as_log())?;
    Ok(())
}

/// A file logger and warnings counter which can be used across a scope for separate logging.
/// Mainly used for worker-task logging.
pub struct FileLogState {
    pub warn_count: u64,
    pub logger: FileLogger,
}

impl FileLogState {
    fn new(logger: FileLogger) -> Self {
        Self {
            warn_count: 0,
            logger,
        }
    }
}

/// A log context can be set for a sync or asynchronous scope to cause log messages to be added to
/// a [`FileLogger`].
#[derive(Clone)]
pub struct LogContext {
    logger: Arc<Mutex<FileLogState>>,
}

impl LogContext {
    /// Create a logging context for a [`FileLogger`].
    pub fn new(logger: FileLogger) -> Self {
        Self {
            logger: Arc::new(Mutex::new(FileLogState::new(logger))),
        }
    }

    /// Check to see if a log context exists without getting a strong reference to it.
    pub fn exists() -> bool {
        LOG_CONTEXT.try_with(|_| ()).is_ok()
    }

    /// Get the current logging context if set.
    pub fn current() -> Option<Self> {
        LOG_CONTEXT.try_with(|ctx| ctx.clone()).ok()
    }

    /// Run a task with this log context.
    pub fn sync_scope<F, R>(self, func: F) -> R
    where
        F: FnOnce() -> R,
    {
        LOG_CONTEXT.sync_scope(self, func)
    }

    /// Run a task with this log context.
    pub fn scope<F>(self, f: F) -> TaskLocalFuture<Self, F>
    where
        F: Future,
    {
        LOG_CONTEXT.scope(self, f)
    }

    /// Access the internal state.
    pub fn state(&self) -> &Arc<Mutex<FileLogState>> {
        &self.logger
    }
}

fn journald_or_stderr_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    match tracing_journald::layer() {
        Ok(layer) => layer.boxed(),
        Err(err) => {
            eprintln!("Unable to open syslog: {err:?}");
            plain_stderr_layer().boxed()
        }
    }
}

fn plain_stderr_layer<S>() -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber,
    S: for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let format = tracing_subscriber::fmt::format()
        .with_level(false)
        .without_time()
        .with_target(false)
        .compact();
    tracing_subscriber::fmt::layer()
        .event_format(format)
        .with_writer(std::io::stderr)
}

/// Initialize default logger for CLI binaries
pub fn init_cli_logger(
    env_var_name: &str,
    default_log_level: LevelFilter,
) -> Result<(), anyhow::Error> {
    let mut log_level = default_log_level;
    if let Ok(v) = env::var(env_var_name) {
        match v.parse::<LevelFilter>() {
            Ok(l) => {
                log_level = l;
            }
            Err(e) => {
                eprintln!("env variable {env_var_name} found, but parsing failed: {e:?}");
            }
        }
    }

    let registry = tracing_subscriber::registry()
        .with(
            plain_stderr_layer()
                .with_filter(filter_fn(|metadata| {
                    !LogContext::exists() || *metadata.level() >= Level::ERROR
                }))
                .with_filter(log_level),
        )
        .with(TasklogLayer {}.with_filter(log_level));

    tracing::subscriber::set_global_default(registry)?;
    LogTracer::init_with_filter(log_level.as_log())?;
    Ok(())
}
