use crate::{
    DockGraph, DockItemId, DockLayoutRect, DockNode, DockSpaceId, SplitAxis,
    dockspace_testkit::{
        DockspaceFloatingSpec, DockspaceScenario, DockspaceScenarioBuildError,
        DockspaceTopologyNode, DockspaceTopologySpace,
        assert_layout_roundtrip_preserves_space_items, assert_root_horizontal_split,
        assert_space_items, assert_unique_items, complex_tabs_split_float_scenario,
        empty_central_with_floating_scenario, three_viewport_rehome_scenario,
    },
};

#[test]
fn dockspace_testkit_builds_complex_tabs_split_float_topology() {
    let scenario = complex_tabs_split_float_scenario();
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
fn dockspace_testkit_preserves_empty_central_space_and_floating_roundtrip() {
    let scenario = empty_central_with_floating_scenario();
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
fn dockspace_testkit_typed_builder_drives_three_viewport_rehome_headlessly() {
    let scenario = three_viewport_rehome_scenario();
    let mut runner =
        crate::dockspace_testkit::DockspaceHeadlessRunner::from_scenario(&scenario, "source")
            .expect("typed multiview scenario should build");

    let outcomes = runner
        .run_all(&scenario.steps)
        .expect("typed multiview scenario should commit headlessly");
    assert_eq!(
        outcomes,
        vec![
            crate::DockActionOutcome::Changed,
            crate::DockActionOutcome::Changed
        ]
    );
    assert_unique_items(runner.graph());
    assert_eq!(
        runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("source")),
        vec![DockItemId::new("AAA")]
    );
}
