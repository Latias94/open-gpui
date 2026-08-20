//! Internal benchmark fixtures for the docking hot path.

use crate::{
    DockDropGuideMetrics, DockGraph, DockItemId, DockNode, DockNodeId, DockPolicy, DockSpaceId,
    DockViewportHostGeometry,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabBarDropTarget, DockTabLabelDropTarget,
    },
    viewport_drop_scene::{DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot},
    viewport_registry::DockViewportWindowBoundsFrame,
};
use open_gpui::{Bounds, Pixels, Point, Size, WindowId, point, px, size};
use std::hint::black_box;

#[doc(hidden)]
pub use crate::dockspace_testkit::benchmarks::DockspaceBenchmarkCase;

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
        let item = DockItemId::new(format!("{space}-panel-{stack_index}"));
        stack_ids.push(graph.insert_node(DockNode::Tabs {
            items: vec![item.clone()],
            selected: Some(item),
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
