use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::scheduler;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Static usage information of a node.
pub struct StaticNodeUsage {
    /// Hostname of the node.
    pub name: String,
    /// CPU utilization. Can be more than `maxcpu` if overcommitted.
    pub cpu: f64,
    /// Total number of CPUs.
    pub maxcpu: usize,
    /// Used memory in bytes. Can be more than `maxmem` if overcommitted.
    pub mem: usize,
    /// Total memory in bytes.
    pub maxmem: usize,
}

impl StaticNodeUsage {
    /// Add usage of `service` to the node's usage.
    pub fn add_service_usage(&mut self, service: &StaticServiceUsage) {
        self.cpu = add_cpu_usage(self.cpu, self.maxcpu as f64, service.maxcpu);
        self.mem += service.maxmem;
    }
}

impl AsRef<StaticNodeUsage> for StaticNodeUsage {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// Calculate new CPU usage in percent.
/// `add` being `0.0` means "unlimited" and results in `max` being added.
fn add_cpu_usage(old: f64, max: f64, add: f64) -> f64 {
    if add == 0.0 {
        old + max
    } else {
        old + add
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Static usage information of an HA resource.
pub struct StaticServiceUsage {
    /// Number of assigned CPUs or CPU limit.
    pub maxcpu: f64,
    /// Maximum assigned memory in bytes.
    pub maxmem: usize,
}

/// Scores candidate `nodes` to start a `service` on. Scoring is done according to the static memory
/// and CPU usages of the nodes as if the service would already be running on each.
///
/// Returns a vector of (nodename, score) pairs. Scores are between 0.0 and 1.0 and a higher score
/// is better.
pub fn score_nodes_to_start_service<T: AsRef<StaticNodeUsage>>(
    nodes: &[T],
    service: &StaticServiceUsage,
) -> Result<Vec<(String, f64)>, Error> {
    scheduler::score_nodes_to_start_service(nodes, service)
}
