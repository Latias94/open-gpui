use crate::graph_test_support::{item, space};
use crate::model::DockLayoutBuilder;
use crate::*;

fn duplicate_item_graph() -> (DockGraph, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, right],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space("main"), root);
    (graph, left, right)
}

#[test]
fn graph_validation_accepts_reachable_canonical_graph() {
    let graph = DockGraph::default_editor_layout(
        space("main"),
        EditorDockLayoutSpec::new(["explorer"], ["editor"], ["terminal"]),
    );

    graph.validate().expect("default graph should validate");
    assert_eq!(graph.stored_node_count(), graph.reachable_node_count());
}

#[test]
fn canonical_graph_validation_rejects_unreachable_staged_nodes() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("root")],
        selected: Some(item("root")),
    });
    let orphan = graph.insert_node(DockNode::Tabs {
        items: vec![item("orphan")],
        selected: Some(item("orphan")),
    });
    graph.set_root(space("main"), root);

    graph
        .validate()
        .expect("reachable validation must allow the public insert-then-attach staging window");
    assert_eq!(
        graph.validate_canonical(),
        Err(DockGraphValidationError::UnreachableNode { node: orphan })
    );
}

#[test]
fn canonicalize_removes_unattached_staged_nodes_at_an_explicit_commit_boundary() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("root")],
        selected: Some(item("root")),
    });
    let staged = graph.insert_node(DockNode::Tabs {
        items: vec![item("staged")],
        selected: Some(item("staged")),
    });
    graph.set_root(space("main"), root);

    graph.canonicalize();

    assert!(graph.node(staged).is_none());
    graph
        .validate_canonical()
        .expect("explicit canonicalization should leave no unattached nodes");
}

#[test]
fn simplify_space_preserves_detached_node_identity_and_staging_authority() {
    let mut graph = DockGraph::new();
    let empty = graph.insert_node(DockNode::Tabs {
        items: Vec::new(),
        selected: None,
    });
    let survivor = graph.insert_node(DockNode::Tabs {
        items: vec![item("survivor")],
        selected: Some(item("survivor")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![empty, survivor],
        fractions: vec![0.5, 0.5],
    });
    let staged = graph.insert_node(DockNode::Tabs {
        items: vec![item("staged")],
        selected: Some(item("staged")),
    });
    graph.set_root(space("main"), root);

    graph.simplify_space(&space("main"));

    assert_eq!(graph.root(&space("main")), Some(survivor));
    assert!(
        graph.node(empty).is_some(),
        "local simplification must not invalidate a detached node identity"
    );
    assert!(
        graph.node(root).is_some(),
        "local simplification must not consume the previous root identity"
    );
    assert!(
        graph.node(staged).is_some(),
        "local simplification must not consume unrelated staging authority"
    );

    graph.set_root(space("secondary"), staged);
    assert_eq!(graph.root(&space("secondary")), Some(staged));
    graph.canonicalize();
    graph
        .validate_canonical()
        .expect("the preserved staged node should attach before the explicit commit boundary");
}

#[test]
fn checked_mutation_preserves_staged_dependencies_on_detached_nodes() {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let root = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, right],
        fractions: vec![0.5, 0.5],
    });
    graph.set_root(space("main"), root);

    let staged_leaf = graph.insert_node(DockNode::Tabs {
        items: vec![item("staged")],
        selected: Some(item("staged")),
    });
    let staged_wrapper = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Vertical,
        children: vec![root, staged_leaf],
        fractions: vec![0.5, 0.5],
    });

    assert!(
        graph
            .apply_op_checked(&DockOp::CloseItem {
                space: space("main"),
                item: item("left"),
            })
            .expect("closing the reachable item should commit")
    );

    assert_eq!(graph.root(&space("main")), Some(right));
    assert!(
        graph.node(root).is_some() && graph.node(left).is_some(),
        "the checked mutation must preserve detached nodes referenced by staging authority"
    );
    assert!(graph.node(staged_wrapper).is_some());
    graph
        .validate()
        .expect("reachable validation must continue to ignore unattached staging authority");
}

