use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use std::collections::HashMap;

#[path = "graph_canonical.rs"]
mod graph_canonical;
#[path = "graph_edge_dock.rs"]
mod graph_edge_dock;
#[path = "graph_mutation.rs"]
mod graph_mutation;
#[path = "graph_op_validation.rs"]
mod graph_op_validation;
#[path = "graph_ops.rs"]
mod graph_ops;
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

                let shares = cleaned_layout_shares(children.len(), fractions);
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

    /// Decides whether an edge dock will insert into an existing same-axis split.
    pub fn edge_dock_decision(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
    ) -> Option<EdgeDockDecision> {
        if zone == DropZone::Center {
            return None;
        }

        let axis = zone.axis()?;
        let index = self.build_parent_index(space);
        if !index.root_for.contains_key(&target) {
            return None;
        }

        if let Some(DockNode::Split {
            axis: split_axis,
            children,
            fractions,
        }) = self.nodes.get(target)
            && *split_axis == axis
            && !children.is_empty()
            && children.len() == fractions.len()
        {
            let len = children.len();
            let (anchor_index, insert_index) = match zone {
                DropZone::Left | DropZone::Top => (0, 0),
                DropZone::Right | DropZone::Bottom => {
                    let last = len.saturating_sub(1);
                    (last, last.saturating_add(1))
                }
                DropZone::Center => unreachable!(),
            };
            return Some(EdgeDockDecision::InsertIntoSplit {
                split: target,
                anchor_index,
                insert_index,
            });
        }

        let mut cur = target;
        while let Some(parent) = index.parent.get(&cur).copied() {
            let Some(DockNode::Split {
                axis: split_axis,
                children,
                fractions,
            }) = self.nodes.get(parent)
            else {
                cur = parent;
                continue;
            };

            if *split_axis == axis && !children.is_empty() && children.len() == fractions.len() {
                let Some(anchor_index) = index.split_child_index.get(&cur).copied() else {
                    break;
                };
                let insert_index = match zone {
                    DropZone::Left | DropZone::Top => anchor_index,
                    DropZone::Right | DropZone::Bottom => anchor_index.saturating_add(1),
                    DropZone::Center => unreachable!(),
                };
                return Some(EdgeDockDecision::InsertIntoSplit {
                    split: parent,
                    anchor_index,
                    insert_index,
                });
            }

            cur = parent;
        }

        Some(EdgeDockDecision::WrapNewSplit)
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

    fn build_parent_index(&self, space: &DockSpaceId) -> DockParentIndex {
        let mut index = DockParentIndex::default();
        if let Some(root) = self.root(space) {
            self.index_subtree(root, root, &mut index);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.index_subtree(floating.node, floating.node, &mut index);
            }
        }
        index
    }

    fn index_subtree(&self, root: DockNodeId, node: DockNodeId, index: &mut DockParentIndex) {
        if index.root_for.contains_key(&node) {
            return;
        }
        index.root_for.insert(node, root);
        let Some(current) = self.nodes.get(node) else {
            return;
        };
        match current {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                index.parent.insert(*child, node);
                self.index_subtree(root, *child, index);
            }
            DockNode::Split { children, .. } => {
                for (child_index, child) in children.iter().copied().enumerate() {
                    index.parent.insert(child, node);
                    index.split_child_index.insert(child, child_index);
                    self.index_subtree(root, child, index);
                }
            }
        }
    }
}

impl DropZone {
    fn axis(self) -> Option<SplitAxis> {
        match self {
            DropZone::Left | DropZone::Right => Some(SplitAxis::Horizontal),
            DropZone::Top | DropZone::Bottom => Some(SplitAxis::Vertical),
            DropZone::Center => None,
        }
    }
}

#[derive(Default)]
struct DockParentIndex {
    root_for: HashMap<DockNodeId, DockNodeId>,
    parent: HashMap<DockNodeId, DockNodeId>,
    split_child_index: HashMap<DockNodeId, usize>,
}

fn normalize_shares(shares: &mut Vec<f32>) {
    for share in shares.iter_mut() {
        if !share.is_finite() || *share < 0.0 {
            *share = 0.0;
        }
    }

    let sum: f32 = shares.iter().sum();
    if !sum.is_finite() || sum <= f32::EPSILON {
        let len = shares.len().max(1);
        *shares = vec![1.0 / len as f32; len];
        return;
    }

    for share in shares.iter_mut() {
        *share /= sum;
    }

    if !shares.is_empty() {
        let rest: f32 = shares.iter().take(shares.len().saturating_sub(1)).sum();
        let last = shares.len().saturating_sub(1);
        shares[last] = (1.0 - rest).clamp(0.0, 1.0);
    }
}

fn cleaned_layout_shares(len: usize, fractions: &[f32]) -> Vec<f32> {
    let mut shares: Vec<f32> = (0..len)
        .map(|index| fractions.get(index).copied().unwrap_or(1.0))
        .collect();
    normalize_shares(&mut shares);
    shares
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}
