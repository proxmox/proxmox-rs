use serde::{Deserialize, Serialize};

use proxmox_schema::api;
use proxmox_schema::api_types::TIME_ZONE_SCHEMA;

#[api(
    properties: {
        timezone: {
            schema: TIME_ZONE_SCHEMA,
        },
        time: {
            type: i64,
            description: "Seconds since 1970-01-01 00:00:00 UTC.",
            minimum: 1_297_163_644,
        },
        localtime: {
            type: i64,
            description: "Seconds since 1970-01-01 00:00:00 UTC. (local time)",
            minimum: 1_297_163_644,
        },
    }
)]
#[derive(Serialize, Deserialize)]
/// Server time and timezone.
pub struct ServerTimeInfo {
    pub timezone: String,
    pub time: i64,
    pub localtime: i64,
}
