use serde::{Deserialize, Serialize};

use proxmox_schema::api;

use crate::schema::ENTITY_NAME_SCHEMA;

pub(crate) const GROUP_TYPENAME: &str = "group";

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
    },
    additional_properties: true,
)]
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for the old target groups - can be removed at some point.
pub struct GroupConfig {
    /// Name of the group
    pub name: String,
}
