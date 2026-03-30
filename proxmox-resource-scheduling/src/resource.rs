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
