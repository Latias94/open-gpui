use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels, Point, Size, px};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use std::collections::HashMap;

#[path = "graph_canonical.rs"]
mod graph_canonical;
#[path = "graph_edge_dock.rs"]
mod graph_edge_dock;
#[path = "graph_floating_mutation.rs"]
mod graph_floating_mutation;
#[path = "graph_layout.rs"]
mod graph_layout;
#[path = "graph_mutation.rs"]
mod graph_mutation;
#[path = "graph_op_validation.rs"]
mod graph_op_validation;
#[path = "graph_ops.rs"]
mod graph_ops;
#[path = "graph_query.rs"]
mod graph_query;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
        /// Selected item identity.
        selected: Option<DockItemId>,
    },
    /// In-window floating container.
    Floating {
        /// Child root rendered inside the floating container.
        child: DockNodeId,
    },
}

/// Pure n-ary topology plan for an edge dock mutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockEdgeDockPlan {
    /// Insert a new child into an existing same-axis split.
    InsertIntoSplit {
        /// The split container receiving the new child.
        split: DockNodeId,
        /// Original edge zone resolved by preview.
        zone: DropZone,
        /// Existing child whose share was selected by preview-time planning.
        anchor_child: DockNodeId,
        /// Existing child whose share will be split.
        anchor_index: usize,
        /// Position where the new child will be inserted.
        insert_index: usize,
        /// Share of the selected anchor child assigned to the inserted child.
        sizing: DockEdgeDockSizing,
        /// Scope the sizing applies to when inserting into an existing split.
        sizing_scope: DockEdgeDockSizingScope,
    },
    /// Wrap the target in a new split.
    WrapTarget {
        /// Target node being wrapped.
        target: DockNodeId,
        /// Axis of the new split.
        axis: SplitAxis,
        /// Position of the new child relative to the target.
        zone: DropZone,
        /// Share of the new split assigned to the inserted child.
        sizing: DockEdgeDockSizing,
    },
}

impl DockEdgeDockPlan {
    /// Returns the existing node this plan was built around.
    pub(crate) fn target_node(self) -> DockNodeId {
        match self {
            Self::InsertIntoSplit {
                split,
                zone: _,
                anchor_child: _,
                anchor_index: _,
                insert_index: _,
                sizing: _,
                sizing_scope: _,
            } => split,
            Self::WrapTarget { target, .. } => target,
        }
    }

    /// Returns the logical drop zone represented by this plan.
    pub(crate) fn drop_zone(self) -> DropZone {
        match self {
            Self::InsertIntoSplit { zone, .. } => zone,
            Self::WrapTarget { zone, .. } => zone,
        }
    }

    pub(crate) fn set_sizing(&mut self, next: DockEdgeDockSizing) {
        match self {
            Self::InsertIntoSplit { sizing, .. } | Self::WrapTarget { sizing, .. } => {
                *sizing = next;
            }
        }
    }
}

/// Initial sizing for an edge dock mutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockEdgeDockSizing {
    /// Fraction of the target extent assigned to the inserted child.
    new_child_share: f32,
}

/// Scope for applying an edge sizing plan to an existing same-axis split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockEdgeDockSizingScope {
    /// The new child takes a share of the whole split, preserving existing child ratios.
    WholeSplit,
    /// The new child takes a share of only the selected anchor child.
    AnchorChild,
}

impl DockEdgeDockSizing {
    const FALLBACK_SHARE: f32 = 0.5;

    /// Builds sizing from a desired inserted-child extent and its target extent.
    pub(crate) fn from_extents(new_child_extent: Pixels, target_extent: Pixels) -> Self {
        let target = f32::from(target_extent);
        if !target.is_finite() || target <= f32::EPSILON {
            return Self::fallback();
        }

        let desired = f32::from(new_child_extent);
        if !desired.is_finite() || desired <= f32::EPSILON {
            return Self::fallback();
        }

        Self::from_new_child_share((desired / target).clamp(0.0, 1.0))
    }

    pub(crate) fn fallback() -> Self {
        Self::from_new_child_share(Self::FALLBACK_SHARE)
    }

    fn from_new_child_share(new_child_share: f32) -> Self {
        let share = if new_child_share.is_finite() && new_child_share > 0.0 {
            new_child_share.clamp(f32::EPSILON, 1.0 - f32::EPSILON)
        } else {
            Self::FALLBACK_SHARE
        };
        Self {
            new_child_share: share,
        }
    }

    pub(crate) fn new_child_share(self) -> f32 {
        self.new_child_share
    }

    pub(crate) fn is_valid(self) -> bool {
        self.new_child_share.is_finite() && self.new_child_share > 0.0 && self.new_child_share < 1.0
    }
}

/// In-window floating container metadata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockFloatingContainer {
    /// Floating node id.
    pub node: DockNodeId,
    /// Container bounds relative to the dock host.
    pub bounds: Bounds<Pixels>,
}

