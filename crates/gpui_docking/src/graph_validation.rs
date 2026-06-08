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

fn bounds_is_finite_with_non_negative_size(bounds: Bounds<Pixels>) -> bool {
    f32::from(bounds.origin.x).is_finite()
        && f32::from(bounds.origin.y).is_finite()
        && f32::from(bounds.size.width).is_finite()
        && f32::from(bounds.size.height).is_finite()
        && f32::from(bounds.size.width) >= 0.0
        && f32::from(bounds.size.height) >= 0.0
}
