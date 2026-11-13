use crate::legacy::detached_tree::PreparedDetach;
use crate::legacy::graph_assembly::{CanonicalGraphError, CanonicalNode, DockGraphAssembly};
use crate::legacy::payload_ref::{DockPayloadKind, DockPayloadRefError};
use crate::legacy::{
    CanonicalGraphAuthority, CanonicalGraphRevisionChange, CanonicalGraphVersion, DockFloatingId,
    DockGroupId, DockItemId, DockLayoutRect, DockSpaceId, SplitAxis,
};

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn rect() -> DockLayoutRect {
    DockLayoutRect {
        x: 12.0,
        y: 18.0,
        width: 320.0,
        height: 220.0,
    }
}

fn version(authority_epoch: u64, topology_revision: u64) -> CanonicalGraphVersion {
    CanonicalGraphVersion::new(authority_epoch, topology_revision)
}

#[test]
fn canonical_assembly_publishes_binary_graph_and_reverse_indexes() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let editor = assembly
        .add_group("editor", [item("A"), item("B")], item("B"))
        .unwrap();
    let inspector = assembly
        .add_group("inspector", [item("C")], item("C"))
        .unwrap();
    let root = assembly
        .add_branch(SplitAxis::Horizontal, editor, inspector, 0.25)
        .unwrap();
    assembly.set_root("main", root).unwrap();
    assembly
        .set_central_node("main", Some(editor), true, false)
        .unwrap();

    let graph = assembly
        .publish(version(7, 3))
        .expect("assembly should publish");
    graph.validate().expect("published graph should validate");
    assert_eq!(graph.version(), version(7, 3));
    assert_eq!(
        graph
            .index()
            .group_for_item(&item("A"))
            .map(DockGroupId::as_str),
        Some("editor")
    );
    assert_eq!(
        graph
            .index()
            .space_for_node(graph.index().node_for_item(&item("C")).unwrap()),
        Some(&DockSpaceId::from("main"))
    );
    let root = graph
        .space(&DockSpaceId::from("main"))
        .unwrap()
        .root
        .unwrap();
    assert!(
        matches!(graph.node(root), Some(CanonicalNode::Branch { ratio, .. }) if (*ratio - 0.25).abs() < f32::EPSILON)
    );
}

#[test]
fn empty_central_space_has_policy_without_placeholder_node() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("empty").unwrap();
    assembly
        .set_central_node("empty", None, true, true)
        .unwrap();

    let graph = assembly
        .publish(version(1, 0))
        .expect("empty central space should publish");
    let space = graph.space(&DockSpaceId::from("empty")).unwrap();
    assert!(space.root.is_none());
    assert!(space.central.node.is_none());
    assert!(space.central.keep_alive_when_empty);
    assert!(space.central.passthrough_when_empty);
    assert!(graph.nodes().is_empty());
}

#[test]
fn payload_ref_and_prepared_detach_preserve_group_identity() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let group = assembly
        .add_group("editor", [item("A"), item("B")], item("B"))
        .unwrap();
    assembly.set_root("main", group).unwrap();
    assembly
        .set_central_node("main", Some(group), true, false)
        .unwrap();
    let graph = assembly.publish(version(1, 0)).unwrap();

    let payload = graph
        .payload_ref("main", "A", DockPayloadKind::WholeGroup)
        .unwrap();
    let prepared = PreparedDetach::prepare(&graph, payload).unwrap();
    assert_eq!(
        prepared.payload.source_group.as_ref().unwrap().as_str(),
        "editor"
    );
    assert_eq!(prepared.source_node, group_node(&graph, "editor"));
    assert_eq!(prepared.expected_central_node, Some(prepared.source_node));
}

