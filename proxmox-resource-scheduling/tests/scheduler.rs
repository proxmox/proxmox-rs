use anyhow::Error;
use proxmox_resource_scheduling::{
    node::NodeStats,
    resource::ResourceStats,
    scheduler::{Migration, MigrationCandidate, NodeUsage, Scheduler},
};

fn new_empty_cluster_scheduler() -> Scheduler {
    Scheduler::from_nodes(Vec::<NodeUsage>::new())
}

fn new_homogeneous_cluster_scheduler() -> Scheduler {
    let (maxcpu, maxmem) = (16, 64 * (1 << 30));

    let node1 = NodeUsage {
        name: String::from("node1"),
        stats: NodeStats {
            cpu: 1.7,
            maxcpu,
            mem: 12334 << 20,
            maxmem,
        },
    };

    let node2 = NodeUsage {
        name: String::from("node2"),
        stats: NodeStats {
            cpu: 15.184,
            maxcpu,
            mem: 529 << 20,
            maxmem,
        },
    };

    let node3 = NodeUsage {
        name: String::from("node3"),
        stats: NodeStats {
            cpu: 5.2,
            maxcpu,
            mem: 9381 << 20,
            maxmem,
        },
    };

    Scheduler::from_nodes(vec![node1, node2, node3])
}

fn new_heterogeneous_cluster_scheduler() -> Scheduler {
    let node1 = NodeUsage {
        name: String::from("node1"),
        stats: NodeStats {
            cpu: 1.7,
            maxcpu: 16,
            mem: 12334 << 20,
            maxmem: 128 << 30,
        },
    };

    let node2 = NodeUsage {
        name: String::from("node2"),
        stats: NodeStats {
            cpu: 15.184,
            maxcpu: 32,
            mem: 529 << 20,
            maxmem: 96 << 30,
        },
    };

    let node3 = NodeUsage {
        name: String::from("node3"),
        stats: NodeStats {
            cpu: 5.2,
            maxcpu: 24,
            mem: 9381 << 20,
            maxmem: 64 << 30,
        },
    };

    Scheduler::from_nodes(vec![node1, node2, node3])
}

#[test]
fn test_node_imbalance_with_empty_cluster() {
    let scheduler = new_empty_cluster_scheduler();

    assert_eq!(scheduler.node_imbalance(), 0.0);
}

#[test]
fn test_node_imbalance_with_perfectly_balanced_cluster() {
    let node = NodeUsage {
        name: String::from("node1"),
        stats: NodeStats {
            cpu: 1.7,
            maxcpu: 16,
            mem: 224395264,
            maxmem: 68719476736,
        },
    };

    let scheduler = Scheduler::from_nodes(vec![node.clone()]);

    assert_eq!(scheduler.node_imbalance(), 0.0);

    let scheduler = Scheduler::from_nodes(vec![node.clone(), node.clone(), node]);

    assert_eq!(scheduler.node_imbalance(), 0.0);
}

fn new_simple_migration_candidates() -> (Vec<MigrationCandidate>, Migration, Migration) {
    let migration1 = Migration {
        sid: String::from("vm:101"),
        source_node: String::from("node1"),
        target_node: String::from("node2"),
    };
    let migration2 = Migration {
        sid: String::from("vm:101"),
        source_node: String::from("node1"),
        target_node: String::from("node3"),
    };
    let stats = ResourceStats {
        cpu: 0.7,
        maxcpu: 4.0,
        mem: 8 << 30,
        maxmem: 16 << 30,
    };

    let candidates = vec![
        MigrationCandidate {
            migration: migration1.clone(),
            stats,
        },
        MigrationCandidate {
            migration: migration2.clone(),
            stats,
        },
    ];

    (candidates, migration1, migration2)
}

fn assert_imbalance(imbalance: f64, expected_imbalance: f64) {
    assert!(
        (expected_imbalance - imbalance).abs() <= f64::EPSILON,
        "imbalance is {imbalance}, but was expected to be {expected_imbalance}"
    );
}

fn rank_best_balancing_migration_candidates(
    scheduler: &Scheduler,
    candidates: Vec<MigrationCandidate>,
    limit: usize,
) -> Vec<Migration> {
    scheduler
        .score_best_balancing_migration_candidates(candidates, limit)
        .into_iter()
        .map(|entry| entry.migration)
        .collect()
}

#[test]
fn test_score_best_balancing_migration_candidates_with_no_candidates() {
    let scheduler = new_homogeneous_cluster_scheduler();

    assert_eq!(
        rank_best_balancing_migration_candidates(&scheduler, vec![], 2),
        vec![]
    );
}

#[test]
fn test_score_best_balancing_migration_candidates_in_homogeneous_cluster() {
    let scheduler = new_homogeneous_cluster_scheduler();

    assert_imbalance(scheduler.node_imbalance(), 0.4893954724628247);

    let (candidates, migration1, migration2) = new_simple_migration_candidates();

    assert_eq!(
        rank_best_balancing_migration_candidates(&scheduler, candidates, 2),
        vec![migration2, migration1]
    );
}

