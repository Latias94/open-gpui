//! Internal benchmark fixtures for the docking hot path.

use crate::{
    DockActionOutcome, DockDropGuideMetrics, DockGraph, DockItemId, DockLayout, DockNode,
    DockNodeId, DockPolicy, DockSpaceId, DockViewportHostGeometry, SplitAxis,
    canonical_runtime::CanonicalGraphRuntime,
    dockspace_testkit::{
        fixtures::load_builtin_scenario_fixtures,
        headless::DockspaceHeadlessRunner,
        ops::{DockspaceScenarioStep, DockspaceScenarioZone},
        topology::{DockspaceScenario, DockspaceTopologyNode},
    },
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabBarDropTarget, DockTabLabelDropTarget,
    },
    graph_assembly::DockGraphAssembly,
    viewport_drop_scene::{DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot},
    viewport_registry::DockViewportWindowBoundsFrame,
};
use open_gpui::{Bounds, Pixels, Point, Size, WindowId, point, px, size};
use open_gpui_ui_core::normalize_split_fractions;
use std::{fmt::Write, hint::black_box};

#[doc(hidden)]
pub use crate::dockspace_testkit::benchmarks::DockspaceBenchmarkCase;

/// A full headless workspace transaction sequence backed by the shared scenario corpus.
#[doc(hidden)]
pub struct DockspaceScenarioSequenceBenchmark {
    runner: DockspaceHeadlessRunner,
    steps: Vec<DockspaceScenarioStep>,
    initial_layout: DockLayout,
    workload_contract: String,
}

/// Canonical binary-kernel benchmark for a repeated whole-group edge move.
///
/// The source and target groups remain stable while the local parent branch is replaced on every
/// operation. This isolates prepared-patch and local commit cost from graph construction and from
/// the retained host hit-test scene.
#[doc(hidden)]
pub struct CanonicalGroupEdgePatchBenchmark {
    runtime: CanonicalGraphRuntime,
}

impl CanonicalGroupEdgePatchBenchmark {
    pub fn new() -> Self {
        Self::with_space_count(1)
    }

    /// Builds the same local two-group workload with unrelated retained dock spaces.
    ///
    /// A canonical local patch should not become materially more expensive merely because the
    /// graph authority owns other spaces that are absent from its read and write sets.
    pub fn with_space_count(space_count: usize) -> Self {
        assert!(space_count > 0, "canonical benchmark needs a main dock space");
        let mut assembly = DockGraphAssembly::new();
        assembly
            .add_space("main")
            .expect("canonical benchmark space should be valid");
        let source = assembly
            .add_group(
                "source",
                [DockItemId::new("source-item")],
                DockItemId::new("source-item"),
            )
            .expect("canonical benchmark source group should be valid");
        let target = assembly
            .add_group(
                "target",
                [DockItemId::new("target-item")],
                DockItemId::new("target-item"),
            )
            .expect("canonical benchmark target group should be valid");
        let root = assembly
            .add_branch(SplitAxis::Horizontal, source, target, 0.5)
            .expect("canonical benchmark root should be valid");
        assembly
            .set_root("main", root)
            .expect("canonical benchmark root should attach");

        for index in 1..space_count {
            let space = format!("unrelated-space-{index}");
            let first_group = format!("unrelated-group-{index}-first");
            let second_group = format!("unrelated-group-{index}-second");
            let first_item = DockItemId::new(format!("unrelated-item-{index}-first"));
            let second_item = DockItemId::new(format!("unrelated-item-{index}-second"));
            assembly
                .add_space(space.clone())
                .expect("unrelated canonical benchmark space should be valid");
            let first = assembly
                .add_group(first_group, [first_item.clone()], first_item)
                .expect("unrelated canonical benchmark group should be valid");
            let second = assembly
                .add_group(second_group, [second_item.clone()], second_item)
                .expect("unrelated canonical benchmark group should be valid");
            let root = assembly
                .add_branch(SplitAxis::Vertical, first, second, 0.5)
                .expect("unrelated canonical benchmark root should be valid");
            assembly
                .set_root(space, root)
                .expect("unrelated canonical benchmark root should attach");
        }

        Self {
            runtime: CanonicalGraphRuntime::from_assembly(assembly)
                .expect("canonical benchmark graph should publish"),
        }
    }

