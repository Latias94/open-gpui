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
#[path = "graph_node_validation.rs"]
mod graph_node_validation;
#[path = "graph_op_validation.rs"]
mod graph_op_validation;
#[path = "graph_ops.rs"]
mod graph_ops;
#[path = "graph_query.rs"]
mod graph_query;
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
