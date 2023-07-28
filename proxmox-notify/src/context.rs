use std::fmt::Debug;

use once_cell::sync::OnceCell;

pub trait Context: Send + Sync + Debug {
    fn lookup_email_for_user(&self, user: &str) -> Option<String>;
    fn default_sendmail_author(&self) -> String;
    fn default_sendmail_from(&self) -> String;
    fn http_proxy_config(&self) -> Option<String>;
}

static CONTEXT: OnceCell<&'static dyn Context> = OnceCell::new();

pub fn set_context(context: &'static dyn Context) {
    CONTEXT.set(context).expect("context has already been set");
}

#[allow(unused)] // context is not used if all endpoint features are disabled
pub(crate) fn context() -> &'static dyn Context {
    *CONTEXT.get().expect("context has not been yet")
}