    #[inline(never)]
    pub fn run_operation(&mut self, _iteration: usize) -> bool {
        let prepared = match self
            .runtime
            .prepare_move_group_to_edge(
                "source",
                "target",
                "main",
                "main",
                SplitAxis::Horizontal,
                true,
                0.5,
            )
            .expect("canonical benchmark patch should prepare")
        {
            crate::graph_patch::GraphPrepareOutcome::Prepared(prepared) => prepared,
            crate::graph_patch::GraphPrepareOutcome::NoOp(_) => return false,
        };
        matches!(
            self.runtime.commit(prepared),
            crate::graph_patch::GraphCommitOutcome::Committed(_)
        )
    }

    pub fn validate_state(&self) -> bool {
        self.runtime.graph().validate().is_ok()
    }
}

impl DockspaceScenarioSequenceBenchmark {
    /// Builds the eight-step three-space rehome cycle used by functional regression tests.
    pub fn three_dockspace_rehome_cycle() -> Self {
        let scenarios = load_builtin_scenario_fixtures()
            .expect("builtin dockspace benchmark fixtures should parse");
        let scenario = scenarios
            .into_iter()
            .find(|scenario| scenario.id == "three_dockspace_rehome_cycle")
            .expect("three-dockspace rehome benchmark scenario should exist");
        let primary_space = scenario
            .primary_space
            .clone()
            .or_else(|| scenario.spaces.first().map(|space| space.id.clone()))
            .expect("three-dockspace rehome benchmark scenario should declare a space");
        let runner = DockspaceHeadlessRunner::from_scenario(&scenario, primary_space)
            .expect("three-dockspace rehome benchmark scenario should build");
        let initial_layout = runner.graph().export_layout();
        let workload_contract = scenario_workload_contract(&scenario);

        Self {
            runner,
            steps: scenario.steps,
            initial_layout,
            workload_contract,
        }
    }

    /// Runs one complete cycle through production workspace transactions.
    #[inline(never)]
    pub fn run_cycle(&mut self, _iteration: usize) -> bool {
        for step in &self.steps {
            if !matches!(self.runner.run_step(step), Ok(DockActionOutcome::Changed)) {
                return false;
            }
        }
        black_box(true)
    }

    /// Returns the number of workspace transactions in one measured cycle.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns whether the complete cycle restored the exact initial serialized topology.
    pub fn validate_cycle_state(&self) -> bool {
        cycle_state_matches(self.runner.graph(), &self.initial_layout)
    }

    /// Returns the topology-and-operation contract shared with the Dear ImGui benchmark.
    pub fn workload_contract(&self) -> &str {
        &self.workload_contract
    }
}

fn cycle_state_matches(graph: &DockGraph, initial_layout: &DockLayout) -> bool {
    graph.validate_canonical().is_ok() && graph.export_layout() == *initial_layout
}

fn scenario_workload_contract(scenario: &DockspaceScenario) -> String {
    let mut contract = String::from("dockspace-sequence-v2|spaces=");
    for (index, space) in scenario.spaces.iter().enumerate() {
        if index > 0 {
            contract.push(';');
        }
        write!(&mut contract, "{}=", space.id).expect("writing to a String cannot fail");
        let root = space
            .root
            .as_ref()
            .expect("the benchmark contract requires a root in every dockspace");
        append_topology_contract(&mut contract, root);
        assert!(
            space.floatings.is_empty(),
            "the three-dockspace benchmark contract does not include floating containers"
        );
        assert!(
            !space.keep_central_when_empty && !space.passthrough_when_empty,
            "the three-dockspace benchmark contract does not include central-region empty-space policy"
        );
    }

    contract.push_str("|steps=");
    for (index, step) in scenario.steps.iter().enumerate() {
        if index > 0 {
            contract.push(';');
        }
        let crate::dockspace_testkit::ops::DockspaceSemanticOperation::MoveItemToItem {
            source_space,
            item,
            target_space,
            target_item,
            zone,
        } = step.operation()
        else {
            panic!("the three-dockspace benchmark contract only supports named item moves");
        };
        write!(
            &mut contract,
            "{item}@{source_space}>{target_item}@{target_space}:{}",
            scenario_zone_name(*zone)
        )
        .expect("writing to a String cannot fail");
    }
    contract.push_str("|final=initial");
    contract
}

