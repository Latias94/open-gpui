use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use std::collections::HashMap;
use thiserror::Error;

#[path = "graph_mutation.rs"]
mod graph_mutation;
#[path = "graph_ops.rs"]
mod graph_ops;

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

/// Validation error for reachable runtime dock graph state.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DockGraphValidationError {
    /// A dock space root references a missing runtime node.
    #[error("dock space {space} references missing root node {root:?}")]
    SpaceRootMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing root node id.
        root: DockNodeId,
    },
    /// A floating container references a missing runtime node.
    #[error("dock space {space} references missing floating node {floating:?}")]
    FloatingNodeMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing floating node id.
        floating: DockNodeId,
    },
    /// A floating container points at a non-floating node.
    #[error("dock space {space} floating container {floating:?} is not a floating node")]
    FloatingContainerNodeNotFloating {
        /// Dock space id.
        space: DockSpaceId,
        /// Invalid floating container node id.
        floating: DockNodeId,
    },
    /// A floating container has non-finite coordinates or negative size.
    #[error("dock space {space} floating container {floating:?} has invalid bounds")]
    InvalidFloatingBounds {
        /// Dock space id.
        space: DockSpaceId,
        /// Floating container node id.
        floating: DockNodeId,
    },
    /// A reachable node references a missing child node.
    #[error("dock graph references missing node {node:?}")]
    MissingNode {
        /// Missing node id.
        node: DockNodeId,
    },
    /// A reachable node is referenced more than once.
    #[error("dock graph node {node:?} is referenced more than once")]
    DuplicateNodeReference {
        /// Shared node id.
        node: DockNodeId,
    },
    /// A reachable graph subtree contains a cycle.
    #[error("dock graph cycle detected at node {node:?}")]
    CycleDetected {
        /// Cyclic node id.
        node: DockNodeId,
    },
    /// A tabs node has no dock items.
    #[error("tabs node {tabs:?} is empty")]
    EmptyTabs {
        /// Empty tabs node.
        tabs: DockNodeId,
    },
    /// A tabs node has an invalid active index.
    #[error("tabs node {tabs:?} active index {active} out of bounds for length {len}")]
    TabsActiveOutOfBounds {
        /// Tabs node id.
        tabs: DockNodeId,
        /// Invalid active index.
        active: usize,
        /// Item count.
        len: usize,
    },
    /// A dock item appears in more than one reachable tab position.
    #[error(
        "duplicate dock graph item id {item}: first seen in tabs node {first_tabs:?}, duplicated in tabs node {duplicate_tabs:?}"
    )]
    DuplicateItemId {
        /// Duplicate dock item id.
        item: DockItemId,
        /// First tabs node containing the item.
        first_tabs: DockNodeId,
        /// Tabs node containing the duplicate item.
        duplicate_tabs: DockNodeId,
    },
    /// A split node has fewer than two children.
    #[error("split node {split:?} has too few children: {children_len}")]
    SplitTooFewChildren {
        /// Split node id.
        split: DockNodeId,
        /// Child count.
        children_len: usize,
    },
    /// A split node has mismatched child and fraction counts.
    #[error("split node {split:?} has {children_len} children and {fractions_len} fractions")]
    SplitFractionsLenMismatch {
        /// Split node id.
        split: DockNodeId,
        /// Child count.
        children_len: usize,
        /// Fraction count.
        fractions_len: usize,
    },
    /// A split fraction is non-finite or negative.
    #[error("split node {split:?} fraction {index} is invalid")]
    SplitFractionInvalid {
        /// Split node id.
        split: DockNodeId,
        /// Invalid fraction index.
        index: usize,
    },
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

    /// Validates all reachable runtime graph state.
    ///
    /// Orphaned nodes are ignored because graph mutations may leave old runtime node ids behind.
    /// Layout export already drops those nodes; this method checks only roots and floating
    /// containers that are still reachable from dock spaces.
    pub fn validate(&self) -> Result<(), DockGraphValidationError> {
        let mut validator = DockGraphValidator::new(self);

        for (space, root) in &self.roots {
            if self.node(*root).is_none() {
                return Err(DockGraphValidationError::SpaceRootMissing {
                    space: space.clone(),
                    root: *root,
                });
            }
            validator.validate_subtree(*root)?;
        }

        for (space, floatings) in &self.floatings {
            for floating in floatings {
                if !bounds_is_finite_with_non_negative_size(floating.bounds) {
                    return Err(DockGraphValidationError::InvalidFloatingBounds {
                        space: space.clone(),
                        floating: floating.node,
                    });
                }
                match self.node(floating.node) {
                    Some(DockNode::Floating { .. }) => {
                        validator.validate_subtree(floating.node)?;
                    }
                    Some(_) => {
                        return Err(DockGraphValidationError::FloatingContainerNodeNotFloating {
                            space: space.clone(),
                            floating: floating.node,
                        });
                    }
                    None => {
                        return Err(DockGraphValidationError::FloatingNodeMissing {
                            space: space.clone(),
                            floating: floating.node,
                        });
                    }
                }
            }
        }

        Ok(())
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum GraphValidationMark {
    Visiting,
    Done,
}

struct DockGraphValidator<'a> {
    graph: &'a DockGraph,
    marks: HashMap<DockNodeId, GraphValidationMark>,
    items: HashMap<DockItemId, DockNodeId>,
}

