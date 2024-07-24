//! Daemon and related state handling.

pub mod command_socket;

mod state;
pub use state::fail_on_shutdown;
pub use state::shutdown_future;
pub use state::{catch_reload_signal, reload_signal_task};
pub use state::{catch_shutdown_signal, shutdown_signal_task};
pub use state::{is_reload_requested, is_shutdown_requested, request_reload, request_shutdown};

pub mod server;
