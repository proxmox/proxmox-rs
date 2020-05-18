use serde_json::Value;

use crate::api::{RpcEnvironment, RpcEnvironmentType};

/// `RpcEnvironmet` implementation for command line tools
#[derive(Default)]
pub struct CliEnvironment {
    result_attributes: Value,
    user: Option<String>,
}

impl CliEnvironment {
    pub fn new() -> Self {
        Default::default()
    }
}

impl RpcEnvironment for CliEnvironment {
    fn result_attrib_mut(&mut self) -> &mut Value {
        &mut self.result_attributes
    }

    fn result_attrib(&self) -> &Value {
        &self.result_attributes
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
