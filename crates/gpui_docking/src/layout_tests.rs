use crate::graph_test_support::{edge_target, item, main_space as space, root_tabs_graph};
use crate::*;

#[test]
fn compute_layout_repairs_mismatched_fraction_lengths_without_truncating_children() {
    let mut graph = DockGraph::new();
    let tabs_a = graph.insert_node(DockNode::Tabs {
        items: vec![item("a")],
        selected: Some(item("a")),
    });
    let tabs_b = graph.insert_node(DockNode::Tabs {
        items: vec![item("b")],
        selected: Some(item("b")),
    });
    let tabs_c = graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
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
fn compute_layout_gives_central_child_remaining_split_space() {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let main = graph.insert_node(DockNode::Tabs {
        items: vec![item("main")],
        selected: Some(item("main")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, main, right],
        fractions: vec![0.2, 0.0, 0.3],
    });
    graph.set_root(space(), split);
    graph.set_central_region(space(), DockCentralRegion::with_node(main));

    let mut layout = std::collections::HashMap::new();
    graph.compute_layout(split, dock_bounds(0.0, 0.0, 1000.0, 100.0), &mut layout);

    assert_eq!(layout[&left].size.width, open_gpui::px(200.0));
    assert_eq!(layout[&main].size.width, open_gpui::px(500.0));
    assert_eq!(layout[&right].size.width, open_gpui::px(300.0));
}

#[test]
fn compute_layout_uses_shared_split_layout_when_neighbors_over_allocate() {
    let mut graph = DockGraph::new();
    let left = graph.insert_node(DockNode::Tabs {
        items: vec![item("left")],
        selected: Some(item("left")),
    });
    let main = graph.insert_node(DockNode::Tabs {
        items: vec![item("main")],
        selected: Some(item("main")),
    });
    let right = graph.insert_node(DockNode::Tabs {
        items: vec![item("right")],
        selected: Some(item("right")),
    });
    let split = graph.insert_node(DockNode::Split {
        axis: SplitAxis::Horizontal,
        children: vec![left, main, right],
        fractions: vec![0.8, 0.0, 0.7],
    });
    graph.set_root(space(), split);
    graph.set_central_region(space(), DockCentralRegion::with_node(main));

    let mut layout = std::collections::HashMap::new();
    graph.compute_layout(split, dock_bounds(0.0, 0.0, 1000.0, 100.0), &mut layout);

    assert_px_close(layout[&left].size.width, 533.3334);
    assert_eq!(layout[&main].size.width, open_gpui::px(0.0));
    assert_px_close(layout[&right].size.width, 466.6667);
}

#[test]
fn layout_roundtrips_roots_splits_and_floatings() {
    let (mut graph, root) = root_tabs_graph(&["a", "b", "c"]);
    assert!(
        graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: space(),
                item: item("b"),
                target_space: space(),
                target: edge_target(&graph, &space(), root, DropZone::Right),
            })
            .expect("root-edge move should commit")
    );
    assert!(
        graph
            .apply_op_checked(&DockOp::FloatItemInWindow {
                source_space: space(),
                item: item("c"),
                target_space: space(),
                bounds: dock_bounds(10.0, 20.0, 300.0, 200.0),
            })
            .expect("float item should commit")
    );

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

fn assert_px_close(actual: open_gpui::Pixels, expected: f32) {
    assert!(
        (f32::from(actual) - expected).abs() <= 0.001,
        "expected {actual:?} to be close to {expected}"
    );
}

#[test]
fn empty_central_region_roundtrips_without_root_or_items() {
    let mut graph = DockGraph::new();
    graph.set_central_region(
        space(),
        DockCentralRegion::empty().with_passthrough_when_empty(true),
    );

    let layout = graph.export_layout();
    assert_eq!(layout.spaces.len(), 1);
    assert_eq!(layout.spaces[0].root, None);
    assert!(layout.spaces[0].floatings.is_empty());
    assert_eq!(
        layout.spaces[0].central,
        Some(DockLayoutCentralRegion {
            node: None,
            keep_alive_when_empty: true,
            passthrough_when_empty: true,
        })
    );
    layout
        .validate()
        .expect("empty central layout should validate");

    let imported = DockGraph::import_layout(&layout).expect("empty central layout should import");
    let central = imported
        .central_region(&space())
        .expect("central metadata should roundtrip");
    assert_eq!(central.node, None);
    assert!(central.keep_alive_when_empty);
    assert!(central.passthrough_when_empty);
    assert!(imported.root(&space()).is_none());
    assert!(imported.collect_items_in_space(&space()).is_empty());
}

