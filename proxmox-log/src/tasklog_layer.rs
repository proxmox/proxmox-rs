use std::fmt::Write as _;

use tracing::field::Field;
use tracing::field::Visit;
use tracing::Event;
use tracing::Level;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::{FileLogState, LogContext};

pub struct TasklogLayer;

impl<S: Subscriber> Layer<S> for TasklogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if let Some(ctx) = LogContext::current() {
            let mut logger = ctx.logger.lock().unwrap();
            let mut buf = String::new();
            event.record(&mut EventVisitor::new(&mut buf));
            let level = event.metadata().level();
            log_to_file(&mut logger, level, &buf);
        }
    }
}

fn log_to_file(logger: &mut FileLogState, level: &Level, buf: &String) {
    match *level {
        Level::ERROR | Level::WARN => {
            logger.warn_count += 1;
            logger.logger.log(buf);
        }
        Level::INFO => logger.logger.log(buf),
        Level::DEBUG => logger.logger.log(format!("DEBUG: {buf}")),
        Level::TRACE => logger.logger.log(format!("TRACE: {buf}")),
    };
}

struct EventVisitor<'a> {
    buf: &'a mut String,
}

impl<'a> EventVisitor<'a> {
    fn new(buf: &'a mut String) -> Self {
        Self { buf }
    }
}

impl Visit for EventVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.buf, "{value:?}");
        }
    }
}