fn append_topology_contract(contract: &mut String, node: &DockspaceTopologyNode) {
    match node {
        DockspaceTopologyNode::Tabs {
            items, selected, ..
        } => {
            let selected = selected
                .as_ref()
                .or_else(|| items.first())
                .expect("the benchmark contract requires non-empty tabs");
            write!(contract, "T[selected={selected};items=")
                .expect("writing to a String cannot fail");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    contract.push(',');
                }
                contract.push_str(item.as_str());
            }
            contract.push(']');
        }
        DockspaceTopologyNode::Split {
            axis,
            children,
            fractions,
            ..
        } => {
            contract.push('S');
            contract.push_str(match axis {
                SplitAxis::Horizontal => "horizontal",
                SplitAxis::Vertical => "vertical",
            });
            contract.push_str("[fractions=");
            let mut fractions = if fractions.is_empty() {
                vec![1.0; children.len()]
            } else {
                fractions.clone()
            };
            normalize_split_fractions(&mut fractions);
            for (index, fraction) in fractions.iter().copied().enumerate() {
                if index > 0 {
                    contract.push(',');
                }
                append_contract_fraction(contract, fraction);
            }
            contract.push_str("](");
            for (index, child) in children.iter().enumerate() {
                if index > 0 {
                    contract.push(',');
                }
                append_topology_contract(contract, child);
            }
            contract.push(')');
        }
    }
}

fn append_contract_fraction(contract: &mut String, fraction: f32) {
    let start = contract.len();
    write!(contract, "{fraction:.6}").expect("writing to a String cannot fail");
    while contract.len() > start && contract.ends_with('0') {
        contract.pop();
    }
    if contract.ends_with('.') {
        contract.pop();
    }
}

const fn scenario_zone_name(zone: DockspaceScenarioZone) -> &'static str {
    match zone {
        DockspaceScenarioZone::Center => "center",
        DockspaceScenarioZone::Left => "left",
        DockspaceScenarioZone::Right => "right",
        DockspaceScenarioZone::Top => "top",
        DockspaceScenarioZone::Bottom => "bottom",
    }
}

#[cfg(test)]
mod scenario_sequence_tests {
    use super::*;

    #[test]
    fn cycle_state_validation_rejects_unreachable_nodes_hidden_from_layout_export() {
        let benchmark = DockspaceScenarioSequenceBenchmark::three_dockspace_rehome_cycle();
        let mut graph = benchmark.runner.graph().clone();
        graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::new("orphan")],
            selected: Some(DockItemId::new("orphan")),
        });

        assert_eq!(graph.export_layout(), benchmark.initial_layout);
        assert!(!cycle_state_matches(&graph, &benchmark.initial_layout));
    }
}

/// A deterministic retained scene that exercises the production tab-drag resolver.
#[doc(hidden)]
pub struct DockDragBenchmark {
    scene: DockHostDropScene,
    policy: DockPolicy,
    excluded_nodes: Vec<DockNodeId>,
    positions: Vec<Point<Pixels>>,
    payload_size: Size<Pixels>,
}

impl DockDragBenchmark {
    /// Builds a grid of tab stacks with realistic tab-label, tab-bar, and leaf facts.
    pub fn new(stack_count: usize, tabs_per_stack: usize) -> Self {
        let mut graph = DockGraph::new();
        let fixture = build_tab_drag_scene(
            &mut graph,
            DockSpaceId::new("benchmark"),
            stack_count,
            tabs_per_stack,
        );
        let mut scene = DockHostDropScene::new(point(px(0.0), px(0.0)));
        for fact in fixture.facts {
            scene.push_fact(fact);
        }
        let excluded_nodes = vec![fixture.root];
        let payload_size = size(px(420.0), px(260.0));
        Self {
            scene,
            policy: DockPolicy::default(),
            excluded_nodes,
            positions: fixture.positions,
            payload_size,
        }
    }

