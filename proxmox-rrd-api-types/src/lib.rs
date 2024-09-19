use serde::{Deserialize, Serialize};

use proxmox_schema::api;

#[api]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
/// RRD consolidation mode
pub enum RrdMode {
    /// Maximum
    Max,
    /// Average
    Average,
}

serde_plain::derive_display_from_serialize!(RrdMode);
serde_plain::derive_fromstr_from_deserialize!(RrdMode);

#[deprecated = "use RrdMode instead"]
pub type RRDMode = RrdMode;

#[api]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
/// RRD time frame
pub enum RrdTimeframe {
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

serde_plain::derive_display_from_serialize!(RrdTimeframe);
serde_plain::derive_fromstr_from_deserialize!(RrdTimeframe);

#[deprecated = "use RrdTimeframe instead"]
pub type RRDTimeFrame = RrdTimeframe;
