#[cfg(feature = "gotify")]
pub mod gotify;
#[cfg(feature = "sendmail")]
pub mod sendmail;
#[cfg(feature = "smtp")]
pub mod smtp;
#[cfg(feature = "webhook")]
pub mod webhook;

mod common;