    /// Runs the production per-pointer-move resolver against the retained scene.
    #[inline(never)]
    pub fn resolve_full_move(&self, iteration: usize) -> bool {
        black_box(
            self.scene
                .resolve_pointer_move(
                    black_box(self.position(iteration)),
                    Some(self.payload_size),
                    black_box(self.excluded_nodes.as_slice()),
                    &self.policy,
                    None,
                    None,
                )
                .is_some(),
        )
    }

    fn position(&self, iteration: usize) -> Point<Pixels> {
        self.positions[iteration % self.positions.len()]
    }
}

/// A retained split-plus-floating scene matching the complex dockspace fixture shape.
#[doc(hidden)]
pub struct DockComplexDockspaceBenchmark {
    scene: DockHostDropScene,
    policy: DockPolicy,
    excluded_nodes: Vec<DockNodeId>,
    positions: Vec<Point<Pixels>>,
    payload_size: Size<Pixels>,
}

impl DockComplexDockspaceBenchmark {
    /// Builds the `AAA, BBB | CCC + DDD` resolver fixture used by scenario tests.
    pub fn tabs_split_float_merge() -> Self {
        let mut graph = DockGraph::new();
        let tabs_ab = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::new("AAA"), DockItemId::new("BBB")],
            selected: Some(DockItemId::new("AAA")),
        });
        let tabs_c = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::new("CCC")],
            selected: Some(DockItemId::new("CCC")),
        });
        let floating_child = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::new("DDD")],
            selected: Some(DockItemId::new("DDD")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: crate::SplitAxis::Horizontal,
            children: vec![tabs_ab, tabs_c],
            fractions: vec![0.5, 0.5],
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_child,
        });
        graph.set_root(DockSpaceId::new("main"), root);

        let root_bounds = rect(0.0, 0.0, 720.0, 320.0);
        let left_leaf = rect(0.0, 0.0, 350.0, 320.0);
        let right_leaf = rect(370.0, 0.0, 350.0, 320.0);
        let left_tab_bar = rect(0.0, 0.0, 350.0, 28.0);
        let right_tab_bar = rect(370.0, 0.0, 350.0, 28.0);
        let floating_bounds = rect(64.0, 360.0, 320.0, 220.0);
        let floating_title = rect(64.0, 360.0, 320.0, 32.0);

        let mut scene = DockHostDropScene::new(point(px(0.0), px(0.0)));
        for fact in [
            DockHostDropSceneFact::Root(DockRootDropTarget {
                root,
                bounds: root_bounds,
            }),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root,
                target_tabs: tabs_ab,
                bounds: left_leaf,
                is_central: true,
            }),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root,
                target_tabs: tabs_c,
                bounds: right_leaf,
                is_central: false,
            }),
            DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
                target_tabs: tabs_ab,
                insert_index: 2,
                bounds: left_tab_bar,
                is_central: true,
            }),
            DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
                target_tabs: tabs_c,
                insert_index: 1,
                bounds: right_tab_bar,
                is_central: false,
            }),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs_ab,
                target_index: 0,
                bounds: rect(0.0, 0.0, 88.0, 28.0),
                is_central: true,
            }),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs_ab,
                target_index: 1,
                bounds: rect(88.0, 0.0, 88.0, 28.0),
                is_central: true,
            }),
            DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs: tabs_c,
                target_index: 0,
                bounds: rect(370.0, 0.0, 88.0, 28.0),
                is_central: false,
            }),
            DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
                floating,
                target_tabs: floating_child,
                title_bounds: floating_title,
                preview_bounds: floating_bounds,
            }),
            DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                space: DockSpaceId::new("main"),
                bounds: root_bounds,
                is_central: false,
            }),
        ] {
            scene.push_fact(fact);
        }

        Self {
            scene,
            policy: DockPolicy::default(),
            excluded_nodes: vec![tabs_ab],
            positions: vec![
                rect(370.0, 0.0, 88.0, 28.0).center(),
                point(px(704.0), px(160.0)),
                floating_title.center(),
            ],
            payload_size: size(px(420.0), px(260.0)),
        }
    }

    /// Runs the production resolver across tab, inner-edge, and floating-title targets.
    #[inline(never)]
    pub fn resolve_complex_move(&self, iteration: usize) -> bool {
        black_box(
            self.scene
                .resolve_pointer_move(
                    black_box(self.position(iteration)),
                    Some(self.payload_size),
                    black_box(self.excluded_nodes.as_slice()),
                    &self.policy,
                    None,
                    None,
                )
                .is_some(),
        )
    }

    fn position(&self, iteration: usize) -> Point<Pixels> {
        self.positions[iteration % self.positions.len()]
    }
}