#[test]
fn assembly_rejects_invalid_ratio_unattached_nodes_and_duplicate_items() {
    let mut invalid_ratio = DockGraphAssembly::new();
    invalid_ratio.add_space("main").unwrap();
    let a = invalid_ratio
        .add_group("a", [item("A")], item("A"))
        .unwrap();
    let b = invalid_ratio
        .add_group("b", [item("B")], item("B"))
        .unwrap();
    assert!(matches!(
        invalid_ratio.add_branch(SplitAxis::Horizontal, a, b, f32::NAN),
        Err(CanonicalGraphError::InvalidRatio { ratio }) if ratio.is_nan()
    ));

    let mut unattached = DockGraphAssembly::new();
    unattached.add_space("main").unwrap();
    unattached
        .add_group("orphan", [item("A")], item("A"))
        .unwrap();
    assert!(matches!(
        unattached.publish(version(1, 0)),
        Err(CanonicalGraphError::UnattachedAssemblyNode { .. })
    ));

    let mut duplicate = DockGraphAssembly::new();
    duplicate.add_space("main").unwrap();
    let first = duplicate
        .add_group("first", [item("A")], item("A"))
        .unwrap();
    let second = duplicate
        .add_group("second", [item("A")], item("A"))
        .unwrap();
    let root = duplicate
        .add_branch(SplitAxis::Horizontal, first, second, 0.5)
        .unwrap();
    duplicate.set_root("main", root).unwrap();
    assert!(matches!(
        duplicate.publish(version(1, 0)),
        Err(CanonicalGraphError::DuplicateItem { item: duplicate_item })
            if duplicate_item == item("A")
    ));
}

#[test]
fn assembly_rejects_empty_durable_ids() {
    let mut empty_space = DockGraphAssembly::new();
    assert_eq!(
        empty_space.add_space(""),
        Err(CanonicalGraphError::EmptySpaceId)
    );

    let mut empty_group = DockGraphAssembly::new();
    empty_group.add_space("main").unwrap();
    assert_eq!(
        empty_group.add_group("", [item("A")], item("A")),
        Err(CanonicalGraphError::EmptyGroupId)
    );

    let mut empty_floating = DockGraphAssembly::new();
    empty_floating.add_space("main").unwrap();
    let group = empty_floating
        .add_group("floating-group", [item("F")], item("F"))
        .unwrap();
    assert_eq!(
        empty_floating.add_floating_root("", "main", group, rect(), 0),
        Err(CanonicalGraphError::EmptyFloatingId)
    );
}

#[test]
fn failed_publication_does_not_consume_authority_revision() {
    let mut authority = CanonicalGraphAuthority::new();
    let mut invalid = DockGraphAssembly::new();
    invalid.add_space("main").unwrap();
    invalid.add_group("orphan", [item("A")], item("A")).unwrap();
    assert!(matches!(
        invalid.publish(&mut authority),
        Err(CanonicalGraphError::UnattachedAssemblyNode { .. })
    ));

    let mut valid = DockGraphAssembly::new();
    valid.add_space("main").unwrap();
    let group = valid.add_group("editor", [item("A")], item("A")).unwrap();
    valid.set_root("main", group).unwrap();
    let graph = valid.publish(&mut authority).unwrap();
    assert_eq!(graph.version().topology_revision(), 1);
    assert_eq!(graph.version().allocator_revision(), 1);

    let after_selection = authority.record_selection_change();
    assert_eq!(after_selection.allocator_revision(), 1);
    let after_topology = authority.record_topology_change();
    assert_eq!(after_topology.allocator_revision(), 2);
}

#[test]
fn authority_advances_only_declared_revision_domains() {
    let mut authority = CanonicalGraphAuthority::new();
    let before = authority.current();
    let change = CanonicalGraphRevisionChange::new(true, false, true, true);
    let expected = authority
        .next_change(change)
        .expect("declared revision domains should have headroom");

    assert_eq!(expected.topology_revision(), before.topology_revision() + 1);
    assert_eq!(expected.allocator_revision(), before.allocator_revision());
    assert_eq!(
        expected.selection_revision(),
        before.selection_revision() + 1
    );
    assert_eq!(
        expected.interaction_revision(),
        before.interaction_revision() + 1
    );
    assert_eq!(authority.current(), before);
    assert_eq!(authority.record_change(change), expected);
    assert_eq!(authority.current(), expected);
}

