use crate::{DockGraph, DockNode, DockSpaceId, SplitAxis};
use std::collections::BTreeSet;

pub(crate) fn assert_space_items(
    graph: &DockGraph,
    space: impl Into<DockSpaceId>,
    expected: &[&str],
) {
    let space = space.into();
    let actual = graph
        .collect_items_in_space(&space)
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "space {space} item order mismatch");
}

pub(crate) fn assert_unique_items(graph: &DockGraph) {
    let mut seen = BTreeSet::new();
    for space in graph.spaces() {
        for item in graph.collect_items_in_space(&space) {
            assert!(
                seen.insert(item.clone()),
                "dockspace scenario graph duplicated item {item}"
            );
        }
    }
}

pub(crate) fn assert_root_horizontal_split(
    graph: &DockGraph,
    space: impl Into<DockSpaceId>,
    child_count: usize,
) {
    let space = space.into();
    let root = graph
        .root(&space)
        .unwrap_or_else(|| panic!("space {space} should have a root"));
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph
        .node(root)
        .unwrap_or_else(|| panic!("space {space} root should exist"))
    else {
        panic!("space {space} root should be a split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children.len(), child_count);
    assert_eq!(fractions.len(), child_count);
}

pub(crate) fn assert_layout_roundtrip_preserves_space_items(graph: &DockGraph) {
    let layout = graph.export_layout();
    let imported = DockGraph::import_layout(&layout).expect("scenario layout should roundtrip");
    for space in graph.spaces() {
        assert_eq!(
            imported.collect_items_in_space(&space),
            graph.collect_items_in_space(&space),
            "space {space} layout roundtrip item mismatch"
        );
    }
}
