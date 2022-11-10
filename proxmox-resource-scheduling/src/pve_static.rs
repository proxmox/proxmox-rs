use anyhow::Error;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::topsis::{TopsisCriteria, TopsisCriterion, TopsisMatrix};

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Static usage information of an HA resource.
pub struct StaticServiceUsage {
    /// Number of assigned CPUs or CPU limit.
    pub maxcpu: f64,
    /// Maximum assigned memory in bytes.
    pub maxmem: usize,
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

impl StaticNodeUsage {
    /// Add usage of `service` to the node's usage.
    pub fn add_service_usage(&mut self, service: &StaticServiceUsage) {
        self.cpu = add_cpu_usage(self.cpu, self.maxcpu as f64, service.maxcpu);
        self.mem += service.maxmem;
    }
}

/// A given alternative.
struct PveTopsisAlternative {
    average_cpu: f64,
    highest_cpu: f64,
    average_memory: f64,
    highest_memory: f64,
}

const N_CRITERIA: usize = 4;

// NOTE It is essenital that the order of the criteria definition and the order in the
// From<PveTopsisAlternative> implementation match up.

lazy_static! {
    static ref PVE_HA_TOPSIS_CRITERIA: TopsisCriteria<N_CRITERIA> = TopsisCriteria::new([
        TopsisCriterion::new("average CPU".to_string(), -1.0),
        TopsisCriterion::new("highest CPU".to_string(), -2.0),
        TopsisCriterion::new("average memory".to_string(), -5.0),
        TopsisCriterion::new("highest memory".to_string(), -10.0),
    ])
    .unwrap();
}

impl From<PveTopsisAlternative> for [f64; N_CRITERIA] {
    fn from(alternative: PveTopsisAlternative) -> Self {
        [
            alternative.average_cpu,
            alternative.highest_cpu,
            alternative.average_memory,
            alternative.highest_memory,
        ]
    }
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
