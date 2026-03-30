use std::{iter::Sum, ops::Add};

/// Usage statistics for a resource.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct ResourceStats {
    /// CPU utilization in CPU cores.
    pub cpu: f64,
    /// Number of assigned CPUs or CPU limit.
    pub maxcpu: f64,
    /// Used memory in bytes.
    pub mem: usize,
    /// Maximum assigned memory in bytes.
    pub maxmem: usize,
}

impl Add for ResourceStats {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            cpu: self.cpu + other.cpu,
            maxcpu: self.maxcpu + other.maxcpu,
            mem: self.mem + other.mem,
            maxmem: self.maxmem + other.maxmem,
        }
    }
}

impl Sum for ResourceStats {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |a, b| a + b)
    }
}

/// Execution state of a resource.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ResourceState {
    /// The resource is stopped.
    Stopped,
    /// The resource is scheduled to start.
    Starting,
    /// The resource is started and currently running.
    Started,
}

/// Placement of a resource.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ResourcePlacement {
    /// The resource is on `current_node`.
    Stationary { current_node: String },
    /// The resource is being moved from `current_node` to `target_node`.
    Moving {
        current_node: String,
        target_node: String,
    },
}

/// A resource in the cluster context.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Resource {
    /// The usage statistics of the resource.
    stats: ResourceStats,
    /// The execution state of the resource.
    state: ResourceState,
    /// The placement of the resource.
    placement: ResourcePlacement,
}

impl Resource {
    pub fn new(stats: ResourceStats, state: ResourceState, placement: ResourcePlacement) -> Self {
        Self {
            stats,
            state,
            placement,
        }
    }

    /// Handles the external removal of a node.
    ///
    /// Returns whether the resource does not have any node left.
    pub fn remove_node(&mut self, nodename: &str) -> bool {
        match &self.placement {
            ResourcePlacement::Stationary { current_node } => current_node == nodename,
            ResourcePlacement::Moving {
                current_node,
                target_node,
            } => {
                if current_node == nodename {
                    self.placement = ResourcePlacement::Stationary {
                        current_node: target_node.to_owned(),
                    };
                } else if target_node == nodename {
                    self.placement = ResourcePlacement::Stationary {
                        current_node: current_node.to_owned(),
                    };
                }

                false
            }
        }
    }

    pub fn state(&self) -> ResourceState {
        self.state
    }

    pub fn stats(&self) -> ResourceStats {
        self.stats
    }

    pub fn placement(&self) -> &ResourcePlacement {
        &self.placement
    }
}
