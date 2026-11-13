#[cfg(feature = "test-support")]
use crate::legacy::benchmark_support::DockspaceScenarioSequenceBenchmark;
use crate::legacy::dockspace_testkit::contract::{
    DockspaceAdapterCapability, DockspaceDragTerminal, DockspaceFaultInjection, DockspaceFaultKind,
    DockspaceFaultPoint, DockspaceProgressState,
};
use crate::legacy::dockspace_testkit::ops::{DockspaceScenarioZone, DockspaceSemanticOperation};
use crate::legacy::dockspace_testkit::topology::DockspaceCheckpointPhase;
use crate::legacy::{
    DockActionOutcome, DockGraph, DockItemId, DockNode, DockSpaceId,
    dockspace_testkit::{
        CanonicalDockspaceHeadlessRunner, CanonicalDockspaceScenarioRunError,
        DockspaceAdapterStepSupport, DockspaceAdapterTag, DockspaceCheckpointAssertionError,
        DockspaceHeadlessRunner, DockspaceOracleKind, DockspaceScenario, DockspaceTopologyNode,
        assert_canonical_checkpoint, assert_canonical_edge_locality, assert_headless_checkpoint,
        assert_layout_roundtrip_preserves_space_items, assert_unique_items,
        benchmarks::DockspaceBenchmarkCase, capture_canonical_edge_target,
        load_builtin_scenario_fixtures,
    },
};
use std::collections::{BTreeMap, BTreeSet};

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
fn dockspace_scenario_checkpoints_are_independent_headless_observations() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");

    for scenario in scenarios
        .iter()
        .filter(|scenario| scenario.supports_adapter(DockspaceAdapterTag::Headless))
    {
        let primary_space = scenario
            .primary_space
            .clone()
            .or_else(|| scenario.spaces.first().map(|space| space.id.clone()))
            .unwrap_or_else(|| panic!("scenario {} should declare a dock space", scenario.id));
        let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, primary_space)
            .unwrap_or_else(|error| panic!("scenario {} should build: {error}", scenario.id));

        let terminal_checkpoints = scenario.terminal_checkpoints().unwrap_or_else(|error| {
            panic!(
                "scenario {} should have terminal checkpoints: {error}",
                scenario.id
            )
        });
        for (step, checkpoint) in scenario.steps.iter().zip(terminal_checkpoints) {
            let support = scenario
                .adapter_step_support(DockspaceAdapterTag::Headless, step)
                .expect("headless adapter contract should be declared");
            if let DockspaceAdapterStepSupport::OptionalUnsupported { capabilities } = support {
                assert!(
                    !capabilities.is_empty(),
                    "optional unsupported result must name its capabilities"
                );
                continue;
            }
            assert_eq!(
                support,
                DockspaceAdapterStepSupport::Supported,
                "headless fixture step must be supported or explicitly optional"
            );
            let pre_state = scenario
                .observe_graph_spaces(runner.graph())
                .unwrap_or_else(|error| panic!("scenario {} should observe: {error}", scenario.id));
            let outcome = runner.run_step(step).unwrap_or_else(|error| {
                panic!(
                    "scenario {} step {} should execute: {error}",
                    scenario.id,
                    step.id().unwrap_or("<missing>")
                )
            });
            assert_headless_checkpoint(scenario, checkpoint, runner.graph(), outcome, &pre_state)
                .unwrap_or_else(|error| panic!("{error}"));
        }
    }
}

#[test]
fn canonical_headless_runner_enforces_full_fixture_evidence_contract() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");

    for scenario in &scenarios {
        let scenario_id = &scenario.id;
        let mut runner =
            CanonicalDockspaceHeadlessRunner::from_scenario(scenario).unwrap_or_else(|error| {
                panic!("canonical scenario {scenario_id} should build: {error}")
            });
        runner
            .graph()
            .validate()
            .unwrap_or_else(|error| panic!("canonical scenario {scenario_id} seed: {error}"));
        let terminal_checkpoints = scenario.terminal_checkpoints().unwrap_or_else(|error| {
            panic!("scenario {scenario_id} should have terminal checkpoints: {error}")
        });
        let mut receipt_aliases = BTreeMap::new();
        let mut unsupported_step = None;
        for (step, checkpoint) in scenario.steps.iter().zip(terminal_checkpoints) {
            let pre_state = scenario
                .observe_canonical_spaces(runner.graph())
                .unwrap_or_else(|error| {
                    panic!("canonical scenario {scenario_id} pre-state should observe: {error}")
                });
            let edge_probe = capture_canonical_edge_target(runner.graph(), step);
            if is_edge_target_operation(&step.operation) {
                assert!(
                    edge_probe.is_some(),
                    "canonical scenario {scenario_id}/{} must resolve its edge target before prepare",
                    step.id
                );
            }
            let evidence = match runner.run_step(step) {
                Ok(evidence) => evidence,
                Err(error @ CanonicalDockspaceScenarioRunError::UnsupportedStepContract { .. }) => {
                    let post_state = scenario
                        .observe_canonical_spaces(runner.graph())
                        .expect("unsupported canonical step must remain observable");
                    assert_eq!(
                        post_state, pre_state,
                        "unsupported canonical scenario {scenario_id}/{} mutated graph authority",
                        step.id
                    );
                    assert_expected_canonical_unsupported(scenario_id, &step.id, &error);
                    unsupported_step = Some(step.id.as_str());
                    break;
                }
                Err(error) => panic!(
                    "canonical scenario {scenario_id} step {} should execute: {error}",
                    step.id
                ),
            };
            assert_canonical_checkpoint(
                scenario,
                step,
                checkpoint,
                runner.graph(),
                &evidence,
                &mut receipt_aliases,
                &pre_state,
            )
            .unwrap_or_else(|error| panic!("{error}"));
            assert_canonical_edge_locality(
                scenario,
                checkpoint,
                runner.graph(),
                edge_probe,
                &pre_state,
            )
            .unwrap_or_else(|error| panic!("{error}"));
        }

        let expected_unsupported_step = match scenario_id.as_str() {
            "provider_first_destination_loss_contract" => Some("provider_destination_loss"),
            _ => None,
        };
        assert_eq!(
            unsupported_step, expected_unsupported_step,
            "canonical fixture support classification drifted for {scenario_id}"
        );
    }
}

