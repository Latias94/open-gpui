use crate::{DockItemId, DockNodeId, DockSpaceId, split_fraction};
use open_gpui::{Bounds, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use std::collections::HashMap;

#[path = "graph_canonical.rs"]
mod graph_canonical;
#[path = "graph_edge_dock.rs"]
mod graph_edge_dock;
#[path = "graph_floating_mutation.rs"]
mod graph_floating_mutation;
#[path = "graph_mutation.rs"]
mod graph_mutation;
#[path = "graph_node_validation.rs"]
mod graph_node_validation;
#[path = "graph_op_validation.rs"]
mod graph_op_validation;
#[path = "graph_ops.rs"]
mod graph_ops;
#[path = "graph_space_validation.rs"]
mod graph_space_validation;
#[path = "graph_split_validation.rs"]
mod graph_split_validation;
#[path = "graph_tab_stack.rs"]
mod graph_tab_stack;
#[path = "graph_validation.rs"]
mod graph_validation;
pub use graph_validation::DockGraphValidationError;

/// Axis used by split dock nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    /// Children are laid out left to right.
    Horizontal,
    /// Children are laid out top to bottom.
    Vertical,
}

/// Drop zone used when docking into an existing node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropZone {
    /// Merge into the target tabs node.
    Center,
    /// Split to the target's left side.
    Left,
    /// Split to the target's right side.
    Right,
    /// Split to the target's top side.
    Top,
    /// Split to the target's bottom side.
    Bottom,
}

/// Runtime node in a dock graph.
#[derive(Debug, Clone, PartialEq)]
pub enum DockNode {
    /// N-ary split container.
    Split {
        /// Split axis.
        axis: SplitAxis,
        /// Child nodes.
        children: Vec<DockNodeId>,
        /// Normalized child fractions.
        fractions: Vec<f32>,
    },
    /// Tab stack containing dock item ids.
    Tabs {
        /// Items in tab order.
        items: Vec<DockItemId>,
        /// Active item index.
        active: usize,
    },
    /// In-window floating container.
    Floating {
        /// Child root rendered inside the floating container.
        child: DockNodeId,
    },
}

/// A pure decision describing how an edge dock will mutate the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDockDecision {
    /// Insert a new child into an existing same-axis split.
    InsertIntoSplit {
        /// The split container receiving the new child.
        split: DockNodeId,
        /// Existing child whose share will be split.
        anchor_index: usize,
        /// Position where the new child will be inserted.
        insert_index: usize,
    },
    /// Wrap the target in a new split.
    WrapNewSplit,
}

/// In-window floating container metadata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockFloatingContainer {
    /// Floating node id.
    pub node: DockNodeId,
    /// Container bounds relative to the dock host.
    pub bounds: Bounds<Pixels>,
}

/// Retained docking graph for one or more logical dock spaces.
#[derive(Debug, Default)]
pub struct DockGraph {
    nodes: SlotMap<DockNodeId, DockNode>,
    roots: HashMap<DockSpaceId, DockNodeId>,
    floatings: HashMap<DockSpaceId, Vec<DockFloatingContainer>>,
}

impl DockGraph {
    /// Creates an empty dock graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a node and returns its runtime id.
    pub fn insert_node(&mut self, node: DockNode) -> DockNodeId {
        self.nodes.insert(node)
    }

    /// Returns a node by id.
    pub fn node(&self, id: DockNodeId) -> Option<&DockNode> {
        self.nodes.get(id)
    }

    /// Sets the root node for a dock space.
    pub fn set_root(&mut self, space: DockSpaceId, root: DockNodeId) {
        self.roots.insert(space, root);
    }

    /// Returns the root node for a dock space.
    pub fn root(&self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.roots.get(space).copied()
    }

    /// Removes and returns the root node for a dock space.
    pub fn remove_root(&mut self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.roots.remove(space)
    }

