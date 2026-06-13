use crate::{DockItemId, DockSpaceId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

use super::{DOCK_LAYOUT_VERSION, DockLayout, DockLayoutNode, DockLayoutSpace};

impl DockLayout {
    /// Validates graph-level layout invariants before import.
    pub fn validate(&self) -> Result<(), DockLayoutValidationError> {
        if self.layout_version != DOCK_LAYOUT_VERSION {
            return Err(DockLayoutValidationError::UnsupportedVersion {
                expected: DOCK_LAYOUT_VERSION,
                found: self.layout_version,
            });
        }

        let mut space_ids = HashSet::new();
        for space in &self.spaces {
            if !space_ids.insert(space.id.clone()) {
                return Err(DockLayoutValidationError::DuplicateSpaceId {
                    space: space.id.clone(),
                });
            }
        }

        let mut by_id = HashMap::new();
        for node in &self.nodes {
            let id = node.id();
            if by_id.insert(id, node).is_some() {
                return Err(DockLayoutValidationError::DuplicateNodeId { id });
            }
        }

        let mut item_nodes = HashMap::new();
        for node in &self.nodes {
            if let DockLayoutNode::Tabs { id, items, .. } = node {
                for item in items {
                    if let Some(first_node) = item_nodes.insert(item.clone(), *id) {
                        return Err(DockLayoutValidationError::DuplicateItemId {
                            item: item.clone(),
                            first_node,
                            duplicate_node: *id,
                        });
                    }
                }
            }
        }

        for (id, node) in &by_id {
            match node {
                DockLayoutNode::Tabs {
                    items,
                    selected,
                    active,
                    ..
                } => {
                    if items.is_empty() {
                        return Err(DockLayoutValidationError::EmptyTabs { id: *id });
                    }
                    if let Some(selected) = selected {
                        if !items.contains(selected) {
                            return Err(DockLayoutValidationError::TabsSelectedItemMissing {
                                id: *id,
                                selected: selected.clone(),
                            });
                        }
                    } else if *active >= items.len() {
                        return Err(DockLayoutValidationError::TabsActiveOutOfBounds {
                            id: *id,
                            active: *active,
                            len: items.len(),
                        });
                    }
                }
                DockLayoutNode::Split {
                    children,
                    fractions,
                    ..
                } => {
                    if children.is_empty() {
                        return Err(DockLayoutValidationError::EmptySplitChildren { id: *id });
                    }
                    if children.len() != fractions.len() {
                        return Err(DockLayoutValidationError::SplitFractionsLenMismatch {
                            id: *id,
                            children_len: children.len(),
                            fractions_len: fractions.len(),
                        });
                    }
                    for (index, value) in fractions.iter().copied().enumerate() {
                        if !value.is_finite() {
                            return Err(DockLayoutValidationError::SplitNonFiniteFraction {
                                id: *id,
                                index,
                                value,
                            });
                        }
                        if value < 0.0 {
                            return Err(DockLayoutValidationError::SplitNegativeFraction {
                                id: *id,
                                index,
                                value,
                            });
                        }
                    }
                }
            }
        }

        for node in by_id.values() {
            if let DockLayoutNode::Split { children, .. } = node {
                for child in children {
                    if !by_id.contains_key(child) {
                        return Err(DockLayoutValidationError::MissingNodeId { id: *child });
                    }
                }
            }
        }

        detect_cycles(&by_id)?;

        for space in &self.spaces {
            if let Some(root) = space.root
                && !by_id.contains_key(&root)
            {
                return Err(DockLayoutValidationError::SpaceRootMissing {
                    space: space.id.clone(),
                    root,
                });
            }
            for floating in &space.floatings {
                if !by_id.contains_key(&floating.root) {
                    return Err(DockLayoutValidationError::FloatingRootMissing {
                        space: space.id.clone(),
                        root: floating.root,
                    });
                }
                if !floating.bounds.is_finite_with_non_negative_size() {
                    return Err(DockLayoutValidationError::InvalidFloatingBounds {
                        space: space.id.clone(),
                        root: floating.root,
                    });
                }
            }
            if let Some(central) = &space.central
                && let Some(node) = central.node
            {
                if !by_id.contains_key(&node) {
                    return Err(DockLayoutValidationError::CentralNodeMissing {
                        space: space.id.clone(),
                        node,
                    });
                }
                if !space
                    .root
                    .is_some_and(|root| subtree_contains(root, node, &by_id))
                {
                    return Err(DockLayoutValidationError::CentralNodeNotInRoot {
                        space: space.id.clone(),
                        node,
                    });
                }
            }
        }

        validate_forest_ownership(&by_id, &self.spaces)?;

        Ok(())
    }
}

/// Validation error for serialized dock layouts.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DockLayoutValidationError {
    /// The layout version is unsupported.
    #[error("unsupported dock layout version: expected {expected}, found {found}")]
    UnsupportedVersion {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// A dock space id appears more than once.
    #[error("duplicate dock layout space id: {space}")]
    DuplicateSpaceId {
        /// Duplicate dock space id.
        space: DockSpaceId,
    },
    /// A layout node id appears more than once.
    #[error("duplicate dock layout node id: {id}")]
    DuplicateNodeId {
        /// Duplicate id.
        id: u32,
    },
    /// A dock item id appears in more than one serialized tab position.
    #[error(
        "duplicate dock layout item id {item}: first seen in node {first_node}, duplicated in node {duplicate_node}"
    )]
    DuplicateItemId {
        /// Duplicate dock item id.
        item: DockItemId,
        /// First tabs node containing the item.
        first_node: u32,
        /// Tabs node containing the duplicate item.
        duplicate_node: u32,
    },
    /// A split references a missing node id.
    #[error("missing dock layout node id: {id}")]
    MissingNodeId {
        /// Missing id.
        id: u32,
    },
    /// The layout graph contains a cycle.
    #[error("dock layout cycle detected at node {id}")]
    CycleDetected {
        /// Cyclic id.
        id: u32,
    },
    /// A tabs node has no items.
    #[error("tabs node {id} is empty")]
    EmptyTabs {
        /// Invalid node id.
        id: u32,
    },
    /// A tabs node has an invalid active index.
    #[error("tabs node {id} active index {active} out of bounds for length {len}")]
    TabsActiveOutOfBounds {
        /// Invalid node id.
        id: u32,
        /// Active index.
        active: usize,
        /// Item count.
        len: usize,
    },
    /// A tabs node selected an item that is not in the tab order.
    #[error("tabs node {id} selected item {selected} is not present")]
    TabsSelectedItemMissing {
        /// Invalid node id.
        id: u32,
        /// Missing selected item.
        selected: DockItemId,
    },
    /// A split node has no children.
    #[error("split node {id} has no children")]
    EmptySplitChildren {
        /// Invalid node id.
        id: u32,
    },
    /// A split node has mismatched child and fraction counts.
    #[error("split node {id} has {children_len} children and {fractions_len} fractions")]
    SplitFractionsLenMismatch {
        /// Invalid node id.
        id: u32,
        /// Child count.
        children_len: usize,
        /// Fraction count.
        fractions_len: usize,
    },
    /// A split fraction is not finite.
    #[error("split node {id} fraction {index} is non-finite: {value}")]
    SplitNonFiniteFraction {
        /// Invalid node id.
        id: u32,
        /// Fraction index.
        index: usize,
        /// Invalid value.
        value: f32,
    },
    /// A split fraction is negative.
    #[error("split node {id} fraction {index} is negative: {value}")]
    SplitNegativeFraction {
        /// Invalid node id.
        id: u32,
        /// Fraction index.
        index: usize,
        /// Invalid value.
        value: f32,
    },
    /// A dock space references a missing root node.
    #[error("dock space {space} references missing root node {root}")]
    SpaceRootMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing root id.
        root: u32,
    },
    /// A floating container references a missing root node.
    #[error("dock space {space} references missing floating root node {root}")]
    FloatingRootMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing root id.
        root: u32,
    },
    /// A floating container has non-finite coordinates or negative size.
    #[error("dock space {space} floating root node {root} has invalid bounds")]
    InvalidFloatingBounds {
        /// Dock space id.
        space: DockSpaceId,
        /// Floating root id.
        root: u32,
    },
    /// A layout node is reachable from more than one parent/root.
    #[error("dock layout node {id} is referenced more than once")]
    DuplicateNodeReference {
        /// Shared node id.
        id: u32,
    },
    /// A layout node is not reachable from any dock space root or floating root.
    #[error("dock layout node {id} is not reachable from any dock space")]
    UnreachableNodeId {
        /// Unreachable node id.
        id: u32,
    },
    /// A central region references a missing node.
    #[error("dock space {space} references missing central node {node}")]
    CentralNodeMissing {
        /// Dock space id.
        space: DockSpaceId,
        /// Missing central node id.
        node: u32,
    },
    /// A central region references a node outside its dock space root subtree.
    #[error("dock space {space} central node {node} is not inside its root subtree")]
    CentralNodeNotInRoot {
        /// Dock space id.
        space: DockSpaceId,
        /// Central node id.
        node: u32,
    },
}