#[test]
fn canonical_float_redock_can_repeat_without_synthetic_identity_collision() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenario_by_id(&scenarios, "floating_redock_by_item");
    let mut runner = CanonicalDockspaceHeadlessRunner::from_scenario(scenario)
        .expect("canonical floating scenario should build");

    for cycle in 0..2 {
        for step in &scenario.steps {
            runner.run_step(step).unwrap_or_else(|error| {
                panic!(
                    "floating cycle {cycle} step {} should execute: {error}",
                    step.id
                )
            });
        }
        runner.graph().validate().unwrap_or_else(|error| {
            panic!("floating cycle {cycle} must remain canonical: {error}")
        });
    }

    let item_a = DockItemId::from("A");
    let item_b = DockItemId::from("B");
    let group_a = runner.graph().index().group_for_item(&item_a).unwrap();
    let group_b = runner.graph().index().group_for_item(&item_b).unwrap();
    assert_eq!(group_a, group_b);
    let group = runner.graph().group(group_a).unwrap();
    assert_eq!(group.items, vec![item_a, item_b.clone()]);
    assert_eq!(group.selected, item_b);
    assert!(runner.graph().floating_roots().is_empty());
}

#[test]
fn float_item_capabilities_distinguish_same_space_and_cross_viewport_moves() {
    let same_space = DockspaceSemanticOperation::FloatItemInWindow {
        source_space: DockSpaceId::from("main"),
        item: DockItemId::from("A"),
        target_space: DockSpaceId::from("main"),
        bounds: [0.0, 0.0, 320.0, 240.0],
    };
    let cross_viewport = DockspaceSemanticOperation::FloatItemInWindow {
        source_space: DockSpaceId::from("main"),
        item: DockItemId::from("A"),
        target_space: DockSpaceId::from("secondary"),
        bounds: [0.0, 0.0, 320.0, 240.0],
    };

    assert!(
        !same_space
            .required_capabilities()
            .contains(&DockspaceAdapterCapability::CrossViewport)
    );
    assert!(
        cross_viewport
            .required_capabilities()
            .contains(&DockspaceAdapterCapability::CrossViewport)
    );
}

fn is_edge_target_operation(operation: &DockspaceSemanticOperation) -> bool {
    matches!(
        operation,
        DockspaceSemanticOperation::MoveItemToItem { zone, .. }
            | DockspaceSemanticOperation::MoveGroupToItem { zone, .. }
            | DockspaceSemanticOperation::MoveFloatingToItem { zone, .. }
            if *zone != DockspaceScenarioZone::Center
    )
}

fn assert_expected_canonical_unsupported(
    scenario: &str,
    step: &str,
    error: &CanonicalDockspaceScenarioRunError,
) {
    let CanonicalDockspaceScenarioRunError::UnsupportedStepContract {
        scenario: actual_scenario,
        step: actual_step,
        capabilities,
        faults,
    } = error
    else {
        panic!("expected typed unsupported contract, got {error}");
    };
    assert_eq!(actual_scenario, scenario);
    assert_eq!(actual_step, step);

    match scenario {
        "provider_first_destination_loss_contract" => {
            assert_eq!(
                capabilities,
                &BTreeSet::from([
                    DockspaceAdapterCapability::ProviderFirstReservation,
                    DockspaceAdapterCapability::Recovery,
                    DockspaceAdapterCapability::ShutdownDrain,
                ])
            );
            assert_eq!(
                faults,
                &[DockspaceFaultInjection {
                    kind: DockspaceFaultKind::DestinationLoss,
                    point: DockspaceFaultPoint::AfterGraphCommitBeforeSubmittedSemantics,
                }]
            );
        }
        _ => panic!("canonical scenario {scenario}/{step} was silently excluded"),
    }
}

