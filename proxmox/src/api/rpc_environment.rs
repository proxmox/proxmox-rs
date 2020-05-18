use crate::tools::AsAny;

use serde_json::Value;

/// Abstract Interface for API methods to interact with the environment
pub trait RpcEnvironment: std::any::Any + AsAny + Send {
    /// Use this to pass additional result data. It is up to the environment
    /// how the data is used.
    fn result_attrib_mut(&mut self) -> &mut Value;

    /// Access result attribute immutable
    fn result_attrib(&self) -> &Value;

    /// The environment type
    fn env_type(&self) -> RpcEnvironmentType;

    /// Set user name
    fn set_user(&mut self, user: Option<String>);

    /// Get user name
    fn get_user(&self) -> Option<String>;
}

/// Environment Type
///
/// We use this to enumerate the different environment types. Some methods
/// needs to do different things when started from the command line interface,
/// or when executed from a privileged server running as root.
#[derive(PartialEq, Copy, Clone)]
pub enum RpcEnvironmentType {
    /// Command started from command line
    CLI,
    /// Access from public accessible server
    PUBLIC,
    /// Access from privileged server (run as root)
    PRIVILEGED,
}

impl core::ops::Index<&str> for &dyn RpcEnvironment
{
    type Output = Value;
    fn index(&self, index: &str) -> &Value {
        &self.result_attrib().index(index)
    }
}

impl core::ops::Index<&str> for &mut dyn RpcEnvironment
{
    type Output = Value;
    fn index(&self, index: &str) -> &Value {
        &self.result_attrib().index(index)
    }
}

impl core::ops::IndexMut<&str> for &mut dyn RpcEnvironment
{
    fn index_mut(&mut self, index: &str) -> &mut Value {
        self.result_attrib_mut().index_mut(index)
    }
}
