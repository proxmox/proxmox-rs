use std::sync::LazyLock;
use std::time::Duration;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use ::serde::{Deserialize, Serialize};
use anyhow::{bail, Error};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

use crate::SmartStatus;

#[cfg(feature = "api-types")]
use proxmox_schema::api;

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
/// SMART Attribute
pub struct SmartAttribute {
    /// Attribute name
    pub name: String,
    /// Attribute raw value.
    pub raw: String,
    // The remaining fields are only available for ATA devices.
    /// ATA Attribute ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// ATA Flags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
    /// ATA normalized value (0..100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<f64>,
    /// ATA worst
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worst: Option<f64>,
    /// ATA threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        status: {
            type: SmartStatus,
        },
        wearout: {
            description: "Wearout level.",
            type: f64,
            optional: true,
        },
        attributes: {
            description: "SMART attributes.",
            type: Array,
            items: {
                type: SmartAttribute,
            },
        },
    },
))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
/// Data from smartctl
pub struct SmartData {
    pub status: SmartStatus,
    pub wearout: Option<f64>,
    pub attributes: Vec<SmartAttribute>,
}

/// Default timeout for smartctl invocations (30 seconds).
pub const DEFAULT_SMART_TIMEOUT: Duration = Duration::from_secs(30);

/// Read S.M.A.R.T. data for a block device via `smartctl`.
///
/// The `timeout` parameter limits how long `smartctl` may run. This prevents stalls from
/// unresponsive disks (for example, a spindle in standby mode) from blocking the entire disk
/// enumeration.
pub fn get_smart_data(
    disk_path: &Path,
    health_only: bool,
    timeout: Duration,
) -> Result<SmartData, Error> {
    let output = run_smartctl(disk_path, health_only, timeout)?;

    let output: serde_json::Value = output.parse()?;

    let mut wearout = None;

    let mut attributes = Vec::new();
    let mut wearout_candidates = HashMap::new();

    // ATA devices
    if let Some(list) = output["ata_smart_attributes"]["table"].as_array() {
        for item in list {
            let id = match item["id"].as_u64() {
                Some(id) => id,
                None => continue, // skip attributes without id
            };

            let name = match item["name"].as_str() {
                Some(name) => name.to_string(),
                None => continue, // skip attributes without name
            };

            let raw_value = match item["raw"]["string"].as_str() {
                Some(value) => value.to_string(),
                None => continue, // skip attributes without raw value
            };

            let flags = match item["flags"]["string"].as_str() {
                Some(flags) => flags.to_string(),
                None => continue, // skip attributes without flags
            };

            let normalized = match item["value"].as_f64() {
                Some(v) => v,
                None => continue, // skip attributes without normalize value
            };

            let worst = match item["worst"].as_f64() {
                Some(v) => v,
                None => continue, // skip attributes without worst entry
            };

            let threshold = match item["thresh"].as_f64() {
                Some(v) => v,
                None => continue, // skip attributes without threshold entry
            };

            if WEAROUT_FIELD_NAMES.contains(&name as &str) {
                wearout_candidates.insert(name.clone(), normalized);
            }

            attributes.push(SmartAttribute {
                name,
                raw: raw_value,
                id: Some(id),
                flags: Some(flags),
                normalized: Some(normalized),
                worst: Some(worst),
                threshold: Some(threshold),
            });
        }
    }

    if !wearout_candidates.is_empty() {
        for field in WEAROUT_FIELD_ORDER {
            if let Some(value) = wearout_candidates.get(field as &str) {
                wearout = Some(*value);
                break;
            }
        }
    }

    // NVME devices
    if let Some(list) = output["nvme_smart_health_information_log"].as_object() {
        for (name, value) in list {
            if name == "percentage_used" {
                // extract wearout from nvme text, allow for decimal values
                if let Some(v) = value.as_f64()
                    && v <= 100.0
                {
                    wearout = Some(100.0 - v);
                }
            }
            if let Some(value) = value.as_f64() {
                attributes.push(SmartAttribute {
                    name: name.to_string(),
                    raw: value.to_string(),
                    id: None,
                    flags: None,
                    normalized: None,
                    worst: None,
                    threshold: None,
                });
            }
        }
    }

    let status = match output["smart_status"]["passed"].as_bool() {
        None => SmartStatus::Unknown,
        Some(true) => SmartStatus::Passed,
        Some(false) => SmartStatus::Failed,
    };

    Ok(SmartData {
        status,
        wearout,
        attributes,
    })
}

fn run_smartctl(disk_path: &Path, health_only: bool, timeout: Duration) -> Result<String, Error> {
    let mut command = std::process::Command::new("smartctl");
    command.arg("-H");
    if !health_only {
        command.arg("-A");
    }
    // always request JSON output so the caller can parse the result
    command.arg("-j").arg(disk_path);

    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let pid = Pid::from_raw(child.id() as i32);

    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let output = match rx.recv_timeout(timeout) {
        Ok(result) => result?,
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
            // Kill the smartctl process to avoid leaving it running indefinitely.
            let _ = signal::kill(pid, Signal::SIGKILL);
            bail!(
                "smartctl timed out after {}s for {}",
                timeout.as_secs(),
                disk_path.display(),
            );
        }
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
            bail!(
                "smartctl thread terminated unexpectedly for {}",
                disk_path.display(),
            );
        }
    };

    let exitcode = output.status.code().unwrap_or(-1);
    // only bits 0-1 in the smartctl exit code are fatal errors
    if (exitcode & 0b0011) != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "smartctl failed for {} (exit code {exitcode}): {stderr}",
            disk_path.display(),
        );
    }

    Ok(String::from_utf8(output.stdout)?)
}

static WEAROUT_FIELD_ORDER: &[&str] = &[
    "Media_Wearout_Indicator",
    "SSD_Life_Left",
    "Wear_Leveling_Count",
    "Perc_Write/Erase_Ct_BC",
    "Perc_Rated_Life_Remain",
    "Remaining_Lifetime_Perc",
    "Percent_Lifetime_Remain",
    "Lifetime_Left",
    "PCT_Life_Remaining",
    "Lifetime_Remaining",
    "Percent_Life_Remaining",
    "Percent_Lifetime_Used",
    "Perc_Rated_Life_Used",
];

static WEAROUT_FIELD_NAMES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| WEAROUT_FIELD_ORDER.iter().cloned().collect());