#[test]
fn checkpoint_mismatch_reports_action_field_values_and_pre_state() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenario_by_id(&scenarios, "tab_reorder_roundtrip");
    let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, "main")
        .expect("tab reorder scenario should build");
    let pre_state = scenario
        .observe_graph_spaces(runner.graph())
        .expect("pre-state should be observable");
    let outcome = runner
        .run_step(&scenario.steps[0])
        .expect("no-op step should execute");
    let mut checkpoint = scenario.checkpoints[0].clone();
    let Some(DockspaceTopologyNode::Tabs { selected, .. }) = checkpoint.spaces[0].root.as_mut()
    else {
        panic!("tab reorder checkpoint should contain root tabs");
    };
    *selected = Some(DockItemId::new("B"));

    let error =
        assert_headless_checkpoint(scenario, &checkpoint, runner.graph(), outcome, &pre_state)
            .expect_err("the altered checkpoint should mismatch");
    let DockspaceCheckpointAssertionError::Mismatch(mismatch) = error else {
        panic!("expected a structured checkpoint mismatch, got {error:?}");
    };
    assert_eq!(mismatch.action, "adjacent_before_noop");
    assert_eq!(mismatch.field, "spaces[0].root.selected");
    assert_eq!(mismatch.expected, serde_json::json!("B"));
    assert_eq!(mismatch.actual, serde_json::json!("A"));
    assert_eq!(mismatch.pre_state, serde_json::to_value(pre_state).unwrap());
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
        vec![
            crate::legacy::DockItemId::new("EEE"),
            crate::legacy::DockItemId::new("BBB")
        ]
    );
    assert_eq!(
        runner
            .graph()
            .collect_items_in_space(&DockSpaceId::from("middle")),
        vec![
            crate::legacy::DockItemId::new("FFF"),
            crate::legacy::DockItemId::new("AAA")
        ]
    );
}

#[test]
fn whole_group_fixture_preserves_order_and_selected_tab() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenario_by_id(&scenarios, "whole_group_cross_space_rehome");
    let step = &scenario.steps[0];
    assert_eq!(
        scenario
            .adapter_step_support(DockspaceAdapterTag::Headless, step)
            .expect("group fixture should declare a headless contract"),
        DockspaceAdapterStepSupport::Supported
    );

    let mut runner = DockspaceHeadlessRunner::from_scenario(scenario, "source")
        .expect("whole-group fixture should build headlessly");
    assert_eq!(
        runner
            .run_step(step)
            .expect("whole-group move should commit"),
        DockActionOutcome::Changed
    );
    assert!(
        runner.graph().root(&DockSpaceId::from("source")).is_none(),
        "the source DockSpace should be empty after moving its entire tab group"
    );
    assert_eq!(
        tabs_items_for(runner.graph(), "target", "A"),
        vec![
            DockItemId::new("C"),
            DockItemId::new("A"),
            DockItemId::new("B")
        ]
    );
    let (tabs, _) = runner
        .graph()
        .find_item_in_space(&DockSpaceId::from("target"), &DockItemId::new("A"))
        .expect("moved group should be reachable in the target space");
    assert_eq!(
        runner.graph().selected_item_in_tabs(tabs),
        Some(DockItemId::new("B")),
        "moving a whole group should preserve its selected tab"
    );
}

#[test]
fn provider_first_fixture_is_explicitly_optional_until_reservation_support_exists() {
    let scenarios =
        load_builtin_scenario_fixtures().expect("builtin dockspace fixtures should parse");
    let scenario = scenario_by_id(&scenarios, "provider_first_destination_loss_contract");
    let step = &scenario.steps[0];
    let support = scenario
        .adapter_step_support(DockspaceAdapterTag::Headless, step)
        .expect("provider-first fixture should declare a headless contract");
    let DockspaceAdapterStepSupport::OptionalUnsupported { capabilities } = support else {
        panic!("provider-first work must remain explicitly unsupported before U5");
    };
    assert!(capabilities.contains(&DockspaceAdapterCapability::ProviderFirstReservation));
    assert!(capabilities.contains(&DockspaceAdapterCapability::Recovery));
    assert!(capabilities.contains(&DockspaceAdapterCapability::ShutdownDrain));

    let phases = scenario
        .checkpoints
        .iter()
        .map(|checkpoint| checkpoint.phase)
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            DockspaceCheckpointPhase::AfterPrepare,
            DockspaceCheckpointPhase::AfterProviderReceipt,
            DockspaceCheckpointPhase::AfterRecoveryTransfer,
            DockspaceCheckpointPhase::Terminal,
            DockspaceCheckpointPhase::AfterTeardown,
        ]
    );
    assert_eq!(
        scenario.checkpoints[1].facts.progress,
        Some(DockspaceProgressState::ForwardOnlyPrepared)
    );
    assert_eq!(
        scenario.checkpoints[2].facts.progress,
        Some(DockspaceProgressState::RecoveryPending)
    );
    assert_eq!(
        scenario.checkpoints[3].facts.terminal,
        Some(DockspaceDragTerminal::Recovered)
    );
}
