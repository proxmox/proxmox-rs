use anyhow::{bail, Error};

use std::collections::HashMap;

use crate::{
    node::{Node, NodeStats},
    resource::{Resource, ResourcePlacement, ResourceState, ResourceStats},
    scheduler::{NodeUsage, Scheduler},
};

/// The state of the usage in the cluster.
///
/// The cluster usage represents the current state of the assignments between nodes and resources
/// and their usage statistics. A resource can be placed on these nodes according to their
/// placement state. See [`crate::resource::Resource`] for more information.
///
/// The cluster usage state can be used to build a current state for the [`Scheduler`].
#[derive(Default)]
pub struct Usage {
    nodes: HashMap<String, Node>,
    resources: HashMap<String, Resource>,
}

/// An aggregator for the [`Usage`] maps the cluster usage to node usage statistics that are
/// relevant for the scheduler.
pub trait UsageAggregator {
    fn aggregate(usage: &Usage) -> Vec<NodeUsage>;
}

impl Usage {
    /// Instantiate an empty cluster usage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the cluster usage.
    ///
    /// This method fails if a node with the same `nodename` already exists.
    pub fn add_node(&mut self, nodename: String, stats: NodeStats) -> Result<(), Error> {
        if self.nodes.contains_key(&nodename) {
            bail!("node '{nodename}' already exists");
        }

        self.nodes.insert(nodename, Node::new(stats));

        Ok(())
    }

    /// Remove a node from the cluster usage.
    pub fn remove_node(&mut self, nodename: &str) {
        if let Some(node) = self.nodes.remove(nodename) {
            node.resources_iter().for_each(|sid| {
                if let Some(resource) = self.resources.get_mut(sid)
                    && resource.remove_node(nodename)
                {
                    self.resources.remove(sid);
                }
            });
        }
    }

    /// Returns a reference to the [`Node`] with the identifier `nodename`.
    pub fn get_node(&self, nodename: &str) -> Option<&Node> {
        self.nodes.get(nodename)
    }

    /// Returns an iterator for the cluster usage's nodes.
    pub fn nodes_iter(&self) -> impl Iterator<Item = (&String, &Node)> {
        self.nodes.iter()
    }

    /// Returns an iterator for the cluster usage's nodes.
    pub fn nodenames_iter(&self) -> impl Iterator<Item = &String> {
        self.nodes.keys()
    }

    /// Returns whether the node with the identifier `nodename` is present in the cluster usage.
    pub fn contains_node(&self, nodename: &str) -> bool {
        self.nodes.contains_key(nodename)
    }

    /// Add `resource` with identifier `sid` to cluster usage.
    ///
    /// This method fails if a resource with the same `sid` already exists or the resource's nodes
    /// do not exist in the cluster usage.
    pub fn add_resource(&mut self, sid: String, resource: Resource) -> Result<(), Error> {
        if self.resources.contains_key(&sid) {
            bail!("resource '{sid}' already exists");
        }

        match resource.placement() {
            ResourcePlacement::Stationary { current_node } => {
                match self.nodes.get_mut(current_node) {
                    Some(current_node) => {
                        current_node.add_resource(sid.to_owned());
                    }
                    _ => bail!("current node for resource '{sid}' does not exist"),
                }
            }
            ResourcePlacement::Moving {
                current_node,
                target_node,
            } => {
                if current_node == target_node {
                    bail!("resource '{sid}' has the same current and target node");
                }

                match self.nodes.get_disjoint_mut([current_node, target_node]) {
                    [Some(current_node), Some(target_node)] => {
                        current_node.add_resource(sid.to_owned());
                        target_node.add_resource(sid.to_owned());
                    }
                    _ => bail!("nodes for resource '{sid}' do not exist"),
                }
            }
        }

        self.resources.insert(sid, resource);

        Ok(())
    }

    /// Add `stats` from resource with identifier `sid` to node `nodename` in cluster usage.
    ///
    /// For the first call, the resource is assumed to be started and stationary on the given node.
    /// If there was no intermediate call to remove the resource, the second call will assume that
    /// the given node is the target node and the resource is being moved there. The second call
    /// will ignore the value of `stats`.
    #[deprecated = "only for backwards compatibility, use add_resource(...) instead"]
    pub fn add_resource_usage_to_node(
        &mut self,
        nodename: &str,
        sid: &str,
        stats: ResourceStats,
    ) -> Result<(), Error> {
        if let Some(resource) = self.resources.remove(sid) {
            match resource.placement() {
                ResourcePlacement::Stationary { current_node } => {
                    let placement = ResourcePlacement::Moving {
                        current_node: current_node.to_owned(),
                        target_node: nodename.to_owned(),
                    };
                    let new_resource = Resource::new(resource.stats(), resource.state(), placement);

                    if let Err(err) = self.add_resource(sid.to_owned(), new_resource) {
                        self.add_resource(sid.to_owned(), resource)?;

                        bail!(err);
                    }

                    Ok(())
                }
                ResourcePlacement::Moving { target_node, .. } => {
                    bail!("resource '{sid}' is already moving to target node '{target_node}'")
                }
            }
        } else {
            let placement = ResourcePlacement::Stationary {
                current_node: nodename.to_owned(),
            };
            let resource = Resource::new(stats, ResourceState::Started, placement);

            self.add_resource(sid.to_owned(), resource)
        }
    }

    /// Remove resource with identifier `sid` from cluster usage.
    pub fn remove_resource(&mut self, sid: &str) {
        if let Some(resource) = self.resources.remove(sid) {
            match resource.placement() {
                ResourcePlacement::Stationary { current_node } => {
                    if let Some(current_node) = self.nodes.get_mut(current_node) {
                        current_node.remove_resource(sid);
                    }
                }
                ResourcePlacement::Moving {
                    current_node,
                    target_node,
                } => {
                    if let Some(current_node) = self.nodes.get_mut(current_node) {
                        current_node.remove_resource(sid);
                    }

                    if let Some(target_node) = self.nodes.get_mut(target_node) {
                        target_node.remove_resource(sid);
                    }
                }
            }
        }
    }

    /// Returns a reference to the [`Resource`] with the identifier `sid`.
    pub fn get_resource(&self, sid: &str) -> Option<&Resource> {
        self.resources.get(sid)
    }

    /// Returns an iterator for the cluster usage's resources.
    pub fn resources_iter(&self) -> impl Iterator<Item = (&String, &Resource)> {
        self.resources.iter()
    }

    /// Use the current cluster usage as a base for a scheduling action.
    pub fn to_scheduler<F: UsageAggregator>(&self) -> Scheduler {
        let node_usages = F::aggregate(self);

        Scheduler::from_nodes(node_usages)
    }
}
