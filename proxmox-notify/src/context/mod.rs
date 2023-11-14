use std::fmt::Debug;
use std::sync::Mutex;

#[cfg(any(feature = "pve-context", feature = "pbs-context"))]
pub mod common;
#[cfg(feature = "pbs-context")]
pub mod pbs;
#[cfg(feature = "pve-context")]
pub mod pve;

/// Product-specific context
pub trait Context: Send + Sync + Debug {
    /// Look up a user's email address from users.cfg
    fn lookup_email_for_user(&self, user: &str) -> Option<String>;
    /// Default mail author for mail-based targets
    fn default_sendmail_author(&self) -> String;
    /// Default from address for sendmail-based targets
    fn default_sendmail_from(&self) -> String;
    /// Proxy configuration for the current node
    fn http_proxy_config(&self) -> Option<String>;
}

static CONTEXT: Mutex<Option<&'static dyn Context>> = Mutex::new(None);

/// Set the product-specific context
pub fn set_context(context: &'static dyn Context) {
    *CONTEXT.lock().unwrap() = Some(context);
}

/// Get product-specific context.
///
/// Panics if the context has not been set yet.
#[allow(unused)] // context is not used if all endpoint features are disabled
pub(crate) fn context() -> &'static dyn Context {
    (*CONTEXT.lock().unwrap()).expect("context for proxmox-notify has not been set yet")
}