#[test]
fn central_node_roundtrips_for_default_editor_layout() {
    let graph = DockGraph::default_editor_layout(
        space(),
        EditorDockLayoutSpec::new(["hierarchy"], ["scene", "game"], ["inspector"]),
    );
    let central = graph
        .central_region(&space())
        .and_then(|central| central.node)
        .expect("default editor layout should mark main tabs as central");
    assert_eq!(
        graph.collect_items_in_subtree(central),
        vec![item("scene"), item("game")]
    );

    let layout = graph.export_layout();
    assert!(
        layout.spaces[0]
            .central
            .as_ref()
            .and_then(|central| central.node)
            .is_some(),
        "central node id should be serialized"
    );
    let imported = DockGraph::import_layout(&layout).expect("central layout should import");
    let imported_central = imported
        .central_region(&space())
        .and_then(|central| central.node)
        .expect("central node should import");
    assert_eq!(
        imported.collect_items_in_subtree(imported_central),
        vec![item("scene"), item("game")]
    );
}

#[test]
fn floating_only_space_exports_and_imports_without_root() {
    let mut builder = DockLayoutBuilder::new();
    let floating_tabs = builder.tabs(["floating"]);
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
fn layout_validation_rejects_duplicate_ids_cycles_and_bad_tab_selection() {
    let duplicate = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
        }],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("a")],
                selected: Some(item("a")),
            },
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("b")],
                selected: Some(item("b")),
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
            central: None,
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

    let missing_selection = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
        }],
        vec![DockLayoutNode::Tabs {
            id: 1,
            items: vec![item("a")],
            selected: None,
        }],
    );
    assert!(matches!(
        missing_selection.validate(),
        Err(DockLayoutValidationError::TabsSelectionMissing { id: 1 })
    ));
}

#[test]
fn layout_validation_rejects_ordinary_empty_tabs() {
    let empty_tabs = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
        }],
        vec![DockLayoutNode::Tabs {
            id: 1,
            items: Vec::new(),
            selected: None,
        }],
    );

    assert_eq!(
        empty_tabs.validate(),
        Err(DockLayoutValidationError::EmptyTabs { id: 1 })
    );
}

#[test]
fn layout_validation_rejects_central_node_outside_root_subtree() {
    let layout = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: Some(DockLayoutCentralRegion {
                node: Some(2),
                keep_alive_when_empty: true,
                passthrough_when_empty: false,
            }),
        }],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("root")],
                selected: Some(item("root")),
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec![item("central")],
                selected: Some(item("central")),
            },
        ],
    );

    assert_eq!(
        layout.validate(),
        Err(DockLayoutValidationError::CentralNodeNotInRoot {
            space: space(),
            node: 2,
        })
    );
}

#[test]
fn layout_validation_rejects_shared_and_unreachable_nodes() {
    let shared_child = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
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
                selected: Some(item("a")),
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
            central: None,
        }],
        vec![
            DockLayoutNode::Tabs {
                id: 1,
                items: vec![item("a")],
                selected: Some(item("a")),
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec![item("unused")],
                selected: Some(item("unused")),
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
                central: None,
            },
            DockLayoutSpace {
                id: space(),
                root: None,
                floatings: Vec::new(),
                central: None,
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
fn layout_validation_rejects_duplicate_items() {
    let duplicate_items = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
        }],
        vec![
            DockLayoutNode::Split {
                id: 1,
                axis: SplitAxis::Horizontal,
                children: vec![2, 3],
                fractions: vec![0.5, 0.5],
            },
            DockLayoutNode::Tabs {
                id: 2,
                items: vec![item("a")],
                selected: Some(item("a")),
            },
            DockLayoutNode::Tabs {
                id: 3,
                items: vec![item("a")],
                selected: Some(item("a")),
            },
        ],
    );

    assert_eq!(
        duplicate_items.validate(),
        Err(DockLayoutValidationError::DuplicateItemId {
            item: item("a"),
            first_node: 2,
            duplicate_node: 3
        })
    );
}

#[test]
fn layout_validation_rejects_invalid_floating_bounds() {
    let invalid_bounds = DockLayout::new(
        vec![DockLayoutSpace {
            id: space(),
            root: None,
            floatings: vec![DockLayoutFloatingContainer {
                root: 1,
                bounds: DockLayoutRect {
                    x: 10.0,
                    y: f32::NAN,
                    width: 300.0,
                    height: 200.0,
                },
            }],
            central: None,
        }],
        vec![DockLayoutNode::Tabs {
            id: 1,
            items: vec![item("a")],
            selected: Some(item("a")),
        }],
    );

    assert_eq!(
        invalid_bounds.validate(),
        Err(DockLayoutValidationError::InvalidFloatingBounds {
            space: space(),
            root: 1
        })
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
