use std::collections::HashSet;

use crate::resource::ResourceStats;

/// Usage statistics of a node.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct NodeStats {
    /// CPU utilization in CPU cores.
    pub cpu: f64,
    /// Total number of CPU cores.
    pub maxcpu: usize,
    /// Used memory in bytes.
    pub mem: usize,
    /// Total memory in bytes.
    pub maxmem: usize,
}

impl NodeStats {
    /// Adds the resource stats to the node stats as if the resource has started on the node.
    pub fn add_started_resource(&mut self, resource_stats: &ResourceStats) {
        // a maxcpu value of `0.0` means no cpu usage limit on the node
        let resource_cpu = if resource_stats.maxcpu == 0.0 {
            self.maxcpu as f64
        } else {
            resource_stats.maxcpu
        };

        self.cpu += resource_cpu;
        self.mem += resource_stats.maxmem;
    }

    /// Adds the resource stats to the node stats as if the resource is running on the node.
    pub fn add_running_resource(&mut self, resource_stats: &ResourceStats) {
        self.cpu += resource_stats.cpu;
        self.mem += resource_stats.mem;
    }

    /// Removes the resource stats from the node stats as if the resource is not running on the node.
    pub fn remove_running_resource(&mut self, resource_stats: &ResourceStats) {
        self.cpu = f64::max(0.0, self.cpu - resource_stats.cpu);
        self.mem = self.mem.saturating_sub(resource_stats.mem);
    }

    /// Returns the current cpu usage as a percentage.
    pub fn cpu_load(&self) -> f64 {
        self.cpu / self.maxcpu as f64
    }

    /// Returns the current memory usage as a percentage.
    pub fn mem_load(&self) -> f64 {
        self.mem as f64 / self.maxmem as f64
    }

    /// Returns a combined node usage as a percentage.
    pub fn load(&self) -> f64 {
        (self.cpu_load() + self.mem_load()) / 2.0
    }
}

/// A node in the cluster context.
#[derive(Clone, Debug)]
pub struct Node {
    /// Base stats of the node.
    stats: NodeStats,
    /// The identifiers of the resources assigned to the node.
    resources: HashSet<String>,
}

impl Node {
    pub fn new(stats: NodeStats) -> Self {
        Self {
            stats,
            resources: HashSet::new(),
        }
    }

    pub fn add_resource(&mut self, sid: String) -> bool {
        self.resources.insert(sid)
    }

    pub fn remove_resource(&mut self, sid: &str) -> bool {
        self.resources.remove(sid)
    }

    pub fn stats(&self) -> NodeStats {
        self.stats
    }

    pub fn resources_iter(&self) -> impl Iterator<Item = &str> {
        self.resources.iter().map(String::as_str)
    }

    pub fn contains_resource(&self, sid: &str) -> bool {
        self.resources.contains(sid)
    }
}
