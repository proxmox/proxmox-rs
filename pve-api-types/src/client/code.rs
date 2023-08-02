use proxmox_client::Client as ProxmoxClient;
use proxmox_client::{ApiResponse, Environment, Error, HttpClient};

use crate::types::*;

use super::{add_query_arg, add_query_bool};

pub struct Client<C, E: Environment> {
    client: ProxmoxClient<C, E>,
}

impl<C, E: Environment> Client<C, E> {
    /// Get the underlying client object.
    pub fn inner(&self) -> &ProxmoxClient<C, E> {
        &self.client
    }

    /// Get a mutable reference to the underlying client object.
    pub fn inner_mut(&mut self) -> &mut ProxmoxClient<C, E> {
        &mut self.client
    }
}

include!("../generated/code.rs");
