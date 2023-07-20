use crate::schema::ENTITY_NAME_SCHEMA;
use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};
use serde::{Deserialize, Serialize};

pub(crate) const GROUP_TYPENAME: &str = "group";

#[api(
    properties: {
        "endpoint": {
            type: Array,
            items: {
                description: "Name of the included endpoint(s)",
                type: String,
            },
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        filter: {
            optional: true,
            schema: ENTITY_NAME_SCHEMA,
        },
    },
)]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for notification channels
pub struct GroupConfig {
    /// Name of the channel
    #[updater(skip)]
    pub name: String,
    /// Endpoints for this channel
    pub endpoint: Vec<String>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Filter to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableGroupProperty {
    Comment,
    Filter,
}
