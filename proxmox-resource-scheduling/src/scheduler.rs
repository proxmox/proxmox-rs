use anyhow::Error;

use crate::{node::NodeStats, resource::ResourceStats, topsis};

/// The scheduler view of a node.
#[derive(Clone, Debug)]
pub struct NodeUsage {
    /// The identifier of the node.
    pub name: String,
    /// The usage statistics of the node.
    pub stats: NodeStats,
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

#[derive(Clone, Debug)]
pub struct Scheduler {
    nodes: Vec<NodeUsage>,
}

impl Scheduler {
    /// Instantiate scheduler instance from node usages.
    pub fn from_nodes<I>(nodes: I) -> Self
    where
        I: IntoIterator<Item: Into<NodeUsage>>,
    {
        Self {
            nodes: nodes.into_iter().map(|node| node.into()).collect(),
        }
    }

    /// Map the current node usages to a [`PveTopsisAlternative`].
    ///
    /// The [`PveTopsisAlternative`] is derived by calculating a modified version of the root mean
    /// square (RMS) and maximum value of each stat in the node usages.
    fn topsis_alternative_with(
        &self,
        map_node_stats: impl Fn(&NodeUsage) -> NodeStats,
    ) -> PveTopsisAlternative {
        let len = self.nodes.len();

        // Base values on percentages to allow comparing nodes with different stats.
        let mut highest_cpu = 0.0;
        let mut squares_cpu = 0.0;
        let mut highest_mem = 0.0;
        let mut squares_mem = 0.0;

        for node in self.nodes.iter() {
            let new_stats = map_node_stats(node);

            let new_cpu = new_stats.cpu_load();
            highest_cpu = f64::max(highest_cpu, new_cpu);
            squares_cpu += new_cpu.powi(2);

            let new_mem = new_stats.mem_load();
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
    }

    /// Scores nodes to start a resource with the usage statistics `resource_stats` on.
    ///
    /// The scoring is done as if the resource is already started on each node. This assumes that
    /// the already started resource consumes the maximum amount of each stat according to its
    /// `resource_stats`.
    ///
    /// Returns a vector of (nodename, score) pairs. Scores are between 0.0 and 1.0 and a higher
    /// score is better.
    pub fn score_nodes_to_start_resource<T: Into<ResourceStats>>(
        &self,
        resource_stats: T,
    ) -> Result<Vec<(String, f64)>, Error> {
        let resource_stats = resource_stats.into();

        let matrix = self
            .nodes
            .iter()
            .map(|node| {
                self.topsis_alternative_with(|target_node| {
                    let mut new_stats = target_node.stats;

                    if node.name == target_node.name {
                        new_stats.add_started_resource(&resource_stats)
                    }

                    new_stats
                })
                .into()
            })
            .collect::<Vec<_>>();

        let scores =
            topsis::score_alternatives(&topsis::Matrix::new(matrix)?, &PVE_HA_TOPSIS_CRITERIA)?;

        Ok(scores
            .into_iter()
            .enumerate()
            .map(|(n, score)| (self.nodes[n].name.clone(), score))
            .collect())
    }
}
