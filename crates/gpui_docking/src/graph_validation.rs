use crate::{DockItemId, DockNode, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels};
use std::collections::HashMap;
use thiserror::Error;

use super::DockGraph;

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
    /// A tabs node has no selected item.
    #[error("tabs node {tabs:?} has no selected item")]
    TabsSelectedMissing {
        /// Tabs node id.
        tabs: DockNodeId,
    },
    /// A tabs node selected an item that is not in the tab order.
    #[error("tabs node {tabs:?} selected item {selected} is not present")]
    TabsSelectedItemMissing {
        /// Tabs node id.
        tabs: DockNodeId,
        /// Missing selected item.
        selected: DockItemId,
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
    /// A central region references a node outside its dock space root subtree.
    #[error("dock space {space} central node {node:?} is not inside its root subtree")]
    CentralNodeNotInRoot {
        /// Dock space id.
        space: DockSpaceId,
        /// Central node id.
        node: DockNodeId,
    },
    /// A runtime node is not reachable from any dock-space root or floating root.
    #[error("dock graph contains unreachable node {node:?}")]
    UnreachableNode {
        /// Unreachable runtime node id.
        node: DockNodeId,
    },
}

impl DockGraph {
    /// Validates graph state reachable from dock-space and floating roots.
    ///
    /// Unattached nodes are allowed because [`DockGraph::insert_node`] and
    /// [`DockGraph::set_root`] form a staged public construction API. Use
    /// [`Self::validate_canonical`] at a complete graph commit boundary.
    pub fn validate(&self) -> Result<(), DockGraphValidationError> {
        self.validate_with_unreachable_nodes(true)
    }

    /// Validates a fully assembled graph and rejects every unattached runtime node.
    pub fn validate_canonical(&self) -> Result<(), DockGraphValidationError> {
        self.validate_with_unreachable_nodes(false)
    }

    fn validate_with_unreachable_nodes(
        &self,
        allow_unreachable_nodes: bool,
    ) -> Result<(), DockGraphValidationError> {
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

        for (space, central) in &self.central_regions {
            let Some(node) = central.node else {
                continue;
            };
            if !self.root_subtree_contains(space, node) {
                return Err(DockGraphValidationError::CentralNodeNotInRoot {
                    space: space.clone(),
                    node,
                });
            }
        }

        if !allow_unreachable_nodes
            && let Some(node) = self
                .nodes
                .keys()
                .find(|node| !validator.marks.contains_key(node))
        {
            return Err(DockGraphValidationError::UnreachableNode { node });
        }

        Ok(())
    }
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
            DockNode::Tabs { items, selected } => {
                self.validate_tabs(node, items, selected)?;
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
        selected: &Option<DockItemId>,
    ) -> Result<(), DockGraphValidationError> {
        if items.is_empty() {
            return Err(DockGraphValidationError::EmptyTabs { tabs });
        }
        let Some(selected) = selected else {
            return Err(DockGraphValidationError::TabsSelectedMissing { tabs });
        };
        if !items.contains(selected) {
            return Err(DockGraphValidationError::TabsSelectedItemMissing {
                tabs,
                selected: selected.clone(),
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

fn bounds_is_finite_with_non_negative_size(bounds: Bounds<Pixels>) -> bool {
    f32::from(bounds.origin.x).is_finite()
        && f32::from(bounds.origin.y).is_finite()
        && f32::from(bounds.size.width).is_finite()
        && f32::from(bounds.size.height).is_finite()
        && f32::from(bounds.size.width) >= 0.0
        && f32::from(bounds.size.height) >= 0.0
}
