use std::{
    cell::{Cell, RefCell},
    env,
};

use tracing::Level;
use tracing_log::{AsLog, LogTracer};
use tracing_subscriber::filter::{filter_fn, LevelFilter};
use tracing_subscriber::prelude::*;

use tasklog_layer::TasklogLayer;

mod file_logger;
pub use file_logger::{FileLogOptions, FileLogger};

mod tasklog_layer;

tokio::task_local! {
    pub static LOGGER: RefCell<FileLogger>;
    pub static WARN_COUNTER: Cell<u64>;
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
                .with_filter(log_level)
                .with_filter(filter_fn(|metadata| {
                    LOGGER.try_with(|_| {}).is_err() || *metadata.level() == Level::ERROR
                })),
        )
        .with(TasklogLayer {}.with_filter(log_level));

    tracing::subscriber::set_global_default(registry)?;
    LogTracer::init_with_filter(log_level.as_log())?;
    Ok(())
}
