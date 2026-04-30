use anyhow::Error;

use crate::{node::NodeStats, resource::ResourceStats, topsis};

use serde::{Deserialize, Serialize};
use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};

/// The scheduler view of a node.
#[derive(Clone, Debug)]
pub struct NodeUsage {
    /// The identifier of the node.
    pub name: String,
    /// The usage statistics of the node.
    pub stats: NodeStats,
}

/// Returns the load imbalance among the nodes, which is a value between 0 and 1 that describes the
/// statistical dispersion of the individual node loads around the mean node load. The lower the
/// value, the better.
///
/// In more detail, the current implementation computes the so-called coefficient of variation (CV),
/// which is the ratio of the standard deviation to the mean of the given node loads. The lower
/// bound of the CV is reached if all node loads are equal. The upper bound is reached if all nodes
/// except one are idle. To present the CV as a value between 0 and 1, it's being divided by the
/// upper bound of the CV for the given number of nodes.
fn calculate_node_imbalance(nodes: &[NodeUsage], to_load: impl Fn(&NodeUsage) -> f64) -> f64 {
    let node_count = nodes.len();

    // early return with perfect imbalance to avoid division by zero
    if node_count < 2 {
        return 0.0;
    }

    let node_loads = nodes.iter().map(to_load).collect::<Vec<_>>();
    let load_sum = node_loads.iter().sum::<f64>();

    // early return with perfect imbalance to avoid division by zero
    if load_sum == 0.0 {
        return 0.0;
    }

    let load_mean = load_sum / node_count as f64;
    let squared_diff_sum = node_loads
        .iter()
        .fold(0.0, |sum, node_load| sum + (node_load - load_mean).powi(2));
    let load_sd = (squared_diff_sum / node_count as f64).sqrt();

    let cv = load_sd / load_mean;

    // https://stats.stackexchange.com/questions/18621
    let max_cv = ((node_count - 1) as f64).sqrt();

    cv / max_cv
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

/// A possible migration.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Migration {
    /// The identifier of a leading resource.
    pub sid: String,
    /// The current node of the leading resource.
    pub source_node: String,
    /// The possible migration target node for the resource.
    pub target_node: String,
}

/// A possible migration with a score.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ScoredMigration {
    /// The possible migration.
    pub migration: Migration,
    /// The expected node imbalance after the migration.
    pub imbalance: f64,
}

impl Ord for ScoredMigration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.imbalance
            .total_cmp(&other.imbalance)
            .then(self.migration.cmp(&other.migration))
    }
}

impl PartialOrd for ScoredMigration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScoredMigration {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ScoredMigration {}

impl ScoredMigration {
    pub fn new<T: Into<Migration>>(migration: T, imbalance: f64) -> Self {
        // Depending on how the imbalance is calculated, it can contain minor approximation errors.
        // As this struct implements the Ord trait, users of the struct's cmp() can run into cases,
        // where the imbalance is the same up to the significant digits in base 10, but treated as
        // different values.
        //
        // Therefore, truncate any non-significant digits to prevent these cases.
        let factor = 10_f64.powi(f64::DIGITS as i32);
        let truncated_imbalance = f64::trunc(factor * imbalance) / factor;

        Self {
            migration: migration.into(),
            imbalance: truncated_imbalance,
        }
    }
}

/// A possible migration candidate with the migrated usage stats.
#[derive(Clone, Debug)]
pub struct MigrationCandidate {
    /// The possible migration.
    pub migration: Migration,
    /// Usage stats of the resource(s) to be migrated.
    pub stats: ResourceStats,
}

impl From<MigrationCandidate> for Migration {
    fn from(candidate: MigrationCandidate) -> Self {
        candidate.migration
    }
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

    /// Returns the load imbalance among the nodes.
    ///
    /// See [`calculate_node_imbalance`] for more information.
    pub fn node_imbalance(&self) -> f64 {
        calculate_node_imbalance(&self.nodes, |node| node.stats.load())
    }

    /// Returns the load imbalance among the nodes as if a specific resource was moved.
    ///
    /// See [`calculate_node_imbalance`] for more information.
    fn node_imbalance_with_migration_candidate(&self, candidate: &MigrationCandidate) -> f64 {
        calculate_node_imbalance(&self.nodes, |node| {
            let mut new_stats = node.stats;

            if node.name == candidate.migration.source_node {
                new_stats.remove_running_resource(&candidate.stats);
            } else if node.name == candidate.migration.target_node {
                new_stats.add_running_resource(&candidate.stats);
            }

            new_stats.load()
        })
    }