#[test]
fn layout_builder_prunes_nodes_removed_by_canonicalization() {
    let mut builder = DockLayoutBuilder::new();
    let left = builder.tabs(["left"]);
    let middle = builder.tabs(["middle"]);
    let right = builder.tabs(["right"]);
    let nested = builder.split_horizontal(middle, right, 0.5);
    let root = builder.split_horizontal(left, nested, 0.5);
    builder.set_root(space("main"), root);

    let graph = builder
        .try_build()
        .expect("canonical builder output should validate");

    assert!(graph.node(nested).is_none());
    assert_eq!(graph.stored_node_count(), graph.reachable_node_count());
}

#[test]
fn layout_builder_try_build_validates_finished_graph() {
    let mut builder = DockLayoutBuilder::new();
    let tabs = builder.tabs(["a", "a"]);
    builder.set_root(space("main"), tabs);

    assert_eq!(
        builder
            .try_build()
            .expect_err("checked builder finish should reject duplicate items"),
        DockGraphValidationError::DuplicateItemId {
            item: item("a"),
            first_tabs: tabs,
            duplicate_tabs: tabs,
        }
    );
}

#[test]
fn layout_builder_try_build_returns_canonical_valid_graph() {
    let mut builder = DockLayoutBuilder::new();
    let empty = builder.tabs(std::iter::empty::<&str>());
    let tabs = builder.tabs(["a"]);
    let root = builder.split_horizontal(empty, tabs, 0.5);
    builder.set_root(space("main"), root);

    let graph = builder
        .try_build()
        .expect("checked builder finish should simplify away empty tabs");

    assert_eq!(
        graph.collect_items_in_space(&space("main")),
        vec![item("a")]
    );
    graph.validate().expect("finished graph should validate");
}

#[test]
fn graph_validation_accepts_empty_central_region_without_empty_tabs() {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space("main"),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );

    graph
        .validate()
        .expect("empty central metadata should not require an empty tabs node");
}

#[test]
fn graph_validation_rejects_ordinary_empty_tabs() {
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: Vec::new(),
        selected: None,
    });
    graph.set_root(space("main"), tabs);

    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::EmptyTabs { tabs })
    );
}

#[test]
fn graph_validation_rejects_central_node_outside_root_subtree() {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![item("root")],
        selected: Some(item("root")),
    });
    let central = graph.insert_node(DockNode::Tabs {
        items: vec![item("central")],
        selected: Some(item("central")),
    });
    graph.set_root(space("main"), root);
    graph.set_central_region(space("main"), DockCentralRegion::with_node(central));

    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::CentralNodeNotInRoot {
            space: space("main"),
            node: central,
        })
    );
}

#[test]
fn graph_validation_rejects_duplicate_reachable_items() {
    let (graph, left, right) = duplicate_item_graph();

    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::DuplicateItemId {
            item: item("a"),
            first_tabs: left,
            duplicate_tabs: right,
        })
    );
}

#[test]
fn graph_validation_rejects_shared_reachable_nodes() {
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    graph.set_root(space("left"), tabs);
    graph.set_root(space("right"), tabs);

    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::DuplicateNodeReference { node: tabs })
    );
}

#[test]
fn graph_validation_rejects_malformed_floating_containers() {
    let mut graph = DockGraph::new();
    let tabs = graph.insert_node(DockNode::Tabs {
        items: vec![item("floating")],
        selected: Some(item("floating")),
    });
    graph
        .floating_containers_mut(space("main"))
        .push(DockFloatingContainer {
            node: tabs,
            bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
        });

    assert_eq!(
        graph.validate(),
        Err(DockGraphValidationError::FloatingContainerNodeNotFloating {
            space: space("main"),
            floating: tabs,
        })
    );
}

#[test]
fn controller_builder_try_build_rejects_invalid_custom_graph() {
    let (graph, left, right) = duplicate_item_graph();

    assert_eq!(
        DockController::builder(space("main"))
            .graph(graph)
            .try_build()
            .expect_err("try_build should reject duplicate runtime items"),
        DockGraphValidationError::DuplicateItemId {
            item: item("a"),
            first_tabs: left,
            duplicate_tabs: right,
        }
    );
}
