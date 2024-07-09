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
/// Syslog API filtering options.
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

#[api(
    properties: {
        since: {
            type: Integer,
            optional: true,
            description: "Display all log since this UNIX epoch. Conflicts with 'startcursor'.",
            minimum: 0,
        },
        until: {
            type: Integer,
            optional: true,
            description: "Display all log until this UNIX epoch. Conflicts with 'endcursor'.",
            minimum: 0,
        },
        lastentries: {
            type: Integer,
            optional: true,
            description: "Limit to the last X lines. Conflicts with a range.",
            minimum: 0,
        },
        startcursor: {
            type: String,
            description: "Start after the given Cursor. Conflicts with 'since'.",
            optional: true,
        },
        endcursor: {
            type: String,
            description: "End before the given Cursor. Conflicts with 'until'",
            optional: true,
        },
    }
)]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
/// Journal API filtering options.
pub struct JournalFilter {
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub lastentries: Option<u64>,
    pub startcursor: Option<String>,
    pub endcursor: Option<String>,
}