impl<'a> DockGraphValidator<'a> {
    fn new(graph: &'a DockGraph) -> Self {
        Self {
            graph,
            marks: HashMap::new(),
            items: HashMap::new(),
        }
    }

    fn validate_subtree(&mut self, node: DockNodeId) -> Result<(), DockGraphValidationError> {
        match self.marks.get(&node).copied() {
            Some(GraphValidationMark::Visiting) => {
                return Err(DockGraphValidationError::CycleDetected { node });
            }
            Some(GraphValidationMark::Done) => {
                return Err(DockGraphValidationError::DuplicateNodeReference { node });
            }
            None => {}
        }

        self.marks.insert(node, GraphValidationMark::Visiting);
        let graph_node = self
            .graph
            .node(node)
            .ok_or(DockGraphValidationError::MissingNode { node })?;
        match graph_node {
            DockNode::Tabs { items, active } => {
                self.validate_tabs(node, items, *active)?;
            }
            DockNode::Floating { child } => {
                self.validate_subtree(*child)?;
            }
            DockNode::Split {
                children,
                fractions,
                ..
            } => {
                self.validate_split(node, children, fractions)?;
                for child in children {
                    self.validate_subtree(*child)?;
                }
            }
        }
        self.marks.insert(node, GraphValidationMark::Done);
        Ok(())
    }

    fn validate_tabs(
        &mut self,
        tabs: DockNodeId,
        items: &[DockItemId],
        active: usize,
    ) -> Result<(), DockGraphValidationError> {
        if items.is_empty() {
            return Err(DockGraphValidationError::EmptyTabs { tabs });
        }
        if active >= items.len() {
            return Err(DockGraphValidationError::TabsActiveOutOfBounds {
                tabs,
                active,
                len: items.len(),
            });
        }

        for item in items {
            if let Some(first_tabs) = self.items.insert(item.clone(), tabs) {
                return Err(DockGraphValidationError::DuplicateItemId {
                    item: item.clone(),
                    first_tabs,
                    duplicate_tabs: tabs,
                });
            }
        }
        Ok(())
    }

    fn validate_split(
        &self,
        split: DockNodeId,
        children: &[DockNodeId],
        fractions: &[f32],
    ) -> Result<(), DockGraphValidationError> {
        if children.len() < 2 {
            return Err(DockGraphValidationError::SplitTooFewChildren {
                split,
                children_len: children.len(),
            });
        }
        if children.len() != fractions.len() {
            return Err(DockGraphValidationError::SplitFractionsLenMismatch {
                split,
                children_len: children.len(),
                fractions_len: fractions.len(),
            });
        }
        for (index, fraction) in fractions.iter().copied().enumerate() {
            if !fraction.is_finite() || fraction < 0.0 {
                return Err(DockGraphValidationError::SplitFractionInvalid { split, index });
            }
        }
        Ok(())
    }
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

fn bounds_is_finite_with_non_negative_size(bounds: Bounds<Pixels>) -> bool {
    f32::from(bounds.origin.x).is_finite()
        && f32::from(bounds.origin.y).is_finite()
        && f32::from(bounds.size.width).is_finite()
        && f32::from(bounds.size.height).is_finite()
        && f32::from(bounds.size.width) >= 0.0
        && f32::from(bounds.size.height) >= 0.0
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}
