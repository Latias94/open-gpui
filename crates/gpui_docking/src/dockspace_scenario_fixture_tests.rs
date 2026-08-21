#[cfg(feature = "test-support")]
use crate::benchmark_support::DockspaceScenarioSequenceBenchmark;
use crate::{
    DockActionOutcome, DockGraph, DockItemId, DockNode, DockSpaceId,
    dockspace_testkit::{
        DockspaceAdapterTag, DockspaceHeadlessRunner, DockspaceOracleKind, DockspaceScenario,
        assert_layout_roundtrip_preserves_space_items, assert_unique_items,
        benchmarks::DockspaceBenchmarkCase, load_builtin_scenario_fixtures,
    },
};
use std::collections::BTreeSet;

fn scenario_by_id<'a>(scenarios: &'a [DockspaceScenario], id: &str) -> &'a DockspaceScenario {
    scenarios
        .iter()
        .find(|scenario| scenario.id == id)
        .unwrap_or_else(|| panic!("fixture corpus should include scenario {id}"))
}

fn tabs_items_for(graph: &DockGraph, space: &str, item: &str) -> Vec<DockItemId> {
    let space = DockSpaceId::from(space);
    let item = DockItemId::new(item);
    let (tabs, _) = graph
        .find_item_in_space(&space, &item)
        .unwrap_or_else(|| panic!("item {item} should exist in space {space}"));
    let DockNode::Tabs { items, .. } = graph
        .node(tabs)
        .unwrap_or_else(|| panic!("tabs node for {item} should exist"))
    else {
        panic!("item {item} should resolve to a tabs node");
    };
    items.clone()
}

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
    for id in [
        "empty_space_split_collapse_roundtrip",
        "tab_reorder_roundtrip",
        "floating_redock_by_item",
        "three_dockspace_rehome_cycle",
    ] {
        scenario_by_id(&scenarios, id);
    }

    for scenario in &scenarios {
        let seed = scenario
            .build_graph()
            .unwrap_or_else(|error| panic!("scenario {} should build: {error}", scenario.id));
        assert_unique_items(&seed.graph);
        assert_layout_roundtrip_preserves_space_items(&seed.graph);
        assert!(
            scenario.supports_adapter(DockspaceAdapterTag::Headless),
            "scenario {} should be headless-verifiable first",
            scenario.id
        );
        let oracles = scenario.oracles();
        assert!(
            oracles.contains(&DockspaceOracleKind::Graph),
            "scenario {} should carry a graph oracle",
            scenario.id
        );
        if oracles.contains(&DockspaceOracleKind::NativeLifetime) {
            assert!(
                scenario.supports_adapter(DockspaceAdapterTag::Native),
                "scenario {} should only request native oracles with native adapter support",
                scenario.id
            );
        }
        if oracles.contains(&DockspaceOracleKind::Performance) {
            assert!(
                scenario.supports_adapter(DockspaceAdapterTag::Benchmark),
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
fn scenario_backed_benchmarks_reference_declared_fixture_steps() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");

    let registered_benchmarks = [
        DockspaceBenchmarkCase::complex_split_float(),
        DockspaceBenchmarkCase::three_dockspace_rehome_cycle(),
    ];
    for benchmark in registered_benchmarks {
        let scenario = scenario_by_id(&scenarios, benchmark.scenario_id());
        assert!(
            scenario.supports_adapter(DockspaceAdapterTag::Benchmark),
            "scenario {} should opt into the benchmark adapter",
            scenario.id
        );
        assert!(
            scenario
                .oracles()
                .contains(&DockspaceOracleKind::Performance),
            "scenario {} should request the performance oracle",
            scenario.id
        );
        if let Some(step_id) = benchmark.step_id() {
            assert!(
                scenario.steps.iter().any(|step| step.id() == Some(step_id)),
                "scenario {} should declare benchmark step {step_id}",
                scenario.id
            );
        }
    }
    let registered_scenarios = registered_benchmarks
        .iter()
        .map(|benchmark| benchmark.scenario_id())
        .collect::<BTreeSet<_>>();
    let tagged_scenarios = scenarios
        .iter()
        .filter(|scenario| scenario.supports_adapter(DockspaceAdapterTag::Benchmark))
        .map(|scenario| scenario.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        tagged_scenarios, registered_scenarios,
        "benchmark adapter tags should match the scenario-backed benchmark registry"
    );

    #[cfg(feature = "test-support")]
    {
        let cycle = DockspaceScenarioSequenceBenchmark::three_dockspace_rehome_cycle();
        assert_eq!(cycle.step_count(), 8);
        assert!(cycle.validate_cycle_state());
        assert_eq!(
            cycle.workload_contract(),
            "dockspace-sequence-v2|spaces=source=T[selected=AAA;items=EEE,AAA];middle=T[selected=BBB;items=FFF,BBB];target=Svertical[fractions=0.5,0.5](T[selected=CCC;items=CCC],T[selected=DDD;items=DDD])|steps=AAA@source>CCC@target:right;BBB@middle>DDD@target:center;AAA@target>FFF@middle:center;BBB@target>EEE@source:center;BBB@source>DDD@target:center;AAA@middle>CCC@target:right;BBB@target>FFF@middle:center;AAA@target>EEE@source:center|final=initial"
        );
    }
}

#[test]
fn dockspace_scenario_fixtures_cover_empty_tab_floating_and_multiview_roundtrips() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");

    let empty = scenario_by_id(&scenarios, "empty_space_split_collapse_roundtrip");
    let initial_empty_layout = empty
        .build_graph()
        .expect("empty-space scenario should build")
        .graph
        .export_layout();
    let mut empty_runner = DockspaceHeadlessRunner::from_scenario(empty, "source")
        .expect("empty-space scenario should build headlessly");
    assert_eq!(
        empty_runner
            .run_step(&empty.steps[0])
            .expect("empty-space move should commit"),
        DockActionOutcome::Changed
    );
    assert_eq!(
        empty_runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("source")),
        vec![DockItemId::new("B"), DockItemId::new("C")],
        "moving the singleton split child should promote the surviving tabs node"
    );
    assert_eq!(
        empty_runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("target")),
        vec![DockItemId::new("A")]
    );
    assert_eq!(
        empty_runner
            .run_step(&empty.steps[1])
            .expect("edge redock should recreate the split"),
        DockActionOutcome::Changed
    );
    assert_eq!(empty_runner.graph().export_layout(), initial_empty_layout);

    let reorder = scenario_by_id(&scenarios, "tab_reorder_roundtrip");
    let mut reorder_runner = DockspaceHeadlessRunner::from_scenario(reorder, "main")
        .expect("tab-reorder scenario should build headlessly");
    assert_eq!(
        reorder_runner
            .run_step(&reorder.steps[0])
            .expect("already-adjacent tab placement should resolve"),
        DockActionOutcome::Unchanged
    );
    assert_eq!(
        reorder_runner
            .run_step(&reorder.steps[1])
            .expect("tab should move after the final tab"),
        DockActionOutcome::Changed
    );
    assert_eq!(
        tabs_items_for(reorder_runner.graph(), "main", "A"),
        vec![
            DockItemId::new("B"),
            DockItemId::new("C"),
            DockItemId::new("A"),
        ]
    );
    assert_eq!(
        reorder_runner
            .run_step(&reorder.steps[2])
            .expect("tab should move back before B"),
        DockActionOutcome::Changed
    );
    assert_eq!(
        tabs_items_for(reorder_runner.graph(), "main", "A"),
        vec![
            DockItemId::new("A"),
            DockItemId::new("B"),
            DockItemId::new("C"),
        ]
    );

    let floating = scenario_by_id(&scenarios, "floating_redock_by_item");
    let initial_floating_layout = floating
        .build_graph()
        .expect("floating scenario should build")
        .graph
        .export_layout();
    let mut floating_runner = DockspaceHeadlessRunner::from_scenario(floating, "main")
        .expect("floating scenario should build headlessly");
    assert_eq!(
        floating_runner
            .run_all(&floating.steps)
            .expect("float and redock should commit"),
        vec![DockActionOutcome::Changed, DockActionOutcome::Changed]
    );
    assert_eq!(
        floating_runner.graph().export_layout(),
        initial_floating_layout
    );
}

#[test]
fn dockspace_cycle_remains_canonical_across_long_replay() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenario_by_id(&scenarios, "three_dockspace_rehome_cycle");
    let initial_layout = scenario
        .build_graph()
        .expect("multiview cycle should build")
        .graph
        .export_layout();
    let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, "target")
        .expect("multiview cycle should build headlessly");

    for cycle in 0..32 {
        let outcomes = runner
            .run_all(&scenario.steps)
            .unwrap_or_else(|error| panic!("multiview cycle {cycle} should commit: {error}"));
        assert!(
            outcomes
                .iter()
                .all(|outcome| *outcome == DockActionOutcome::Changed),
            "every multiview cycle operation should mutate the graph"
        );
        assert_unique_items(runner.graph());
        assert_layout_roundtrip_preserves_space_items(runner.graph());
        assert_eq!(
            runner.graph().export_layout(),
            initial_layout,
            "cycle {cycle} should restore the exact serialized topology"
        );
    }
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
