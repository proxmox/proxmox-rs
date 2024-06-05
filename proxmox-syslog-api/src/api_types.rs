use serde::{Deserialize, Serialize};

use proxmox_schema::api;
use proxmox_schema::api_types::SYSTEMD_DATETIME_FORMAT;

#[api(
    properties: {
        start: {
            type: Integer,
            description: "Start line number.",
            minimum: 0,
            optional: true,
        },
        limit: {
            type: Integer,
            description: "Max. number of lines.",
            optional: true,
            minimum: 0,
        },
        since: {
            type: String,
            optional: true,
            description: "Display all log since this date-time string.",
	        format: &SYSTEMD_DATETIME_FORMAT,
        },
        until: {
            type: String,
            optional: true,
            description: "Display all log until this date-time string.",
	        format: &SYSTEMD_DATETIME_FORMAT,
        },
        service: {
            type: String,
            optional: true,
            description: "Service ID.",
            max_length: 128,
        },
    },
)]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
/// Syslog filtering options.
pub struct SyslogFilter {
    pub start: Option<u64>,
    pub limit: Option<u64>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub service: Option<String>,
}

#[api]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
/// Syslog line with line number.
pub struct SyslogLine {
    /// Line number.
    pub n: u64,
    /// Line text.
    pub t: String,
}
