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

    /// Returns the current cpu usage as a percentage.
    pub fn cpu_load(&self) -> f64 {
        self.cpu / self.maxcpu as f64
    }

    /// Returns the current memory usage as a percentage.
    pub fn mem_load(&self) -> f64 {
        self.mem as f64 / self.maxmem as f64
    }
}
