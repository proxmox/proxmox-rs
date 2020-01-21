use serde_json::Value;
use std::collections::HashMap;

use crate::api::{RpcEnvironment, RpcEnvironmentType};

/// `RpcEnvironmet` implementation for command line tools
#[derive(Default)]
pub struct CliEnvironment {
    result_attributes: HashMap<String, Value>,
    user: Option<String>,
}

impl CliEnvironment {
    pub fn new() -> Self {
        Default::default()
    }
}

impl RpcEnvironment for CliEnvironment {
    fn set_result_attrib(&mut self, name: &str, value: Value) {
        self.result_attributes.insert(name.into(), value);
    }

    fn get_result_attrib(&self, name: &str) -> Option<&Value> {
        self.result_attributes.get(name)
    }

    fn env_type(&self) -> RpcEnvironmentType {
        RpcEnvironmentType::CLI
    }

    fn set_user(&mut self, user: Option<String>) {
        self.user = user;
    }

    fn get_user(&self) -> Option<String> {
        self.user.clone()
    }
}
