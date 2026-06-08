use crate::*;

fn space() -> DockSpaceId {
    DockSpaceId::new("main")
}

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn root_tabs_graph(items: &[&str]) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: items.iter().copied().map(item).collect(),
        active: 0,
    });
    graph.set_root(space(), root);
    (graph, root)
}

#[test]
fn compute_layout_repairs_mismatched_fraction_lengths_without_truncating_children() {
    let mut graph = DockGraph::new();
    let tabs_a = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        active: 0,
    });
    let tabs_b = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        active: 0,
    });
    let tabs_c = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        active: 0,
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![tabs_a, tabs_b, tabs_c],
        fractions: vec![2.0, 1.0],
    });

    let mut layout = std::collections::HashMap::new();
    graph.compute_layout(split, dock_bounds(0.0, 0.0, 400.0, 100.0), &mut layout);

    assert_eq!(
        layout
            .get(&tabs_a)
            .expect("tabs_a should receive computed bounds")
            .size
            .width,
        open_gpui::px(200.0)
    );
    assert_eq!(
        layout
            .get(&tabs_b)
            .expect("tabs_b should receive computed bounds")
            .size
            .width,
        open_gpui::px(100.0)
    );
    assert_eq!(
        layout
            .get(&tabs_c)
            .expect("tabs_c should receive computed bounds")
            .size
            .width,
        open_gpui::px(100.0)
    );
}

#[test]
fn layout_roundtrips_roots_splits_and_floatings() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);
    assert!(graph.apply_op(&DockOp::MoveItem {
        source_space: space(),
        item: item("b"),
        target_space: space(),
        target_tabs: root,
        zone: DropZone::Right,
        insert_index: None,
    }));
    assert!(graph.apply_op(&DockOp::FloatItemInWindow {
        source_space: space(),
        item: item("c"),
        target_space: space(),
        bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
    }));

    let layout = graph.export_layout();
    let json = serde_json::to_string(&layout).expect("layout should serialize");
    let layout: DockLayout = serde_json::from_str(&json).expect("layout json should deserialize");
    layout.validate().expect("layout should validate");

    let imported = DockGraph::import_layout(&layout).expect("layout should import");
    assert_eq!(imported.collect_items_in_space(&space()).len(), 3);
    assert_eq!(imported.floating_containers(&space()).len(), 1);
    assert_eq!(
        imported.floating_containers(&space())[0].bounds,
        dock_bounds(10.0, 20.0, 300.0, 200.0)
    );
    imported.assert_canonical_space(&space());
}

#[test]
fn floating_only_space_exports_and_imports_without_root() {
    let mut builder = DockLayoutBuilder::new();
    let floating_tabs = builder.tabs(["floating"], 0);
    builder.add_floating(
        space(),
        floating_tabs,
        dock_bounds(10.0, 20.0, 300.0, 200.0),
    );
    let graph = builder.build();

    assert!(graph.root(&space()).is_none());
    assert_eq!(graph.floating_containers(&space()).len(), 1);

    let layout = graph.export_layout();
    assert_eq!(layout.spaces.len(), 1);
    assert_eq!(layout.spaces[0].id, space());
    assert_eq!(layout.spaces[0].root, None);
    assert_eq!(layout.spaces[0].floatings.len(), 1);
    layout
        .validate()
        .expect("floating-only layout should validate");

    let imported = DockGraph::import_layout(&layout).expect("floating-only layout should import");
    assert!(imported.root(&space()).is_none());
    assert_eq!(imported.floating_containers(&space()).len(), 1);
    assert_eq!(
        imported.collect_items_in_space(&space()),
        vec![item("floating")]
    );
    imported.assert_canonical_space(&space());
}

#[test]
fn layout_validation_rejects_duplicate_ids_cycles_and_bad_active_indexes() {
    let duplicate = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
        }],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("a")],
                active: 0,
            },
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("b")],
                active: 0,
            },
        ],
    );
    assert!(matches!(
        duplicate.validate(),
        Err(DockLayoutValidationError::DuplicateNodeId { id: 1 })
    ));

    let cycle = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
        }],
        vec![DockLayoutNode::Split {
            id: 1,
            axis: SplitAxis::Horizontal,
            children: vec![1],
            fractions: vec![1.0],
        }],
    );
    assert!(matches!(
        cycle.validate(),
        Err(DockLayoutValidationError::CycleDetected { id: 1 })
    ));

    let bad_active = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
        }],
        vec![DockLayoutNode::Tabs {
            id: 1,
            items: vec![item("a")],
            active: 1,
        }],
    );
    assert!(matches!(
        bad_active.validate(),
        Err(DockLayoutValidationError::TabsActiveOutOfBounds { .. })
    ));
}

#[test]
fn layout_validation_rejects_shared_and_unreachable_nodes() {
    let shared_child = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
        }],
        vec![
            DockLayoutNode::Split {
                id: 1,
                axis: SplitAxis::Horizontal,
                children: vec![2, 2],
                fractions: vec![0.5, 0.5],
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec![item("a")],
                active: 0,
            },
        ],
    );
    assert_eq!(
        shared_child.validate(),
        Err(DockLayoutValidationError::DuplicateNodeReference { id: 2 })
    );

    let unreachable = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
        }],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("a")],
                active: 0,
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec![item("unused")],
                active: 0,
            },
        ],
    );
    assert_eq!(
        unreachable.validate(),
        Err(DockLayoutValidationError::UnreachableNodeId { id: 2 })
    );
}

#[test]
fn layout_validation_rejects_duplicate_spaces() {
    let duplicate_spaces = DockLayout::new(
        vec![
            DockLayoutSpace {
                id: space(),
                root: None,
                floatings: Vec::new(),
            },
            DockLayoutSpace {
                id: space(),
                root: None,
                floatings: Vec::new(),
            },
        ],
        Vec::new(),
    );

    assert_eq!(
        duplicate_spaces.validate(),
        Err(DockLayoutValidationError::DuplicateSpaceId { space: space() })
    );
}

#[test]
fn builder_default_editor_layout_sets_root_and_roundtrips() {
    let spec = EditorDockLayoutSpec::new(["hierarchy"], ["scene", "game"], ["inspector"]);
    let graph = DockGraph::default_editor_layout(space(), spec);

    assert!(graph.root(&space()).is_some());
    assert_eq!(graph.collect_items_in_space(&space()).len(), 4);
    graph.assert_canonical_space(&space());

    let layout = graph.export_layout();
    layout.validate().expect("builder layout should validate");
    let imported = DockGraph::import_layout(&layout).expect("builder layout should import");
    assert_eq!(imported.collect_items_in_space(&space()).len(), 4);
}