/// A deterministic multi-viewport fixture for the cross-window known-target hot path.
#[doc(hidden)]
pub struct DockMultiViewDragBenchmark {
    registry: DockViewportHostSceneRegistry,
    policy: DockPolicy,
    excluded_nodes: Vec<DockNodeId>,
    targets: Vec<DockMultiViewDragTarget>,
    payload_size: Size<Pixels>,
}

struct DockMultiViewDragTarget {
    space: DockSpaceId,
    window_id: WindowId,
    positions: Vec<Point<Pixels>>,
}

impl DockMultiViewDragBenchmark {
    /// Builds multiple registered viewport host scenes and resolves against the selected target.
    pub fn new(viewport_count: usize, stacks_per_viewport: usize, tabs_per_stack: usize) -> Self {
        assert!(
            viewport_count > 0,
            "the benchmark needs at least one viewport"
        );
        let mut graph = DockGraph::new();
        let source_item = DockItemId::new("source-panel");
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![source_item.clone()],
            selected: Some(source_item),
        });
        let mut registry = DockViewportHostSceneRegistry::default();
        let mut targets = Vec::with_capacity(viewport_count);

        for viewport_index in 0..viewport_count {
            let space = DockSpaceId::new(format!("benchmark-view-{viewport_index}"));
            let window_id = WindowId::from(20_000 + viewport_index as u64);
            let fixture = build_tab_drag_scene(
                &mut graph,
                space.clone(),
                stacks_per_viewport,
                tabs_per_stack,
            );
            let geometry = DockViewportHostGeometry::identity_with_hit_region_for_test(
                fixture.host_bounds,
                fixture.host_bounds,
            );
            let snapshot = DockViewportHostSceneSnapshot::new_with_facts(
                space.clone(),
                window_id,
                DockViewportWindowBoundsFrame::WindowLocal(fixture.host_bounds),
                geometry,
                point(px(0.0), px(0.0)),
                DockDropGuideMetrics::default(),
                fixture.facts,
            );
            registry.register(snapshot);
            targets.push(DockMultiViewDragTarget {
                space,
                window_id,
                positions: fixture.positions,
            });
        }

        Self {
            registry,
            policy: DockPolicy::default(),
            excluded_nodes: vec![source_tabs],
            targets,
            payload_size: size(px(420.0), px(260.0)),
        }
    }

    /// Resolves one pointer move through the multi-viewport registry with a known target window.
    #[inline(never)]
    pub fn resolve_known_target_move(&self, iteration: usize) -> bool {
        let target = self.target(iteration);
        black_box(
            self.registry
                .resolve_frame_for_window(
                    black_box(&target.space),
                    Some(black_box(target.window_id)),
                    black_box(target.position(iteration)),
                    Some(self.payload_size),
                    black_box(self.excluded_nodes.as_slice()),
                    &self.policy,
                    None,
                    None,
                )
                .is_some(),
        )
    }

    /// Resolves a pointer move against a stale window id and expects fail-closed routing.
    #[inline(never)]
    pub fn reject_unknown_target_move(&self, iteration: usize) -> bool {
        let target = self.target(iteration);
        black_box(
            self.registry
                .resolve_frame_for_window(
                    black_box(&target.space),
                    Some(WindowId::from(99_000_000 + iteration as u64)),
                    black_box(target.position(iteration)),
                    Some(self.payload_size),
                    black_box(self.excluded_nodes.as_slice()),
                    &self.policy,
                    None,
                    None,
                )
                .is_none(),
        )
    }

    fn target(&self, iteration: usize) -> &DockMultiViewDragTarget {
        &self.targets[iteration % self.targets.len()]
    }
}

