//! The generated API client code.

use proxmox_client::{ApiResponseData, Error, HttpApiClient};

use crate::types::*;

use super::{add_query_arg, add_query_bool};

pub struct PveClient<T: HttpApiClient>(pub T);

include!("../generated/code.rs");
