use crate::{
    DockFloatingContainer, DockGraph, DockItemId, DockLayoutRect, DockNode, DockSpaceId, SplitAxis,
    dock_bounds,
    dockspace_testkit::{
        DockspaceFloatingSpec, DockspaceScenario, DockspaceScenarioBuildError,
        DockspaceTopologyNode, DockspaceTopologySpace,
        assert_layout_roundtrip_preserves_space_items, assert_root_horizontal_split,
        assert_space_items, assert_unique_items,
        contract::{
            DockspaceCommitObservation, DockspaceDragTerminal, DockspacePrepareDecision,
            DockspaceReceiptExpectation, DockspaceStepExpectation,
        },
        load_builtin_scenario_fixtures,
        ops::{DockspaceScenarioStep, DockspaceScenarioZone, DockspaceSemanticOperation},
    },
    native_scenario_support::{
        NativeDockspaceTarget, NativeDockspaceZone, load_native_dockspace_scenario,
    },
};
use open_gpui::{Bounds, point, px, size};

#[test]
fn dockspace_testkit_builds_complex_tabs_split_float_topology() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == "tabs_split_float_merge")
        .expect("complex scenario should exist");
    let seed = scenario
        .build_graph()
        .expect("complex scenario should build");
    let graph = &seed.graph;

    assert_root_horizontal_split(graph, "main", 2);
    assert_space_items(graph, "main", &["AAA", "BBB", "CCC", "DDD"]);
    assert_eq!(
        graph.floating_containers(&DockSpaceId::from("main")).len(),
        1
    );
    assert_unique_items(graph);
    assert_layout_roundtrip_preserves_space_items(graph);
}

#[test]
fn dockspace_testkit_builds_all_fixture_and_checkpoint_topologies_as_canonical_binary_graphs() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");

    for scenario in scenarios {
        let runtime = scenario
            .build_canonical_runtime()
            .unwrap_or_else(|error| panic!("{} should build canonically: {error}", scenario.id));
        runtime
            .graph()
            .validate()
            .unwrap_or_else(|error| panic!("{} should validate: {error}", scenario.id));
        for space in &scenario.spaces {
            let state = runtime
                .graph()
                .space(&space.id)
                .unwrap_or_else(|| panic!("{} should retain space {}", scenario.id, space.id));
            assert_eq!(
                state.root.is_some(),
                space.root.is_some(),
                "{}",
                scenario.id
            );
            assert_eq!(
                state.floating_roots.len(),
                space.floatings.len(),
                "{}",
                scenario.id
            );
            if let Some(item) = &space.central_item {
                let central_group = runtime
                    .graph()
                    .index()
                    .group_for_item(item)
                    .expect("central item should have a group");
                let central_node = runtime
                    .graph()
                    .index()
                    .node_for_group(central_group)
                    .expect("central group should have a leaf");
                assert_eq!(state.central.node, Some(central_node), "{}", scenario.id);
            }
        }
        for item in scenario.declared_items() {
            assert!(
                runtime.graph().index().group_for_item(&item).is_some(),
                "{} should retain item {item}",
                scenario.id
            );
        }

        for checkpoint in &scenario.checkpoints {
            scenario
                .build_canonical_checkpoint_runtime(checkpoint)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} checkpoint {} should build canonically: {error}",
                        scenario.id, checkpoint.after_step
                    )
                });
        }
    }
}

#[test]
fn canonical_fixture_builder_preserves_nary_split_weights_in_a_binary_tree() {
    let scenario = DockspaceScenario::new("nary-binary").space(
        DockspaceTopologySpace::new("main").root(DockspaceTopologyNode::Split {
            node_key: Some("root".into()),
            axis: SplitAxis::Horizontal,
            children: vec![
                DockspaceTopologyNode::tabs(["A"]),
                DockspaceTopologyNode::tabs(["B"]),
                DockspaceTopologyNode::tabs(["C"]),
            ],
            fractions: vec![0.2, 0.3, 0.5],
        }),
    );
    let runtime = scenario
        .build_canonical_runtime()
        .expect("n-ary fixture should build canonically");
    let root = runtime
        .graph()
        .space(&DockSpaceId::from("main"))
        .and_then(|space| space.root)
        .expect("canonical fixture should retain a root");
    let a = runtime
        .graph()
        .index()
        .node_for_item(&DockItemId::new("A"))
        .unwrap();
    let b = runtime
        .graph()
        .index()
        .node_for_item(&DockItemId::new("B"))
        .unwrap();
    let c = runtime
        .graph()
        .index()
        .node_for_item(&DockItemId::new("C"))
        .unwrap();
    let Some(crate::graph_assembly::CanonicalNode::Branch {
        first,
        second,
        ratio,
        ..
    }) = runtime.graph().node(root)
    else {
        panic!("canonical root should be binary");
    };
    assert_eq!(*first, a);
    assert!((*ratio - 0.2).abs() <= f32::EPSILON);
    assert!(matches!(
        runtime.graph().node(*second),
        Some(crate::graph_assembly::CanonicalNode::Branch {
            first,
            second,
            ratio,
            ..
        }) if *first == b && *second == c && (*ratio - 0.375).abs() <= f32::EPSILON
    ));
}