#[test]
fn canonical_authorities_receive_distinct_epochs() {
    let first = CanonicalGraphAuthority::new();
    let second = CanonicalGraphAuthority::new();
    assert_ne!(
        first.current().authority_epoch(),
        second.current().authority_epoch()
    );
}

#[test]
fn floating_root_is_first_class_and_payload_ownership_is_indexed() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let floating = assembly
        .add_group("floating-group", [item("F")], item("F"))
        .unwrap();
    assembly
        .add_floating_root("float-1", "main", floating, rect(), 7)
        .unwrap();
    let graph = assembly.publish(version(1, 0)).unwrap();
    let payload = graph
        .payload_ref("main", "F", DockPayloadKind::WholeFloating)
        .unwrap();
    assert_eq!(
        payload.source_floating.as_ref().map(|id| id.as_str()),
        Some("float-1")
    );
    assert_eq!(
        graph
            .index()
            .floating_for_item(&item("F"))
            .map(|id| id.as_str()),
        Some("float-1")
    );
    assert_eq!(
        payload.source_node,
        graph.floating_roots().values().next().unwrap().root
    );
}

#[test]
fn publication_canonicalizes_floating_order_by_z_then_id() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let first = assembly
        .add_group("first-floating", [item("F1")], item("F1"))
        .unwrap();
    let second = assembly
        .add_group("second-floating", [item("F2")], item("F2"))
        .unwrap();
    assembly
        .add_floating_root("z-later", "main", first, rect(), 20)
        .unwrap();
    assembly
        .add_floating_root("z-first", "main", second, rect(), 10)
        .unwrap();

    let mut authority = CanonicalGraphAuthority::new();
    let graph = assembly.publish(&mut authority).unwrap();
    assert_eq!(
        graph
            .space(&DockSpaceId::from("main"))
            .unwrap()
            .floating_roots,
        vec![
            DockFloatingId::from("z-first"),
            DockFloatingId::from("z-later")
        ]
    );
    graph.validate().unwrap();
}

#[test]
fn payload_proof_rejects_an_identical_graph_from_another_authority() {
    fn graph(version: CanonicalGraphVersion) -> crate::legacy::graph_assembly::CanonicalDockGraph {
        let mut assembly = DockGraphAssembly::new();
        assembly.add_space("main").unwrap();
        let group = assembly
            .add_group("editor", [item("A")], item("A"))
            .unwrap();
        assembly.set_root("main", group).unwrap();
        assembly.publish(version).unwrap()
    }

    let original = graph(version(41, 0));
    let replacement = graph(version(42, 0));
    let payload = original
        .payload_ref("main", "A", DockPayloadKind::WholeGroup)
        .unwrap();

    assert_eq!(
        replacement.validate_payload_ref(&payload),
        Err(DockPayloadRefError::StaleVersion {
            expected: version(41, 0),
            actual: version(42, 0),
        })
    );
}

#[test]
fn central_policy_accepts_only_a_group_in_the_main_root() {
    let mut branch_central = DockGraphAssembly::new();
    branch_central.add_space("main").unwrap();
    let first = branch_central
        .add_group("first", [item("A")], item("A"))
        .unwrap();
    let second = branch_central
        .add_group("second", [item("B")], item("B"))
        .unwrap();
    let branch = branch_central
        .add_branch(SplitAxis::Horizontal, first, second, 0.5)
        .unwrap();
    branch_central.set_root("main", branch).unwrap();
    branch_central
        .set_central_node("main", Some(branch), true, false)
        .unwrap();
    assert!(matches!(
        branch_central.publish(version(1, 0)),
        Err(CanonicalGraphError::CentralNodeNotGroup { .. })
    ));

    let mut floating_central = DockGraphAssembly::new();
    floating_central.add_space("main").unwrap();
    let main = floating_central
        .add_group("main-group", [item("M")], item("M"))
        .unwrap();
    let floating = floating_central
        .add_group("floating-group", [item("F")], item("F"))
        .unwrap();
    floating_central.set_root("main", main).unwrap();
    floating_central
        .add_floating_root("float-1", "main", floating, rect(), 0)
        .unwrap();
    floating_central
        .set_central_node("main", Some(floating), true, false)
        .unwrap();
    assert!(matches!(
        floating_central.publish(version(2, 0)),
        Err(CanonicalGraphError::CentralNodeOutsideSpace { .. })
    ));
}

