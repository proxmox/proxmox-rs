use anyhow::Error;
use proxmox_resource_scheduling::{
    node::NodeStats,
    resource::ResourceStats,
    scheduler::{NodeUsage, Scheduler},
};

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