    /// Scores the given migration `candidates` by the best node imbalance improvement with
    /// exhaustive search.
    ///
    /// The `candidates` are assumed to be consistent with the scheduler. No further validation is
    /// done whether the given nodenames actually exist in the scheduler.
    ///
    /// The scoring is done as if each resource migration has already been done. This assumes that
    /// the already migrated resource consumes the same amount of each stat as on the previous node
    /// according to its `stats`.
    ///
    /// Returns up to `limit` of the best scored migrations.
    pub fn score_best_balancing_migration_candidates<I>(
        &self,
        candidates: I,
        limit: usize,
    ) -> Vec<ScoredMigration>
    where
        I: IntoIterator<Item = MigrationCandidate>,
    {
        let mut scored_migrations = candidates
            .into_iter()
            .map(|candidate| {
                let imbalance = self.node_imbalance_with_migration_candidate(&candidate);

                Reverse(ScoredMigration::new(candidate, imbalance))
            })
            .collect::<BinaryHeap<_>>();

        let mut best_migrations = Vec::with_capacity(limit);

        // BinaryHeap::into_iter_sorted() is still in nightly unfortunately
        while best_migrations.len() < limit {
            match scored_migrations.pop() {
                Some(Reverse(alternative)) => best_migrations.push(alternative),
                None => break,
            }
        }

        best_migrations
    }

    /// Scores the given migration `candidates` by the best node imbalance improvement with the
    /// TOPSIS method.
    ///
    /// The `candidates` are assumed to be consistent with the scheduler. No further validation is
    /// done whether the given nodenames actually exist in the scheduler.
    ///
    /// The scoring is done as if each resource migration has already been done. This assumes that
    /// the already migrated resource consumes the same amount of each stat as on the previous node
    /// according to its `stats`.
    ///
    /// Returns up to `limit` of the best scored migrations.
    pub fn score_best_balancing_migration_candidates_topsis(
        &self,
        candidates: &[MigrationCandidate],
        limit: usize,
    ) -> Result<Vec<ScoredMigration>, Error> {
        let matrix = candidates
            .iter()
            .map(|candidate| {
                let resource_stats = &candidate.stats;
                let source_node = &candidate.migration.source_node;
                let target_node = &candidate.migration.target_node;

                self.topsis_alternative_with(|node| {
                    let mut new_stats = node.stats;

                    if &node.name == source_node {
                        new_stats.remove_running_resource(resource_stats);
                    } else if &node.name == target_node {
                        new_stats.add_running_resource(resource_stats);
                    }

                    new_stats
                })
                .into()
            })
            .collect::<Vec<_>>();

        let best_alternatives =
            topsis::rank_alternatives(&topsis::Matrix::new(matrix)?, &PVE_HA_TOPSIS_CRITERIA)?;

        Ok(best_alternatives
            .into_iter()
            .take(limit)
            .map(|i| {
                let imbalance = self.node_imbalance_with_migration_candidate(&candidates[i]);

                ScoredMigration::new(candidates[i].clone(), imbalance)
            })
            .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scored_migration_order() {
        let migration1 = ScoredMigration::new(
            Migration {
                sid: String::from("vm:102"),
                source_node: String::from("node1"),
                target_node: String::from("node2"),
            },
            0.7231749488916931,
        );
        let migration2 = ScoredMigration::new(
            Migration {
                sid: String::from("vm:102"),
                source_node: String::from("node1"),
                target_node: String::from("node3"),
            },
            0.723174948891693,
        );
        let migration3 = ScoredMigration::new(
            Migration {
                sid: String::from("vm:101"),
                source_node: String::from("node1"),
                target_node: String::from("node2"),
            },
            0.723174948891693 + 1e-15,
        );

        let mut migrations = vec![migration2.clone(), migration3.clone(), migration1.clone()];

        migrations.sort();

        assert_eq!(
            vec![migration1.clone(), migration2.clone(), migration3.clone()],
            migrations
        );

        let mut heap = BinaryHeap::from(vec![
            Reverse(migration2.clone()),
            Reverse(migration3.clone()),
            Reverse(migration1.clone()),
        ]);

        assert_eq!(heap.pop(), Some(Reverse(migration1)));
        assert_eq!(heap.pop(), Some(Reverse(migration2)));
        assert_eq!(heap.pop(), Some(Reverse(migration3)));
    }
}
