use proxmox_resource_scheduling::{
    node::NodeStats,
    resource::{Resource, ResourcePlacement, ResourceState, ResourceStats},
    usage::Usage,
};

#[test]
fn test_no_duplicate_nodes() {
    let mut usage = Usage::new();

    assert!(
        usage
            .add_node("node1".to_owned(), NodeStats::default())
            .is_ok()
    );

    assert!(
        usage
            .add_node("node1".to_owned(), NodeStats::default())
            .is_err(),
        "cluster usage does allow duplicate node entries"
    );
}

#[test]
fn test_no_duplicate_resources() {
    let mut usage = Usage::new();

    assert!(
        usage
            .add_node("node1".to_owned(), NodeStats::default())
            .is_ok()
    );

    let placement = ResourcePlacement::Stationary {
        current_node: "node1".to_owned(),
    };
    let resource = Resource::new(ResourceStats::default(), ResourceState::Stopped, placement);

    assert!(
        usage
            .add_resource("vm:101".to_owned(), resource.clone())
            .is_ok()
    );

    assert!(
        usage.add_resource("vm:101".to_owned(), resource).is_err(),
        "cluster usage does allow duplicate resource entries"
    );
}

fn assert_add_node(usage: &mut Usage, nodename: &str) {
    assert!(
        usage
            .add_node(nodename.to_owned(), NodeStats::default())
            .is_ok()
    );

    assert!(
        usage.get_node(nodename).is_some(),
        "node '{nodename}' was not added"
    );
}

fn assert_add_resource(usage: &mut Usage, sid: &str, resource: Resource) {
    assert!(usage.add_resource(sid.to_owned(), resource).is_ok());

    assert!(
        usage.get_resource(sid).is_some(),
        "resource '{sid}' was not added"
    );
}

#[test]
#[allow(deprecated)]
fn test_add_resource_usage_to_node() {
    let mut usage = Usage::new();

    assert_add_node(&mut usage, "node1");
    assert_add_node(&mut usage, "node2");
    assert_add_node(&mut usage, "node3");

    assert!(
        usage
            .add_resource_usage_to_node("node1", "vm:101", ResourceStats::default())
            .is_ok()
    );

    assert!(
        usage
            .add_resource_usage_to_node("node4", "vm:101", ResourceStats::default())
            .is_err(),
        "add_resource_usage_to_node() allows adding non-existent nodes"
    );

    assert!(
        usage
            .add_resource_usage_to_node("node2", "vm:101", ResourceStats::default())
            .is_ok()
    );

    assert!(
        usage
            .add_resource_usage_to_node("node3", "vm:101", ResourceStats::default())
            .is_err(),
        "add_resource_usage_to_node() allows adding resources to more than two nodes"
    );
}

#[test]
fn test_add_remove_stationary_resource() {
    let mut usage = Usage::new();

    let (sid, nodename) = ("vm:101", "node1");

    assert_add_node(&mut usage, nodename);

    let placement = ResourcePlacement::Stationary {
        current_node: nodename.to_owned(),
    };
    let resource = Resource::new(ResourceStats::default(), ResourceState::Stopped, placement);

    assert_add_resource(&mut usage, sid, resource);

    if let Some(node) = usage.get_node(nodename) {
        assert!(
            node.contains_resource(sid),
            "resource '{sid}' was not added from node '{nodename}'"
        );
    }

    usage.remove_resource(sid);

    assert!(
        usage.get_resource(sid).is_none(),
        "resource '{sid}' was not removed"
    );

    if let Some(node) = usage.get_node(nodename) {
        assert!(
            !node.contains_resource(sid),
            "resource '{sid}' was not removed from node '{nodename}'"
        );
    }
}

#[test]
fn test_add_remove_moving_resource() {
    let mut usage = Usage::new();

    let (sid, current_nodename, target_nodename) = ("vm:101", "node1", "node2");

    assert_add_node(&mut usage, current_nodename);
    assert_add_node(&mut usage, target_nodename);

    let placement = ResourcePlacement::Moving {
        current_node: current_nodename.to_owned(),
        target_node: target_nodename.to_owned(),
    };
    let resource = Resource::new(ResourceStats::default(), ResourceState::Stopped, placement);

    assert_add_resource(&mut usage, sid, resource);

    if let Some(current_node) = usage.get_node(current_nodename) {
        assert!(
            current_node.contains_resource(sid),
            "resource '{sid}' was not added to current node '{current_nodename}'"
        );
    }

    if let Some(target_node) = usage.get_node(target_nodename) {
        assert!(
            target_node.contains_resource(sid),
            "resource '{sid}' was not added to target node '{target_nodename}'"
        );
    }

    usage.remove_resource(sid);

    if let Some(current_node) = usage.get_node(current_nodename) {
        assert!(
            !current_node.contains_resource(sid),
            "resource '{sid}' was not removed from current node '{current_nodename}'"
        );
    }

    if let Some(target_node) = usage.get_node(target_nodename) {
        assert!(
            !target_node.contains_resource(sid),
            "resource '{sid}' was not removed from target node '{target_nodename}'"
        );
    }
}