    /// Returns all logical dock spaces known to the graph.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        let mut spaces: Vec<DockSpaceId> = self
            .roots
            .keys()
            .chain(self.floatings.keys())
            .cloned()
            .collect();
        spaces.sort();
        spaces.dedup();
        spaces
    }

    /// Returns floating containers for a dock space.
    pub fn floating_containers(&self, space: &DockSpaceId) -> &[DockFloatingContainer] {
        self.floatings
            .get(space)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn floating_container(
        &self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Option<&DockFloatingContainer> {
        self.floatings
            .get(space)?
            .iter()
            .find(|container| container.node == floating)
    }

    /// Returns mutable floating containers for a dock space.
    pub fn floating_containers_mut(
        &mut self,
        space: DockSpaceId,
    ) -> &mut Vec<DockFloatingContainer> {
        self.floatings.entry(space).or_default()
    }

    /// Computes layout bounds for a subtree into `out`.
    pub fn compute_layout(
        &self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        out: &mut HashMap<DockNodeId, Bounds<Pixels>>,
    ) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };

        out.insert(root, bounds);
        match node {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                self.compute_layout(*child, bounds, out);
            }
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                if children.is_empty() {
                    return;
                }

                let shares = split_fraction::cleaned_shares(children.len(), fractions);
                let mut cursor = 0.0_f32;
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                let x = f32::from(bounds.origin.x);
                let y = f32::from(bounds.origin.y);

                for (child, share) in children.iter().copied().zip(shares) {
                    let (child_bounds, next_cursor) = match axis {
                        SplitAxis::Horizontal => {
                            let child_width = width * share;
                            (
                                Bounds::new(
                                    point(px(x + cursor), bounds.origin.y),
                                    size(px(child_width), bounds.size.height),
                                ),
                                cursor + child_width,
                            )
                        }
                        SplitAxis::Vertical => {
                            let child_height = height * share;
                            (
                                Bounds::new(
                                    point(bounds.origin.x, px(y + cursor)),
                                    size(bounds.size.width, px(child_height)),
                                ),
                                cursor + child_height,
                            )
                        }
                    };

                    cursor = next_cursor;
                    self.compute_layout(child, child_bounds, out);
                }
            }
        }
    }

    /// Returns all dock items reachable from a dock space.
    pub fn collect_items_in_space(&self, space: &DockSpaceId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        if let Some(root) = self.root(space) {
            self.collect_items_in_subtree_into(root, &mut out);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.collect_items_in_subtree_into(floating.node, &mut out);
            }
        }
        out
    }

    /// Returns all dock items reachable from a subtree.
    pub fn collect_items_in_subtree(&self, root: DockNodeId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        self.collect_items_in_subtree_into(root, &mut out);
        out
    }

    /// Returns true when an item is reachable from any dock space.
    pub fn contains_item(&self, item: &DockItemId) -> bool {
        self.spaces()
            .iter()
            .any(|space| self.find_item_in_space(space, item).is_some())
    }

    /// Finds an item in a dock space and returns its tabs node and tab index.
    pub fn find_item_in_space(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        if let Some(root) = self.root(space)
            && let Some(found) = self.find_item_in_subtree(root, item)
        {
            return Some(found);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings
                .iter()
                .find_map(|floating| self.find_item_in_subtree(floating.node, item))
        })
    }

    /// Returns the root that contains a node within a dock space forest.
    pub fn root_for_node_in_space(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> Option<DockNodeId> {
        if let Some(root) = self.root(space)
            && self.subtree_contains(root, target)
        {
            return Some(root);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings.iter().find_map(|floating| {
                self.subtree_contains(floating.node, target)
                    .then_some(floating.node)
            })
        })
    }

    fn find_item_in_subtree(
        &self,
        root: DockNodeId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        match self.nodes.get(root)? {
            DockNode::Tabs { items, .. } => items
                .iter()
                .position(|candidate| candidate == item)
                .map(|index| (root, index)),
            DockNode::Floating { child } => self.find_item_in_subtree(*child, item),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .find_map(|child| self.find_item_in_subtree(child, item)),
        }
    }

    fn collect_items_in_subtree_into(&self, root: DockNodeId, out: &mut Vec<DockItemId>) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };
        match node {
            DockNode::Tabs { items, .. } => out.extend(items.iter().cloned()),
            DockNode::Floating { child } => self.collect_items_in_subtree_into(*child, out),
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_items_in_subtree_into(*child, out);
                }
            }
        }
    }

    fn subtree_contains(&self, root: DockNodeId, target: DockNodeId) -> bool {
        if root == target {
            return true;
        }
        let Some(node) = self.nodes.get(root) else {
            return false;
        };
        match node {
            DockNode::Tabs { .. } => false,
            DockNode::Floating { child } => self.subtree_contains(*child, target),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .any(|child| self.subtree_contains(child, target)),
        }
    }
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}
