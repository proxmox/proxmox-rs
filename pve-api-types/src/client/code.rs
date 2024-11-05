// Supress warnings about unused function parameter in the default impl of the trait methods.
#![allow(unused)]
//! The generated API client code.

use proxmox_client::{ApiResponseData, Error, HttpApiClient};

use crate::types::*;

use super::{add_query_arg, add_query_bool};

pub struct PveClientImpl<T: HttpApiClient>(pub T);

include!("../generated/code.rs");
