//! Internal benchmark fixtures for the docking hot path.

use crate::{
    DockGraph, DockItemId, DockNode, DockNodeId, DockPolicy, DockSpaceId,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{
        DockEmptySpaceDropTarget, DockLeafDropTarget, DockRootDropTarget, DockTabBarDropTarget,
        DockTabLabelDropTarget,
    },
};
use open_gpui::{Bounds, Pixels, Point, Size, point, px, size};
use std::hint::black_box;

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
        assert!(
            stack_count >= 2,
            "the benchmark needs a source and a target stack"
        );
        assert!(
            tabs_per_stack > 0,
            "each benchmark stack needs at least one tab"
        );

        let mut graph = DockGraph::new();
        let mut stack_ids = Vec::with_capacity(stack_count);
        for stack_index in 0..stack_count {
            let item = DockItemId::new(format!("panel-{stack_index}"));
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
        let mut scene = DockHostDropScene::new(point(px(0.0), px(0.0)));
        scene.push_fact(DockHostDropSceneFact::Root(DockRootDropTarget {
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

            scene.push_fact(DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
                target_tabs,
                insert_index: tabs_per_stack,
                bounds: tab_bar_bounds,
                is_central: stack_index == 0,
            }));
            scene.push_fact(DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root,
                target_tabs,
                bounds: stack_bounds,
                is_central: stack_index == 0,
            }));

            for tab_index in 0..tabs_per_stack {
                let label_bounds = rect(x + tab_index as f32 * tab_width, y, tab_width, tab_height);
                scene.push_fact(DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
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

        let space = DockSpaceId::new("benchmark");
        scene.push_fact(DockHostDropSceneFact::EmptySpace(
            DockEmptySpaceDropTarget {
                space,
                bounds: host_bounds,
                is_central: false,
            },
        ));

        let excluded_nodes = vec![stack_ids[0]];
        let payload_size = size(px(420.0), px(260.0));
        Self {
            scene,
            policy: DockPolicy::default(),
            excluded_nodes,
            positions,
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
                    black_box(self.excluded_nodes.clone()),
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

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}
