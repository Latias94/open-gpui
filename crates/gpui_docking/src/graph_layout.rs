use crate::{DockNodeId, geometry};
use open_gpui::{Bounds, Pixels};
use std::collections::HashMap;

use super::{DockGraph, DockNode};

impl DockGraph {
    /// Computes layout bounds for a subtree into `out`.
    pub fn compute_layout(
        &self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        out: &mut HashMap<DockNodeId, Bounds<Pixels>>,
    ) {
        let central_node = self.central_node_for_subtree(root);
        self.compute_layout_with_central(root, bounds, out, central_node);
    }

    fn compute_layout_with_central(
        &self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        out: &mut HashMap<DockNodeId, Bounds<Pixels>>,
        central_node: Option<DockNodeId>,
    ) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };

        out.insert(root, bounds);
        match node {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                self.compute_layout_with_central(*child, bounds, out, central_node);
            }
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                if children.is_empty() {
                    return;
                }

                let layout = geometry::DockSplitLayout::from_fractions(
                    children.len(),
                    fractions,
                    self.central_child_index(children, central_node),
                );
                let pane_bounds = layout.pane_bounds(*axis, bounds);
                for (child, child_bounds) in children.iter().copied().zip(pane_bounds) {
                    self.compute_layout_with_central(child, child_bounds, out, central_node);
                }
            }
        }
    }

    fn central_node_for_subtree(&self, root: DockNodeId) -> Option<DockNodeId> {
        self.central_regions
            .values()
            .filter_map(|central| central.node)
            .find(|node| self.subtree_contains(root, *node))
    }

    fn central_child_index(
        &self,
        children: &[DockNodeId],
        central_node: Option<DockNodeId>,
    ) -> Option<usize> {
        let central_node = central_node?;
        children
            .iter()
            .position(|child| self.subtree_contains(*child, central_node))
    }
}
