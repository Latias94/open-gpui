use crate::{
    DockActionOutcome, DockSpaceId,
    dockspace_testkit::{
        DockspaceAdapterTag, DockspaceHeadlessRunner, DockspaceOracleKind,
        assert_layout_roundtrip_preserves_space_items, assert_unique_items,
        load_builtin_scenario_fixtures,
    },
};

#[test]
fn dockspace_scenario_fixtures_build_and_validate_adapter_oracles() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    assert!(
        scenarios
            .iter()
            .any(|scenario| scenario.id == "tabs_split_float_merge"),
        "fixture corpus should include the complex tabs/split/floating topology"
    );
    assert!(
        scenarios
            .iter()
            .any(|scenario| scenario.id == "three_viewport_rehome"),
        "fixture corpus should include the multiview rehome topology"
    );

    for scenario in &scenarios {
        let seed = scenario
            .build_graph()
            .unwrap_or_else(|error| panic!("scenario {} should build: {error}", scenario.id));
        assert_unique_items(&seed.graph);
        assert_layout_roundtrip_preserves_space_items(&seed.graph);
        assert!(
            scenario.adapters.contains(&DockspaceAdapterTag::Headless),
            "scenario {} should be headless-verifiable first",
            scenario.id
        );
        assert!(
            scenario.oracles.contains(&DockspaceOracleKind::Graph),
            "scenario {} should carry a graph oracle",
            scenario.id
        );
        if scenario
            .oracles
            .contains(&DockspaceOracleKind::NativeLifetime)
        {
            assert!(
                scenario.adapters.contains(&DockspaceAdapterTag::Native),
                "scenario {} should only request native oracles with native adapter support",
                scenario.id
            );
        }
        if scenario.oracles.contains(&DockspaceOracleKind::Performance) {
            assert!(
                scenario.adapters.contains(&DockspaceAdapterTag::Benchmark),
                "scenario {} should only request perf oracles with benchmark adapter support",
                scenario.id
            );
        }
    }

    let rehome = scenarios
        .iter()
        .find(|scenario| scenario.id == "three_viewport_rehome")
        .expect("three viewport fixture should exist");
    assert!(
        rehome
            .steps
            .iter()
            .any(|step| step.id() == Some("cross_viewport_known_target_preview")),
        "native and benchmark adapters need a stable multiview step id"
    );
}

#[test]
fn dockspace_scenario_fixtures_drive_headless_workspace_transactions() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == "tabs_split_float_merge")
        .expect("complex topology fixture should exist");
    let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, "main")
        .expect("headless runner should build");

    let outcomes = runner
        .run_all(&scenario.steps)
        .expect("scenario steps should commit through workspace transactions");
    assert_eq!(
        outcomes,
        vec![DockActionOutcome::Changed, DockActionOutcome::Changed]
    );
    assert_unique_items(runner.graph());
    assert_eq!(
        runner
            .graph()
            .floating_containers(&DockSpaceId::from("main"))
            .len(),
        0
    );
}

#[test]
fn dockspace_scenario_fixtures_drive_cross_space_rehome_without_native_windows() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == "three_viewport_rehome")
        .expect("three viewport fixture should exist");
    let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, "source")
        .expect("headless runner should build");

    let outcomes = runner
        .run_all(&scenario.steps)
        .expect("cross-space scenario should commit headlessly");
    assert_eq!(
        outcomes,
        vec![
            DockActionOutcome::Changed,
            DockActionOutcome::Changed,
            DockActionOutcome::Changed,
            DockActionOutcome::Changed,
        ]
    );
    assert_unique_items(runner.graph());
    assert_eq!(
        runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("source")),
        vec![crate::DockItemId::new("EEE"), crate::DockItemId::new("BBB")]
    );
    assert_eq!(
        runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("middle")),
        vec![crate::DockItemId::new("FFF"), crate::DockItemId::new("AAA")]
    );
}
