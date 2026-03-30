use anyhow::Error;

use crate::{
    pve_static::{StaticNodeUsage, StaticServiceUsage},
    topsis,
};

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
pub fn score_nodes_to_start_service<T: AsRef<StaticNodeUsage>>(
    nodes: &[T],
    service: &StaticServiceUsage,
) -> Result<Vec<(String, f64)>, Error> {
    let len = nodes.len();

    let matrix = nodes
        .iter()
        .enumerate()
        .map(|(target_index, _)| {
            // Base values on percentages to allow comparing nodes with different stats.
            let mut highest_cpu = 0.0;
            let mut squares_cpu = 0.0;
            let mut highest_mem = 0.0;
            let mut squares_mem = 0.0;

            for (index, node) in nodes.iter().enumerate() {
                let node = node.as_ref();
                let new_cpu = if index == target_index {
                    if service.maxcpu == 0.0 {
                        node.cpu + node.maxcpu as f64
                    } else {
                        node.cpu + service.maxcpu
                    }
                } else {
                    node.cpu
                } / (node.maxcpu as f64);
                highest_cpu = f64::max(highest_cpu, new_cpu);
                squares_cpu += new_cpu.powi(2);

                let new_mem = if index == target_index {
                    node.mem + service.maxmem
                } else {
                    node.mem
                } as f64
                    / node.maxmem as f64;
                highest_mem = f64::max(highest_mem, new_mem);
                squares_mem += new_mem.powi(2);
            }

            // Add 1.0 to avoid boosting tiny differences: e.g. 0.004 is twice as much as 0.002, but
            // 1.004 is only slightly more than 1.002.
            PveTopsisAlternative {
                average_cpu: 1.0 + (squares_cpu / len as f64).sqrt(),
                highest_cpu: 1.0 + highest_cpu,
                average_memory: 1.0 + (squares_mem / len as f64).sqrt(),
                highest_memory: 1.0 + highest_mem,
            }
            .into()
        })
        .collect::<Vec<_>>();

    let scores =
        topsis::score_alternatives(&topsis::Matrix::new(matrix)?, &PVE_HA_TOPSIS_CRITERIA)?;

    Ok(scores
        .into_iter()
        .enumerate()
        .map(|(n, score)| (nodes[n].as_ref().name.clone(), score))
        .collect())
}