fn detect_cycles(by_id: &HashMap<u32, &DockLayoutNode>) -> Result<(), DockLayoutValidationError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }

    #[derive(Clone, Copy)]
    enum Step {
        Enter(u32),
        Exit(u32),
    }

    let mut marks = HashMap::new();
    for start in by_id.keys().copied() {
        if marks.contains_key(&start) {
            continue;
        }

        let mut stack = vec![Step::Enter(start)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(id) => {
                    if marks.get(&id) == Some(&Mark::Done) {
                        continue;
                    }
                    if marks.get(&id) == Some(&Mark::Visiting) {
                        return Err(DockLayoutValidationError::CycleDetected { id });
                    }

                    marks.insert(id, Mark::Visiting);
                    stack.push(Step::Exit(id));

                    if let Some(DockLayoutNode::Split { children, .. }) = by_id.get(&id) {
                        for child in children.iter().rev().copied() {
                            stack.push(Step::Enter(child));
                        }
                    }
                }
                Step::Exit(id) => {
                    marks.insert(id, Mark::Done);
                }
            }
        }
    }

    Ok(())
}

fn validate_forest_ownership(
    by_id: &HashMap<u32, &DockLayoutNode>,
    spaces: &[DockLayoutSpace],
) -> Result<(), DockLayoutValidationError> {
    let mut seen = HashSet::new();
    for root in spaces.iter().filter_map(|space| space.root).chain(
        spaces
            .iter()
            .flat_map(|space| space.floatings.iter().map(|floating| floating.root)),
    ) {
        mark_reachable_once(root, by_id, &mut seen)?;
    }

    for id in by_id.keys().copied() {
        if !seen.contains(&id) {
            return Err(DockLayoutValidationError::UnreachableNodeId { id });
        }
    }

    Ok(())
}

fn mark_reachable_once(
    root: u32,
    by_id: &HashMap<u32, &DockLayoutNode>,
    seen: &mut HashSet<u32>,
) -> Result<(), DockLayoutValidationError> {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            return Err(DockLayoutValidationError::DuplicateNodeReference { id });
        }

        if let Some(DockLayoutNode::Split { children, .. }) = by_id.get(&id) {
            stack.extend(children.iter().rev().copied());
        }
    }

    Ok(())
}

fn subtree_contains(root: u32, target: u32, by_id: &HashMap<u32, &DockLayoutNode>) -> bool {
    if root == target {
        return true;
    }
    match by_id.get(&root) {
        Some(DockLayoutNode::Split { children, .. }) => children
            .iter()
            .copied()
            .any(|child| subtree_contains(child, target, by_id)),
        Some(DockLayoutNode::Tabs { .. }) | None => false,
    }
}