/// Dock-space level central region semantics.
///
/// The central region is not a special [`DockNode`]. It is durable metadata owned by the dock
/// space so an empty central area can stay represented without weakening the graph invariant that
/// ordinary tabs nodes are non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockCentralRegion {
    /// Reachable graph node currently occupying the central region, when it has docked content.
    pub node: Option<DockNodeId>,
    /// Whether the central region remains semantically present when it has no node.
    pub keep_alive_when_empty: bool,
    /// Whether an empty central region should allow underlying application input to pass through.
    pub passthrough_when_empty: bool,
}

impl DockCentralRegion {
    /// Creates an empty central region with ImGui-like keep-alive semantics.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a central region backed by a reachable graph node.
    pub fn with_node(node: DockNodeId) -> Self {
        Self {
            node: Some(node),
            ..Self::default()
        }
    }

    /// Sets whether an empty central region allows underlying application input to pass through.
    pub fn with_passthrough_when_empty(mut self, passthrough: bool) -> Self {
        self.passthrough_when_empty = passthrough;
        self
    }
}

impl Default for DockCentralRegion {
    fn default() -> Self {
        Self {
            node: None,
            keep_alive_when_empty: true,
            passthrough_when_empty: false,
        }
    }
}

/// Retained docking graph for one or more logical dock spaces.
#[derive(Debug, Clone, Default)]
pub struct DockGraph {
    nodes: SlotMap<DockNodeId, DockNode>,
    roots: HashMap<DockSpaceId, DockNodeId>,
    floatings: HashMap<DockSpaceId, Vec<DockFloatingContainer>>,
    central_regions: HashMap<DockSpaceId, DockCentralRegion>,
    tab_selection_history: HashMap<DockNodeId, DockTabSelectionState>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(in crate::graph) struct DockTabSelectionState {
    next_stamp: u64,
    selected_stamps_by_item: HashMap<DockItemId, u64>,
}

impl DockGraph {
    pub(crate) fn matches_exactly(&self, other: &Self) -> bool {
        self.nodes.len() == other.nodes.len()
            && self
                .nodes
                .iter()
                .all(|(key, node)| other.nodes.get(key) == Some(node))
            && self.roots == other.roots
            && self.floatings == other.floatings
            && self.central_regions == other.central_regions
            && self.tab_selection_history == other.tab_selection_history
    }

    /// Creates an empty dock graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a node and returns its runtime id.
    pub fn insert_node(&mut self, node: DockNode) -> DockNodeId {
        self.nodes.insert(node)
    }

    pub(in crate::graph) fn record_tab_selection(&mut self, tabs: DockNodeId, item: &DockItemId) {
        let state = self.tab_selection_history.entry(tabs).or_default();
        let stamp = state.next_stamp;
        state.next_stamp = state.next_stamp.saturating_add(1);
        state.selected_stamps_by_item.insert(item.clone(), stamp);
    }

    pub(in crate::graph) fn preferred_tab_after_close(
        &self,
        tabs: DockNodeId,
        closing_item: &DockItemId,
        items: &[DockItemId],
    ) -> Option<DockItemId> {
        let state = self.tab_selection_history.get(&tabs)?;
        state
            .selected_stamps_by_item
            .iter()
            .filter(|(candidate, _)| *candidate != closing_item && items.contains(candidate))
            .max_by_key(|(_, stamp)| *stamp)
            .map(|(item, _)| item.clone())
    }

    pub(in crate::graph) fn take_tab_selection_state(
        &mut self,
        tabs: DockNodeId,
    ) -> DockTabSelectionState {
        self.tab_selection_history.remove(&tabs).unwrap_or_default()
    }

    pub(in crate::graph) fn restore_tab_selection_state(
        &mut self,
        tabs: DockNodeId,
        state: DockTabSelectionState,
    ) {
        if state.selected_stamps_by_item.is_empty() {
            return;
        }
        self.tab_selection_history.insert(tabs, state);
    }

    /// Returns a node by id.
    pub fn node(&self, id: DockNodeId) -> Option<&DockNode> {
        self.nodes.get(id)
    }

    /// Sets the root node for a dock space.
    pub fn set_root(&mut self, space: DockSpaceId, root: DockNodeId) {
        self.roots.insert(space, root);
    }

    pub(in crate::graph) fn set_root_for_empty_space(
        &mut self,
        space: &DockSpaceId,
        root: DockNodeId,
    ) {
        self.set_root(space.clone(), root);
        if let Some(central) = self.central_regions.get_mut(space)
            && central.keep_alive_when_empty
            && central.node.is_none()
        {
            central.node = Some(root);
        }
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
            .chain(self.central_regions.keys())
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

    /// Returns central region semantics for a dock space.
    pub fn central_region(&self, space: &DockSpaceId) -> Option<&DockCentralRegion> {
        self.central_regions.get(space)
    }

    /// Sets central region semantics for a dock space.
    pub fn set_central_region(
        &mut self,
        space: impl Into<DockSpaceId>,
        central: DockCentralRegion,
    ) {
        self.central_regions.insert(space.into(), central);
    }

    /// Removes central region semantics for a dock space.
    pub fn remove_central_region(&mut self, space: &DockSpaceId) -> Option<DockCentralRegion> {
        self.central_regions.remove(space)
    }
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}
