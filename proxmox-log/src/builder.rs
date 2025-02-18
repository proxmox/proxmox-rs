use tracing::{level_filters::LevelFilter, Level};
use tracing_log::{AsLog, LogTracer};
use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer};

use crate::{
    get_env_variable, journald_or_stderr_layer, plain_stderr_layer,
    tasklog_layer::TasklogLayer, LogContext,
};

/// Builder-like struct to compose your logging layers.
///
/// Stores a global log level which will also be applied to all layers. The different layers can be
/// added with the builder methods. Note that the init method can only be called once.
///
/// # Examples
///
/// ```
/// // The default PBS daemon/proxy logger
/// Logger::from_env("PBS_LOG", LevelFilter::INFO)
///     .journald_on_no_workertask()
///     .tasklog_pbs()
///     .init()?;
/// ```
///
/// ```
/// // The default PBS cli logger
/// Logger::from_env("PBS_LOG", LevelFilter::INFO)
///     .stderr()
///     .init()?;
/// ```
pub struct Logger {
    global_log_level: LevelFilter,
    layer: Vec<
        Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync + 'static>,
    >,
}

impl Logger {
    /// Create a new LogBuilder with no layers and a default loglevel retrieved from an env
    /// variable. If the env variable cannot be retrieved or the content is not parsable, fallback
    /// to the default_log_level passed.
    pub fn from_env(env_var: &str, default_log_level: LevelFilter) -> Logger {
        let log_level = get_env_variable(env_var, default_log_level);
        Logger {
            global_log_level: log_level,
            layer: vec![],
        }
    }

    /// Print to journald.
    ///
    /// If the journal cannot be opened, print to stderr instead.
    pub fn journald(mut self) -> Logger {
        self.layer.push(
            journald_or_stderr_layer()
                .with_filter(self.global_log_level)
                .boxed(),
        );
        self
    }

    /// Print to journald if no LogContext (we're not in a PBS workertask) is given.
    ///
    /// If opening the journal fails, we fallback and print to stderr. We print to journald if
    /// no LogContext exists – which means we are not in a PBS workertask – or the level of the
    /// log message is 'ERROR'.
    pub fn journald_on_no_workertask(mut self) -> Logger {
        self.layer.push(
            journald_or_stderr_layer()
                .with_filter(filter_fn(|metadata| {
                    !LogContext::exists() || *metadata.level() == Level::ERROR
                }))
                .with_filter(self.global_log_level)
                .boxed(),
        );
        self
    }

    /// Print to the PBS tasklog if we are in a PBS workertask.
    ///
    /// Check if a LogContext exists and if it does, print to the corresponding task log file.
    pub fn tasklog_pbs(mut self) -> Logger {
        self.layer
            .push(TasklogLayer {}.with_filter(self.global_log_level).boxed());
        self
    }

    /// Print to stderr.
    ///
    /// Prints all the events to stderr with the compact format (no level, no timestamp).
    pub fn stderr(mut self) -> Logger {
        self.layer.push(
            plain_stderr_layer()
                .with_filter(self.global_log_level)
                .boxed(),
        );
        self
    }

    /// Print to stderr if no workertask exists or the event level is `ERROR`.
    ///
    /// Print to stderr in the default compact format (no level, no timestamp). This will only be
    /// triggered if no workertask could be found (no LogContext exists) or the event level is
    /// `ERROR`.
    pub fn stderr_on_no_workertask(mut self) -> Logger {
        self.layer.push(
            plain_stderr_layer()
                .with_filter(filter_fn(|metadata| {
                    !LogContext::exists() || *metadata.level() == Level::ERROR
                }))
                .with_filter(self.global_log_level)
                .boxed(),
        );
        self
    }

    /// Inits the tracing logger with the previously configured layers.
    ///
    /// Also configures the `LogTracer` which will convert all `log` events to tracing events.
    pub fn init(self) -> Result<(), anyhow::Error> {
        let registry = tracing_subscriber::registry().with(self.layer);
        tracing::subscriber::set_global_default(registry)?;

        LogTracer::init_with_filter(self.global_log_level.as_log())?;
        Ok(())
    }
}
