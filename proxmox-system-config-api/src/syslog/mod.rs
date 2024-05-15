mod api_types;
pub use api_types::*;


#[cfg(feature = "syslog-impl")]
mod journal;
#[cfg(feature = "syslog-impl")]
pub use journal::dump_journal;