impl DockMultiViewDragTarget {
    fn position(&self, iteration: usize) -> Point<Pixels> {
        self.positions[iteration % self.positions.len()]
    }
}

struct DockTabDragSceneFixture {
    facts: Vec<DockHostDropSceneFact>,
    positions: Vec<Point<Pixels>>,
    host_bounds: Bounds<Pixels>,
    root: DockNodeId,
}

fn build_tab_drag_scene(
    graph: &mut DockGraph,
    space: DockSpaceId,
    stack_count: usize,
    tabs_per_stack: usize,
) -> DockTabDragSceneFixture {
    assert!(
        stack_count >= 2,
        "the benchmark needs a source and a target stack"
    );
    assert!(
        tabs_per_stack > 0,
        "each benchmark stack needs at least one tab"
    );

    let mut stack_ids = Vec::with_capacity(stack_count);
    for stack_index in 0..stack_count {
        let items = (0..tabs_per_stack)
            .map(|tab_index| DockItemId::new(format!("{space}-panel-{stack_index}-{tab_index}")))
            .collect::<Vec<_>>();
        stack_ids.push(graph.insert_node(DockNode::Tabs {
            selected: items.first().cloned(),
            items,
        }));
    }

    let columns = (stack_count as f32).sqrt().ceil() as usize;
    let rows = stack_count.div_ceil(columns);
    let tab_width = 88.0;
    let tab_height = 28.0;
    let stack_width = (tabs_per_stack as f32 * tab_width).max(320.0);
    let stack_height = 220.0;
    let gap = 12.0;
    let host_width = columns as f32 * (stack_width + gap) - gap;
    let host_height = rows as f32 * (stack_height + gap) - gap;
    let host_bounds = rect(0.0, 0.0, host_width, host_height);
    let root = stack_ids[0];
    let mut facts = Vec::with_capacity(1 + stack_count * (2 + tabs_per_stack) + 1);
    facts.push(DockHostDropSceneFact::Root(DockRootDropTarget {
        root,
        bounds: host_bounds,
    }));

    let mut positions = Vec::with_capacity((stack_count - 1) * tabs_per_stack);
    for (stack_index, target_tabs) in stack_ids.iter().copied().enumerate() {
        let column = stack_index % columns;
        let row = stack_index / columns;
        let x = column as f32 * (stack_width + gap);
        let y = row as f32 * (stack_height + gap);
        let stack_bounds = rect(x, y, stack_width, stack_height);
        let tab_bar_bounds = rect(x, y, stack_width, tab_height);

        facts.push(DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
            target_tabs,
            insert_index: tabs_per_stack,
            bounds: tab_bar_bounds,
            is_central: stack_index == 0,
        }));
        facts.push(DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root,
            target_tabs,
            bounds: stack_bounds,
            is_central: stack_index == 0,
        }));

        for tab_index in 0..tabs_per_stack {
            let label_bounds = rect(x + tab_index as f32 * tab_width, y, tab_width, tab_height);
            facts.push(DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
                target_tabs,
                target_index: tab_index,
                bounds: label_bounds,
                is_central: stack_index == 0,
            }));
            if stack_index != 0 {
                positions.push(label_bounds.center());
            }
        }
    }

    facts.push(DockHostDropSceneFact::EmptySpace(
        DockEmptySpaceDropTarget {
            space,
            bounds: host_bounds,
            is_central: false,
        },
    ));

    DockTabDragSceneFixture {
        facts,
        positions,
        host_bounds,
        root,
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}
