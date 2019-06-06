//! This contains traits used to implement methods to be added to the `Router`.

use serde_json::Value;

pub trait ApiMethodInfo {
    fn description(&self) -> &'static str;
    fn parameters(&self) -> &'static [Parameter];
    fn return_type(&self) -> &'static TypeInfo;
    fn protected(&self) -> bool;
    fn reload_timezone(&self) -> bool;
    fn handler(&self) -> fn(Value) -> super::ApiFuture;
}

pub type CompleteFn = fn(&str) -> Vec<String>;

pub struct Parameter {
    pub name: &'static str,
    pub description: &'static str,
    pub type_info: &'static TypeInfo,
}

pub struct TypeInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub complete_fn: Option<CompleteFn>,
}

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