#[test]
fn test_score_best_balancing_migration_candidates_in_heterogeneous_cluster() {
    let scheduler = new_heterogeneous_cluster_scheduler();

    assert_imbalance(scheduler.node_imbalance(), 0.33026013056867354);

    let (candidates, migration1, migration2) = new_simple_migration_candidates();

    assert_eq!(
        rank_best_balancing_migration_candidates(&scheduler, candidates, 2),
        vec![migration2, migration1]
    );
}

fn rank_best_balancing_migration_candidates_topsis(
    scheduler: &Scheduler,
    candidates: &[MigrationCandidate],
    limit: usize,
) -> Result<Vec<Migration>, Error> {
    Ok(scheduler
        .score_best_balancing_migration_candidates_topsis(candidates, limit)?
        .into_iter()
        .map(|entry| entry.migration)
        .collect())
}

#[test]
fn test_score_best_balancing_migration_candidates_topsis_with_no_candidates() -> Result<(), Error> {
    let scheduler = new_homogeneous_cluster_scheduler();

    assert_eq!(
        rank_best_balancing_migration_candidates_topsis(&scheduler, &[], 2)?,
        vec![]
    );

    Ok(())
}

#[test]
fn test_score_best_balancing_migration_candidates_topsis_in_homogeneous_cluster(
) -> Result<(), Error> {
    let scheduler = new_homogeneous_cluster_scheduler();

    assert_imbalance(scheduler.node_imbalance(), 0.4893954724628247);

    let (candidates, migration1, migration2) = new_simple_migration_candidates();

    assert_eq!(
        rank_best_balancing_migration_candidates_topsis(&scheduler, &candidates, 2)?,
        vec![migration1, migration2]
    );

    Ok(())
}

#[test]
fn test_score_best_balancing_migration_candidates_topsis_in_heterogeneous_cluster(
) -> Result<(), Error> {
    let scheduler = new_heterogeneous_cluster_scheduler();

    assert_imbalance(scheduler.node_imbalance(), 0.33026013056867354);

    let (candidates, migration1, migration2) = new_simple_migration_candidates();

    assert_eq!(
        rank_best_balancing_migration_candidates_topsis(&scheduler, &candidates, 2)?,
        vec![migration1, migration2]
    );

    Ok(())
}

fn rank_nodes_to_start_resource(
    scheduler: &Scheduler,
    resource_stats: ResourceStats,
) -> Result<Vec<String>, Error> {
    let mut alternatives = scheduler.score_nodes_to_start_resource(resource_stats)?;

    alternatives.sort_by(|a, b| b.1.total_cmp(&a.1));

    Ok(alternatives
        .iter()
        .map(|alternative| alternative.0.to_string())
        .collect())
}

#[test]
fn test_score_homogeneous_nodes_to_start_resource() -> Result<(), Error> {
    let scheduler = new_homogeneous_cluster_scheduler();

    let heavy_memory_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 1.0,
        mem: 0,
        maxmem: 12 << 30,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, heavy_memory_resource_stats)?,
        vec!["node2", "node3", "node1"]
    );

    let heavy_cpu_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 12.0,
        mem: 0,
        maxmem: 0,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, heavy_cpu_resource_stats)?,
        vec!["node1", "node3", "node2"]
    );

    let unlimited_cpu_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 0.0,
        mem: 0,
        maxmem: 0,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, unlimited_cpu_resource_stats)?,
        vec!["node1", "node3", "node2"]
    );

    let combined_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 12.0,
        mem: 0,
        maxmem: 12 << 30,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, combined_resource_stats)?,
        vec!["node2", "node3", "node1"]
    );

    Ok(())
}

#[test]
fn test_score_heterogeneous_nodes_to_start_resource() -> Result<(), Error> {
    let scheduler = new_heterogeneous_cluster_scheduler();

    let heavy_memory_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 1.0,
        mem: 0,
        maxmem: 12 << 30,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, heavy_memory_resource_stats)?,
        vec!["node2", "node1", "node3"]
    );

    let heavy_cpu_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 12.0,
        mem: 0,
        maxmem: 0,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, heavy_cpu_resource_stats)?,
        vec!["node3", "node2", "node1"]
    );

    let unlimited_cpu_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 0.0,
        mem: 0,
        maxmem: 0,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, unlimited_cpu_resource_stats)?,
        vec!["node1", "node3", "node2"]
    );

    let combined_resource_stats = ResourceStats {
        cpu: 0.0,
        maxcpu: 12.0,
        mem: 0,
        maxmem: 12 << 30,
    };

    assert_eq!(
        rank_nodes_to_start_resource(&scheduler, combined_resource_stats)?,
        vec!["node2", "node1", "node3"]
    );

    Ok(())
}
