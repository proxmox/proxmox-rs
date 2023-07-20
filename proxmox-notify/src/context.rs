use once_cell::sync::OnceCell;
use std::fmt::Debug;

pub trait Context: Send + Sync + Debug {
    fn lookup_email_for_user(&self, user: &str) -> Option<String>;
}

static CONTEXT: OnceCell<&'static dyn Context> = OnceCell::new();

pub fn set_context(context: &'static dyn Context) {
    CONTEXT.set(context).expect("context has already been set");
}

pub(crate) fn context() -> &'static dyn Context {
    *CONTEXT.get().expect("context has not been yet")
}