#[test]
fn assembly_rejects_a_child_reused_by_two_branches() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let shared = assembly
        .add_group("shared", [item("A")], item("A"))
        .unwrap();
    let left = assembly.add_group("left", [item("B")], item("B")).unwrap();
    let right = assembly.add_group("right", [item("C")], item("C")).unwrap();
    let first = assembly
        .add_branch(SplitAxis::Horizontal, shared, left, 0.5)
        .unwrap();
    let second = assembly
        .add_branch(SplitAxis::Horizontal, shared, right, 0.5)
        .unwrap();
    let root = assembly
        .add_branch(SplitAxis::Vertical, first, second, 0.5)
        .unwrap();
    assembly.set_root("main", root).unwrap();

    assert!(matches!(
        assembly.publish(version(1, 0)),
        Err(CanonicalGraphError::SharedAssemblyNode { .. })
    ));
}

#[test]
fn retained_subtree_payload_binds_to_the_explicit_subtree_root() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let first = assembly.add_group("first", [item("A")], item("A")).unwrap();
    let second = assembly
        .add_group("second", [item("B")], item("B"))
        .unwrap();
    let root = assembly
        .add_branch(SplitAxis::Horizontal, first, second, 0.5)
        .unwrap();
    assembly.set_root("main", root).unwrap();
    let graph = assembly.publish(version(1, 0)).unwrap();
    let root = graph
        .space(&DockSpaceId::from("main"))
        .unwrap()
        .root
        .unwrap();

    let payload = graph.subtree_payload_ref("main", root, "B").unwrap();
    assert_eq!(payload.kind, DockPayloadKind::RetainedSubtree);
    assert_eq!(payload.source_node, root);
    graph.validate_payload_ref(&payload).unwrap();
}

#[test]
fn canonical_graph_rejects_invalid_runtime_mru_and_duplicate_floating_order() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let main = assembly
        .add_group("main-group", [item("A")], item("A"))
        .unwrap();
    assembly.set_root("main", main).unwrap();
    let mut graph = assembly.publish(version(1, 0)).unwrap();
    graph
        .group_mut_for_test(&DockGroupId::from("main-group"))
        .unwrap()
        .mru = vec![item("missing")];
    assert!(matches!(
        graph.validate(),
        Err(CanonicalGraphError::MruItemMissing { .. })
    ));

    let mut duplicate_order = DockGraphAssembly::new();
    duplicate_order.add_space("main").unwrap();
    let first = duplicate_order
        .add_group("first-floating", [item("F1")], item("F1"))
        .unwrap();
    let second = duplicate_order
        .add_group("second-floating", [item("F2")], item("F2"))
        .unwrap();
    duplicate_order
        .add_floating_root("float-1", "main", first, rect(), 5)
        .unwrap();
    duplicate_order
        .add_floating_root("float-2", "main", second, rect(), 5)
        .unwrap();
    assert!(matches!(
        duplicate_order.publish(version(2, 0)),
        Err(CanonicalGraphError::DuplicateFloatingZOrder { z_order: 5, .. })
    ));
}

fn group_node(
    graph: &crate::legacy::graph_assembly::CanonicalDockGraph,
    group: &str,
) -> crate::legacy::CanonicalNodeId {
    graph
        .index()
        .node_for_group(&DockGroupId::from(group))
        .unwrap()
}
