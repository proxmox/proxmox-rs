use serde_json::Value;

use crate::{RpcEnvironment, RpcEnvironmentType};

/// `RpcEnvironmet` implementation for command line tools
#[derive(Default)]
pub struct CliEnvironment {
    result_attributes: Value,
    auth_id: Option<String>,
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

    fn set_auth_id(&mut self, auth_id: Option<String>) {
        self.auth_id = auth_id;
    }

    fn get_auth_id(&self) -> Option<String> {
        self.auth_id.clone()
    }
}