#[test]
fn dockspace_testkit_rejects_duplicate_panel_labels_before_graph_creation() {
    let scenario = DockspaceScenario::new("duplicate-panels").space(
        DockspaceTopologySpace::new("main").root(DockspaceTopologyNode::split(
            SplitAxis::Horizontal,
            [
                DockspaceTopologyNode::tabs(["AAA"]),
                DockspaceTopologyNode::tabs(["AAA"]),
            ],
        )),
    );

    assert_eq!(
        scenario
            .build_graph()
            .expect_err("duplicate labels should fail"),
        DockspaceScenarioBuildError::DuplicatePanel {
            scenario: "duplicate-panels".into(),
            item: DockItemId::new("AAA"),
        }
    );
}

#[test]
fn dockspace_testkit_rejects_an_unknown_primary_space() {
    let scenario = DockspaceScenario::new("unknown-primary")
        .primary_space("missing")
        .space(DockspaceTopologySpace::new("main").root(DockspaceTopologyNode::tabs(["AAA"])));

    assert_eq!(
        scenario
            .build_graph()
            .expect_err("unknown primary space should fail"),
        DockspaceScenarioBuildError::UnknownPrimarySpace {
            scenario: "unknown-primary".into(),
            space: DockSpaceId::from("missing"),
        }
    );
}

#[test]
fn dockspace_testkit_preserves_empty_central_space_and_floating_roundtrip() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == "empty_central_with_floating")
        .expect("empty central scenario should exist");
    let seed = scenario
        .build_graph()
        .expect("empty central fixture should build");
    let graph = &seed.graph;
    let main = DockSpaceId::from("main");

    assert!(graph.root(&main).is_none());
    assert_eq!(graph.floating_containers(&main).len(), 1);
    assert_eq!(
        graph
            .central_region(&main)
            .expect("empty central metadata should remain present")
            .node,
        None
    );

    let layout = graph.export_layout();
    let imported = DockGraph::import_layout(&layout).expect("layout should import");
    assert!(imported.root(&main).is_none());
    assert_eq!(imported.floating_containers(&main).len(), 1);
    assert_eq!(
        imported.collect_items_in_space(&main),
        vec![DockItemId::new("floating")]
    );
    assert_eq!(
        imported
            .central_region(&main)
            .expect("central metadata should survive import")
            .node,
        None
    );
}

#[test]
fn dockspace_testkit_validates_selected_tabs() {
    let scenario =
        DockspaceScenario::new("missing-selected").space(DockspaceTopologySpace::new("main").root(
            DockspaceTopologyNode::tabs_selected(["AAA", "BBB"], "missing"),
        ));

    assert_eq!(
        scenario
            .build_graph()
            .expect_err("missing selected item should fail"),
        DockspaceScenarioBuildError::MissingSelected {
            scenario: "missing-selected".into(),
            selected: DockItemId::new("missing"),
        }
    );
}

#[test]
fn dockspace_testkit_uses_graph_nodes_without_second_engine_state() {
    let scenario = DockspaceScenario::new("root-only").space(
        DockspaceTopologySpace::new("main")
            .root(DockspaceTopologyNode::tabs(["AAA"]))
            .floating(DockspaceFloatingSpec::new(
                DockspaceTopologyNode::tabs(["BBB"]),
                DockLayoutRect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
            )),
    );
    let seed = scenario.build_graph().expect("scenario should build");
    let root = seed.roots_by_space[&DockSpaceId::from("main")];
    assert!(matches!(seed.graph.node(root), Some(DockNode::Tabs { .. })));
}

