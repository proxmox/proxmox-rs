//! This contains traits used to implement methods to be added to the `Router`.

use serde_json::Value;

/// Method entries in a `Router` are actually just `&dyn ApiMethodInfo` trait objects.
/// This contains all the info required to call, document, or command-line-complete parameters for
/// a method.
pub trait ApiMethodInfo {
    fn description(&self) -> &'static str;
    fn parameters(&self) -> &'static [Parameter];
    fn return_type(&self) -> &'static TypeInfo;
    fn protected(&self) -> bool;
    fn reload_timezone(&self) -> bool;
    fn handler(&self) -> fn(Value) -> super::ApiFuture;
}

/// Shortcut to not having to type it out. This function signature is just a dummy and not yet
/// stabalized!
pub type CompleteFn = fn(&str) -> Vec<String>;

/// Provides information about a method's parameter. Every parameter has a name and must be
/// documented with a description, type information, and optional constraints.
pub struct Parameter {
    pub name: &'static str,
    pub description: &'static str,
    pub type_info: &'static TypeInfo,
}

/// Bare type info. Types themselves should also have a description, even if a method's parameter
/// usually overrides it. Ideally we can hyperlink the parameter to the type information in the
/// generated documentation.
pub struct TypeInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub complete_fn: Option<CompleteFn>,
}

/// Until we can slap `#[api]` onto all the functions we cann start translating our existing
/// `ApiMethod` structs to this new layout.
/// Otherwise this is mostly there so we can run the tests in the tests subdirectory without
/// depending on the api-macro crate. Tests using the macros belong into the api-macro crate itself
/// after all!
pub struct ApiMethod {
    pub description: &'static str,
    pub parameters: &'static [Parameter],
    pub return_type: &'static TypeInfo,
    pub protected: bool,
    pub reload_timezone: bool,
    pub handler: fn(Value) -> super::ApiFuture,
}

impl ApiMethodInfo for ApiMethod {
    fn description(&self) -> &'static str {
        self.description
    }

    fn parameters(&self) -> &'static [Parameter] {
        self.parameters
    }

    fn return_type(&self) -> &'static TypeInfo {
        self.return_type
    }

    fn protected(&self) -> bool {
        self.protected
    }

    fn reload_timezone(&self) -> bool {
        self.reload_timezone
    }

    fn handler(&self) -> fn(Value) -> super::ApiFuture {
        self.handler
    }
}
