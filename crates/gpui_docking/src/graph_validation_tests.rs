use crate::graph_test_support::{item, space};
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
}

#[test]
fn layout_builder_try_build_validates_finished_graph() {
    let mut builder = DockLayoutBuilder::new();
    let tabs = builder.tabs(["a", "a"], 0);
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
    let empty = builder.tabs(std::iter::empty::<&str>(), 0);
    let tabs = builder.tabs(["a"], 0);
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