#[test]
fn dockspace_testkit_resolves_floating_payload_by_item_identity() {
    let main = DockSpaceId::from("main");
    let mut graph = DockGraph::new();
    let root = graph.insert_node(DockNode::Tabs {
        items: vec![DockItemId::new("target")],
        selected: Some(DockItemId::new("target")),
    });
    graph.set_root(main.clone(), root);
    for (index, item) in ["first", "second"].into_iter().enumerate() {
        let child = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::new(item)],
            selected: Some(DockItemId::new(item)),
        });
        let floating = graph.insert_node(DockNode::Floating { child });
        graph
            .floating_containers_mut(main.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds: dock_bounds(20.0 + index as f32 * 40.0, 30.0, 220.0, 140.0),
            });
    }
    let step = DockspaceScenarioStep {
        id: "move-second".into(),
        operation: DockspaceSemanticOperation::MoveFloatingToItem {
            source_space: main.clone(),
            floating_item: DockItemId::new("second"),
            target_space: main.clone(),
            target_item: DockItemId::new("target"),
            zone: DockspaceScenarioZone::Center,
        },
        required_capabilities: Default::default(),
        interaction: None,
        faults: Vec::new(),
        expected: DockspaceStepExpectation {
            prepare: DockspacePrepareDecision::Prepared,
            commit: Some(DockspaceCommitObservation::Committed),
            terminal: DockspaceDragTerminal::Committed,
            receipt: DockspaceReceiptExpectation::New {
                alias: "move-second".into(),
            },
        },
    };
    let mut runner =
        crate::dockspace_testkit::DockspaceHeadlessRunner::from_graph(main.clone(), graph);

    assert_eq!(
        runner
            .run_step(&step)
            .expect("the matching floating container should move"),
        crate::DockActionOutcome::Changed
    );
    let remaining = runner.graph().floating_containers(&main);
    assert_eq!(remaining.len(), 1);
    assert!(
        runner
            .graph()
            .subtree_contains_item(remaining[0].node, &DockItemId::new("first")),
        "the first floating must remain untouched when the second item is named"
    );
    let DockNode::Tabs { items, .. } = runner
        .graph()
        .node(root)
        .expect("target tabs should remain present")
    else {
        panic!("target root should remain tabs");
    };
    assert_eq!(
        items,
        &vec![DockItemId::new("target"), DockItemId::new("second")]
    );
}

#[test]
fn native_dockspace_projection_exposes_the_full_three_window_workflow() {
    let scenario = load_native_dockspace_scenario("three_viewport_rehome")
        .expect("native scenario projection should load");

    assert_eq!(scenario.primary_space().as_str(), "target");
    assert_eq!(
        scenario
            .spaces()
            .iter()
            .map(DockSpaceId::as_str)
            .collect::<Vec<_>>(),
        ["source", "middle", "target"]
    );
    assert_eq!(
        scenario
            .steps()
            .iter()
            .map(|step| step.id())
            .collect::<Vec<_>>(),
        [
            "cross_viewport_known_target_preview",
            "middle_viewport_known_target_dock",
            "cross_viewport_rehome_to_middle",
            "cross_viewport_return_to_source",
        ]
    );
    assert_eq!(
        scenario
            .items()
            .iter()
            .map(DockItemId::as_str)
            .collect::<Vec<_>>(),
        ["AAA", "EEE", "BBB", "FFF", "CCC", "DDD"]
    );
    DockGraph::import_layout(scenario.layout()).expect("projected native layout should import");
    assert_eq!(scenario.steps().len(), 4);
    for index in 0..scenario.steps().len() {
        DockGraph::import_layout(
            scenario
                .expected_layout_after_step(index)
                .expect("every native step should carry a headless checkpoint"),
        )
        .expect("native checkpoint layout should import");
    }
}

#[test]
fn native_dockspace_projection_supports_empty_space_then_item_redock() {
    let scenario = load_native_dockspace_scenario("empty_space_split_collapse_roundtrip")
        .expect("native empty-space scenario projection should load");

    assert_eq!(scenario.primary_space().as_str(), "source");
    assert_eq!(scenario.steps().len(), 2);
    assert!(matches!(
        scenario.steps()[0].target(),
        NativeDockspaceTarget::EmptySpace { space } if space.as_str() == "target"
    ));
    assert!(matches!(
        scenario.steps()[1].target(),
        NativeDockspaceTarget::Item {
            space,
            item,
            zone: NativeDockspaceZone::Left,
        } if space.as_str() == "source" && item.as_str() == "B"
    ));
    for index in 0..scenario.steps().len() {
        DockGraph::import_layout(
            scenario
                .expected_layout_after_step(index)
                .expect("every empty-space step should carry a headless checkpoint"),
        )
        .expect("empty-space checkpoint layout should import");
    }
}

#[test]
fn native_dockspace_projection_uses_production_drop_guide_geometry() {
    let bounds = Bounds::new(point(px(20.0), px(30.0)), size(px(520.0), px(360.0)));
    for (zone, expected) in [
        (NativeDockspaceZone::Center, crate::DropZone::Center),
        (NativeDockspaceZone::Right, crate::DropZone::Right),
    ] {
        let geometry = zone
            .target_geometry(bounds)
            .expect("drop guide should fit in the target bounds");
        let resolved = crate::geometry::resolve_inner_drop_geometry_with_style(
            bounds,
            geometry.target_point(),
            crate::DockDropGuideMetrics::default(),
        )
        .expect("the projected point should resolve through production geometry");

        assert_eq!(resolved.zone(), expected);
        assert_eq!(resolved.drop_box.draw_bounds, geometry.guide_bounds());
        assert_eq!(resolved.preview_bounds(), geometry.preview_bounds());
    }
}
