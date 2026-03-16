use std::any::{Any, TypeId};
use std::collections::HashMap;

use serde_json::Value;

use proxmox_schema::ApiType;

use crate::{RpcEnvironment, RpcEnvironmentType};

/// [`RpcEnvironment`] implementation for command line tools.
///
/// Stores global options parsed by the [`CommandLine`](super::CommandLine) parser, accessible via
/// [`global_option`](Self::global_option) and [`take_global_option`](Self::take_global_option).
#[derive(Default)]
pub struct CliEnvironment {
    result_attributes: Value,
    auth_id: Option<String>,
    pub(crate) global_options: HashMap<TypeId, Box<dyn Any + Send + Sync + 'static>>,
}

impl CliEnvironment {
    pub fn new() -> Self {
        Default::default()
    }

    /// Borrow a global option by type.
    ///
    /// Returns `None` if the option type was not registered or no value was provided on the
    /// command line.
    pub fn global_option<T>(&self) -> Option<&T>
    where
        T: ApiType + Any + Send + Sync + 'static,
    {
        Some(
            self.global_options
                .get(&TypeId::of::<T>())?
                .downcast_ref::<T>()
                .unwrap(), // the map must only store correctly typed items!
        )
    }

    /// Mutably borrow a global option by type.
    pub fn global_option_mut<T>(&mut self) -> Option<&mut T>
    where
        T: ApiType + Any + Send + Sync + 'static,
    {
        Some(
            self.global_options
                .get_mut(&TypeId::of::<T>())?
                .downcast_mut::<T>()
                .unwrap(), // the map must only store correctly typed items!
        )
    }

    /// Remove and return a global option by type.
    ///
    /// Typically used after [`CommandLine::parse`](super::CommandLine::parse) to extract global
    /// options before calling the command handler. a2abfd30 (fixup! router: cli: improve documentation for CLI parser types)
    pub fn take_global_option<T>(&mut self) -> Option<T>
    where
        T: ApiType + Any + Send + Sync + 'static,
    {
        Some(
            *self
                .global_options
                .remove(&TypeId::of::<T>())?
                .downcast::<T>()
                .unwrap(), // the map must only store correctly typed items!
        )
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
