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
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}
