use serde::{Deserialize, Serialize};

use proxmox_schema::api;

#[api]
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
/// RRD consolidation mode
pub enum RRDMode {
    /// Maximum
    Max,
    /// Average
    Average,
}

serde_plain::derive_display_from_serialize!(RRDMode);
serde_plain::derive_fromstr_from_deserialize!(RRDMode);

#[api]
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// RRD time frame
pub enum RRDTimeFrame {
    /// Hour
    Hour,
    /// Day
    Day,
    /// Week
    Week,
    /// Month
    Month,
    /// Year
    Year,
    /// Decade (10 years)
    Decade,
}

serde_plain::derive_display_from_serialize!(RRDTimeFrame);
serde_plain::derive_fromstr_from_deserialize!(RRDTimeFrame);
