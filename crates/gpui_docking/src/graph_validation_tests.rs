use crate::*;

fn space(id: &str) -> DockSpaceId {
    DockSpaceId::new(id)
}

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn duplicate_item_graph() -> (DockGraph, DockNodeId, DockNodeId) {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
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
        active: 0,
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
        active: 0,
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
