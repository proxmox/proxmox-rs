//! Code dealing with tags in VMs.

use crate::{LxcEntry, VmEntry};

fn tag_iter(s: Option<&str>) -> impl Iterator<Item = &str> + Send + Sync + '_ {
    s.into_iter().flat_map(|s| s.split(';'))
}

impl VmEntry {
    pub fn tags(&self) -> impl Iterator<Item = &str> + Send + Sync + '_ {
        tag_iter(self.tags.as_deref())
    }
}

impl LxcEntry {
    pub fn tags(&self) -> impl Iterator<Item = &str> + Send + Sync + '_ {
        tag_iter(self.tags.as_deref())
    }
}
