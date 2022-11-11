use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::topsis::TopsisMatrix;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Static usage information of a node.
pub struct StaticNodeUsage {
    /// Hostname of the node.
    pub name: String,
    /// CPU utilization. Can be more than `maxcpu` if overcommited.
    pub cpu: f64,
    /// Total number of CPUs.
    pub maxcpu: usize,
    /// Used memory in bytes. Can be more than `maxmem` if overcommited.
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

criteria_struct! {
    /// A given alternative.
    struct PveTopsisAlternative {
        #[criterion("average CPU", -1.0)]
        average_cpu: f64,
        #[criterion("highest CPU", -2.0)]
        highest_cpu: f64,
        #[criterion("average memory", -5.0)]
        average_memory: f64,
        #[criterion("highest memory", -10.0)]
        highest_memory: f64,
    }

    const N_CRITERIA;
    static PVE_HA_TOPSIS_CRITERIA;
}

/// Scores candidate `nodes` to start a `service` on. Scoring is done according to the static memory
/// and CPU usages of the nodes as if the service would already be running on each.
///
/// Returns a vector of (nodename, score) pairs. Scores are between 0.0 and 1.0 and a higher score
/// is better.
pub fn score_nodes_to_start_service(
    nodes: &[&StaticNodeUsage],
    service: &StaticServiceUsage,
) -> Result<Vec<(String, f64)>, Error> {
    let len = nodes.len();

    let matrix = nodes
        .iter()
        .enumerate()
        .map(|(target_index, _)| {
            // all of these are as percentages to be comparable across nodes
            let mut highest_cpu = 0.0;
            let mut sum_cpu = 0.0;
            let mut highest_mem = 0.0;
            let mut sum_mem = 0.0;

            for (index, node) in nodes.iter().enumerate() {
                let new_cpu = if index == target_index {
                    add_cpu_usage(node.cpu, node.maxcpu as f64, service.maxcpu)
                } else {
                    node.cpu
                } / (node.maxcpu as f64);
                highest_cpu = f64::max(highest_cpu, new_cpu);
                sum_cpu += new_cpu;

                let new_mem = if index == target_index {
                    node.mem + service.maxmem
                } else {
                    node.mem
                } as f64
                    / node.maxmem as f64;
                highest_mem = f64::max(highest_mem, new_mem);
                sum_mem += new_mem;
            }

            PveTopsisAlternative {
                average_cpu: sum_cpu / len as f64,
                highest_cpu,
                average_memory: sum_mem / len as f64,
                highest_memory: highest_mem,
            }
            .into()
        })
        .collect::<Vec<_>>();

    let scores =
        crate::topsis::score_alternatives(&TopsisMatrix::new(matrix)?, &PVE_HA_TOPSIS_CRITERIA)?;

    Ok(scores
        .into_iter()
        .enumerate()
        .map(|(n, score)| (nodes[n].name.clone(), score))
        .collect())
}
