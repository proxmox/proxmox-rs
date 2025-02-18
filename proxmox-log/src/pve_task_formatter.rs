use std::fmt;
use tracing::{Event, Subscriber};
use tracing_subscriber::field::VisitOutput;
use tracing_subscriber::fmt::format::{DefaultVisitor, Writer};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// This custom formatter outputs logs as they are visible in the PVE task log.
///
/// e.g.: "DEBUG: sample message"
pub struct PveTaskFormatter {}

impl<C, N> FormatEvent<C, N> for PveTaskFormatter
where
    C: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, C, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        write!(writer, "{}: ", event.metadata().level().as_str())?;

        let mut v = DefaultVisitor::new(writer.by_ref(), true);
        event.record(&mut v);
        v.finish()?;
        writer.write_char('\n')
    }
}
