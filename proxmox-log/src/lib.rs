#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unsafe_op_in_unsafe_fn)]

use std::env;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::task::futures::TaskLocalFuture;
use tracing::Level;
use tracing_log::{AsLog, LogTracer};
use tracing_subscriber::filter::{filter_fn, LevelFilter};
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

tokio::task_local! {
    static LOG_CONTEXT: LogContext;
}

pub fn init_logger(
    env_var_name: &str,
    default_log_level: LevelFilter,
    _application_name: &str,
) -> Result<(), anyhow::Error> {
    let mut log_level = default_log_level;
    if let Ok(v) = env::var(env_var_name) {
        if let Ok(l) = v.parse::<LevelFilter>() {
            log_level = l;
        }
    }
    let registry = tracing_subscriber::registry()
        .with(
            tracing_journald::layer()
                .expect("Unable to open syslog")
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
