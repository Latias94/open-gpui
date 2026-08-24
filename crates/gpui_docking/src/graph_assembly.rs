#![allow(dead_code)]

use crate::graph_index::{CanonicalGraphIndex, CanonicalNodeOwner};
use crate::{
    CanonicalGraphAuthority, CanonicalGraphVersion, CanonicalNodeId, DockFloatingId, DockGroupId,
    DockItemId, DockLayoutRect, DockSpaceId, SplitAxis,
};
use slotmap::SlotMap;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// A canonical tab group with durable content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalGroup {
    pub(crate) items: Vec<DockItemId>,
    pub(crate) selected: DockItemId,
    pub(crate) mru: Vec<DockItemId>,
}

/// Dock-space-owned central policy. An empty central space has no placeholder node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalCentralPolicy {
    pub(crate) node: Option<CanonicalNodeId>,
    pub(crate) keep_alive_when_empty: bool,
    pub(crate) passthrough_when_empty: bool,
}

/// Durable size reference used to rebuild binary split intent across viewport sizes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CanonicalSizeRef {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl CanonicalSizeRef {
    pub(crate) fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

impl Default for CanonicalCentralPolicy {
    fn default() -> Self {
        Self {
            node: None,
            keep_alive_when_empty: true,
            passthrough_when_empty: false,
        }
    }
}

/// Binary retained topology. A branch always has exactly two children.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CanonicalNode {
    Branch {
        axis: SplitAxis,
        first: CanonicalNodeId,
        second: CanonicalNodeId,
        ratio: f32,
        size_ref: Option<CanonicalSizeRef>,
    },
    Group {
        group: DockGroupId,
        size_ref: Option<CanonicalSizeRef>,
    },
}

/// A first-class floating root owned by one dock space.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CanonicalFloatingRoot {
    pub(crate) space: DockSpaceId,
    pub(crate) root: CanonicalNodeId,
    pub(crate) bounds: DockLayoutRect,
    pub(crate) z_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CanonicalSpace {
    pub(crate) root: Option<CanonicalNodeId>,
    pub(crate) central: CanonicalCentralPolicy,
    pub(crate) floating_roots: Vec<DockFloatingId>,
}

/// Canonical binary graph published from an assembly.
#[derive(Debug)]
pub(crate) struct CanonicalDockGraph {
    nodes: SlotMap<CanonicalNodeId, CanonicalNode>,
    groups: BTreeMap<DockGroupId, CanonicalGroup>,
    spaces: BTreeMap<DockSpaceId, CanonicalSpace>,
    floating_roots: BTreeMap<DockFloatingId, CanonicalFloatingRoot>,
    index: CanonicalGraphIndex,
    version: CanonicalGraphVersion,
}

impl PartialEq for CanonicalDockGraph {
    fn eq(&self, other: &Self) -> bool {
        self.nodes.len() == other.nodes.len()
            && self
                .nodes
                .iter()
                .all(|(id, node)| other.nodes.get(id) == Some(node))
            && self.groups == other.groups
            && self.spaces == other.spaces
            && self.floating_roots == other.floating_roots
            && self.index == other.index
            && self.version == other.version
    }
}

impl CanonicalDockGraph {
    pub(crate) fn version(&self) -> CanonicalGraphVersion {
        self.version
    }

    pub(crate) fn node(&self, id: CanonicalNodeId) -> Option<&CanonicalNode> {
        self.nodes.get(id)
    }

    pub(crate) fn group(&self, id: &DockGroupId) -> Option<&CanonicalGroup> {
        self.groups.get(id)
    }

    pub(crate) fn space(&self, id: &DockSpaceId) -> Option<&CanonicalSpace> {
        self.spaces.get(id)
    }

    pub(crate) fn index(&self) -> &CanonicalGraphIndex {
        &self.index
    }

    pub(crate) fn nodes(&self) -> &SlotMap<CanonicalNodeId, CanonicalNode> {
        &self.nodes
    }

    pub(crate) fn groups(&self) -> &BTreeMap<DockGroupId, CanonicalGroup> {
        &self.groups
    }

    pub(crate) fn spaces(&self) -> &BTreeMap<DockSpaceId, CanonicalSpace> {
        &self.spaces
    }

    pub(crate) fn floating_roots(&self) -> &BTreeMap<DockFloatingId, CanonicalFloatingRoot> {
        &self.floating_roots
    }

    /// Compares semantic graph state without publication metadata or derived indexes.
    pub(crate) fn state_eq(&self, other: &Self) -> bool {
        self.nodes.len() == other.nodes.len()
            && self
                .nodes
                .iter()
                .all(|(id, node)| other.nodes.get(id) == Some(node))
            && self.groups == other.groups
            && self.spaces == other.spaces
            && self.floating_roots == other.floating_roots
    }

    pub(crate) fn publication_eq(&self, other: &Self) -> bool {
        self.version == other.version
    }

    /// Applies a selection/MRU update after the transaction boundary has revalidated its read set.
    ///
    /// This is intentionally the only production mutation entry point for the canonical group
    /// state in the first migration slice. Callers must prove membership before entering it; the
    /// boolean is retained as an invariant check so future patch kinds can share the same seam.
    pub(crate) fn apply_selection_for_commit(
        &mut self,
        group: &DockGroupId,
        item: &DockItemId,
        version: CanonicalGraphVersion,
    ) -> bool {
        let Some(group_state) = self.groups.get_mut(group) else {
            return false;
        };
        if !group_state.items.contains(item) {
            return false;
        }
        group_state.mru.retain(|entry| entry != item);
        group_state.mru.insert(0, item.clone());
        group_state.selected = item.clone();
        self.version = version;
        true
    }

    /// Inserts a new group beside an existing group and rewires only its local parent/root path.
    pub(crate) fn apply_split_group_for_commit(
        &mut self,
        target_group: &DockGroupId,
        new_group: &DockGroupId,
        items: &[DockItemId],
        selected: &DockItemId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        space: &DockSpaceId,
        version: CanonicalGraphVersion,
    ) -> bool {
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return false;
        };
        let Some(owner) = self.index.owner_for_node(target_node).cloned() else {
            return false;
        };
        let owner_space = match &owner {
            CanonicalNodeOwner::Main(owner_space)
            | CanonicalNodeOwner::Floating {
                space: owner_space, ..
            } => owner_space,
        };
        if owner_space != space
            || self.groups.contains_key(new_group)
            || items.is_empty()
            || !items.contains(selected)
            || !ratio.is_finite()
            || !(0.0..=1.0).contains(&ratio)
            || ratio <= 0.0
            || ratio >= 1.0
        {
            return false;
        }
        if items
            .iter()
            .any(|item| self.index.group_for_item(item).is_some())
        {
            return false;
        }

        let parent = self.index.parent_of(target_node).flatten();
        if let Some(parent) = parent
            && !matches!(self.nodes.get(parent), Some(CanonicalNode::Branch { .. }))
        {
            return false;
        }

        self.groups.insert(
            new_group.clone(),
            CanonicalGroup {
                items: items.to_vec(),
                selected: selected.clone(),
                mru: Vec::new(),
            },
        );
        let new_leaf = self.nodes.insert(CanonicalNode::Group {
            group: new_group.clone(),
            size_ref: None,
        });
        let branch = if insert_before {
            CanonicalNode::Branch {
                axis,
                first: new_leaf,
                second: target_node,
                ratio,
                size_ref: None,
            }
        } else {
            CanonicalNode::Branch {
                axis,
                first: target_node,
                second: new_leaf,
                ratio,
                size_ref: None,
            }
        };
        let branch_id = self.nodes.insert(branch);

        if let Some(parent) = parent {
            let parent_node = self
                .nodes
                .get_mut(parent)
                .expect("validated parent must remain in the node map");
            match parent_node {
                CanonicalNode::Branch { first, second, .. } if *first == target_node => {
                    *first = branch_id;
                }
                CanonicalNode::Branch { first, second, .. } if *second == target_node => {
                    *second = branch_id;
                }
                _ => unreachable!("validated parent must reference the target node"),
            }
        } else {
            match owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    self.spaces
                        .get_mut(&owner_space)
                        .expect("validated main owner must remain present")
                        .root = Some(branch_id);
                }
                CanonicalNodeOwner::Floating { floating, .. } => {
                    self.floating_roots
                        .get_mut(&floating)
                        .expect("validated floating owner must remain present")
                        .root = branch_id;
                }
            }
        }

        self.index = CanonicalGraphIndex::build(self)
            .expect("a prepared canonical split must preserve graph invariants");
        self.version = version;
        true
    }

    pub(crate) fn apply_move_group_to_edge_for_commit(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        version: CanonicalGraphVersion,
    ) -> bool {
        if source_group == target_group
            || !ratio.is_finite()
            || !(0.0..=1.0).contains(&ratio)
            || ratio <= 0.0
            || ratio >= 1.0
        {
            return false;
        }
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return false;
        };
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return false;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return false;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return false;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space || target_owner_space != target_space {
            return false;
        }
        let source_parent = self.index.parent_of(source_node).flatten();
        if let Some(parent) = source_parent {
            let sibling = match self.nodes.get(parent) {
                Some(CanonicalNode::Branch { first, second, .. }) if *first == source_node => {
                    *second
                }
                Some(CanonicalNode::Branch { first, second, .. }) if *second == source_node => {
                    *first
                }
                _ => return false,
            };
            let grandparent = self.index.parent_of(parent).flatten();
            if let Some(grandparent) = grandparent {
                let parent_node = self
                    .nodes
                    .get_mut(grandparent)
                    .expect("validated source grandparent must remain present");
                match parent_node {
                    CanonicalNode::Branch { first, second, .. } if *first == parent => {
                        *first = sibling;
                    }
                    CanonicalNode::Branch { first, second, .. } if *second == parent => {
                        *second = sibling;
                    }
                    _ => return false,
                }
            } else {
                match &source_owner {
                    CanonicalNodeOwner::Main(owner_space) => {
                        self.spaces
                            .get_mut(owner_space)
                            .expect("validated source space must remain present")
                            .root = Some(sibling);
                    }
                    CanonicalNodeOwner::Floating { floating, .. } => {
                        self.floating_roots
                            .get_mut(floating)
                            .expect("validated source floating root must remain present")
                            .root = sibling;
                    }
                }
            }
            self.nodes.remove(parent);
            if let CanonicalNodeOwner::Main(owner_space) = &source_owner
                && self
                    .spaces
                    .get(owner_space)
                    .and_then(|space| space.central.node)
                    == Some(source_node)
            {
                self.spaces
                    .get_mut(owner_space)
                    .expect("validated source space must remain present")
                    .central
                    .node = Some(sibling);
            }
        } else {
            match &source_owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    let space = self
                        .spaces
                        .get_mut(owner_space)
                        .expect("validated source space must remain present");
                    if space.root != Some(source_node) {
                        return false;
                    }
                    space.root = None;
                    if space.central.node == Some(source_node) {
                        space.central.node = None;
                    }
                }
                CanonicalNodeOwner::Floating { space, floating } => {
                    let floating_root = self
                        .floating_roots
                        .get(floating)
                        .expect("validated source floating root must remain present");
                    if floating_root.root != source_node {
                        return false;
                    }
                    self.floating_roots.remove(floating);
                    self.spaces
                        .get_mut(space)
                        .expect("validated source space must remain present")
                        .floating_roots
                        .retain(|candidate| candidate != floating);
                }
            }
        }

        let target_parent = match &target_owner {
            CanonicalNodeOwner::Main(owner_space) => {
                let root = self
                    .spaces
                    .get(owner_space)
                    .and_then(|space| space.root)
                    .expect("validated target main space must retain a root");
                Self::find_reachable_parent(self, root, target_node, None)
                    .expect("validated target group must remain reachable")
            }
            CanonicalNodeOwner::Floating { floating, .. } => {
                let root = self
                    .floating_roots
                    .get(floating)
                    .map(|floating| floating.root)
                    .expect("validated target floating root must remain present");
                Self::find_reachable_parent(self, root, target_node, None)
                    .expect("validated target group must remain reachable")
            }
        };
        let branch_id = self.nodes.insert(if insert_before {
            CanonicalNode::Branch {
                axis,
                first: source_node,
                second: target_node,
                ratio,
                size_ref: None,
            }
        } else {
            CanonicalNode::Branch {
                axis,
                first: target_node,
                second: source_node,
                ratio,
                size_ref: None,
            }
        });
        if let Some(parent) = target_parent {
            let parent_node = self
                .nodes
                .get_mut(parent)
                .expect("validated target parent must remain present");
            match parent_node {
                CanonicalNode::Branch { first, second, .. } if *first == target_node => {
                    *first = branch_id;
                }
                CanonicalNode::Branch { first, second, .. } if *second == target_node => {
                    *second = branch_id;
                }
                _ => return false,
            }
        } else {
            match target_owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    self.spaces
                        .get_mut(&owner_space)
                        .expect("validated target space must remain present")
                        .root = Some(branch_id);
                }
                CanonicalNodeOwner::Floating { floating, .. } => {
                    self.floating_roots
                        .get_mut(&floating)
                        .expect("validated target floating root must remain present")
                        .root = branch_id;
                }
            }
        }
        self.index = CanonicalGraphIndex::build(self)
            .expect("a prepared group move must preserve graph invariants");
        self.version = version;
        true
    }

    fn find_reachable_parent(
        graph: &Self,
        current: CanonicalNodeId,
        target: CanonicalNodeId,
        parent: Option<CanonicalNodeId>,
    ) -> Option<Option<CanonicalNodeId>> {
        if current == target {
            return Some(parent);
        }
        let CanonicalNode::Branch { first, second, .. } = graph.nodes.get(current)? else {
            return None;
        };
        Self::find_reachable_parent(graph, *first, target, Some(current))
            .or_else(|| Self::find_reachable_parent(graph, *second, target, Some(current)))
    }

    #[cfg(test)]
    pub(crate) fn group_mut_for_test(&mut self, id: &DockGroupId) -> Option<&mut CanonicalGroup> {
        self.groups.get_mut(id)
    }

    pub(crate) fn validate(&self) -> Result<(), CanonicalGraphError> {
        let rebuilt = CanonicalGraphIndex::build(self)?;
        if rebuilt != self.index {
            return Err(CanonicalGraphError::IndexMismatch);
        }

        for (space_id, space) in &self.spaces {
            let mut z_orders = BTreeSet::new();
            for floating_id in &space.floating_roots {
                let floating = self.floating_roots.get(floating_id).ok_or_else(|| {
                    CanonicalGraphError::FloatingRootMissing {
                        floating: floating_id.clone(),
                    }
                })?;
                if floating.space != *space_id {
                    return Err(CanonicalGraphError::FloatingSpaceMissing {
                        floating: floating_id.clone(),
                        space: floating.space.clone(),
                    });
                }
                if !floating.bounds.is_finite_with_non_negative_size()
                    || floating.bounds.width <= 0.0
                    || floating.bounds.height <= 0.0
                {
                    return Err(CanonicalGraphError::InvalidFloatingBounds {
                        floating: floating_id.clone(),
                    });
                }
                if !z_orders.insert(floating.z_order) {
                    return Err(CanonicalGraphError::DuplicateFloatingZOrder {
                        space: space_id.clone(),
                        z_order: floating.z_order,
                    });
                }
            }
            if space.floating_roots.windows(2).any(|window| {
                let left = self
                    .floating_roots
                    .get(&window[0])
                    .expect("floating root validated above");
                let right = self
                    .floating_roots
                    .get(&window[1])
                    .expect("floating root validated above");
                (left.z_order, &window[0]) > (right.z_order, &window[1])
            }) {
                return Err(CanonicalGraphError::NonCanonicalFloatingOrder {
                    space: space_id.clone(),
                });
            }
            if let Some(node) = space.central.node {
                if !matches!(
                    self.index.owner_for_node(node),
                    Some(CanonicalNodeOwner::Main(owner)) if owner == space_id
                ) {
                    return Err(CanonicalGraphError::CentralNodeOutsideSpace {
                        space: space_id.clone(),
                        node,
                    });
                }
                if !matches!(self.nodes.get(node), Some(CanonicalNode::Group { .. })) {
                    return Err(CanonicalGraphError::CentralNodeNotGroup {
                        space: space_id.clone(),
                        node,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Errors raised before a canonical graph is published.
#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum CanonicalGraphError {
    #[error("dock space {space} is declared more than once")]
    DuplicateSpace { space: DockSpaceId },
    #[error("dock space id cannot be empty")]
    EmptySpaceId,
    #[error("group {group} is declared more than once")]
    DuplicateGroup { group: DockGroupId },
    #[error("group id cannot be empty")]
    EmptyGroupId,
    #[error("floating root {floating} is declared more than once")]
    DuplicateFloating { floating: DockFloatingId },
    #[error("floating root id cannot be empty")]
    EmptyFloatingId,
    #[error("group {group} has no items")]
    EmptyGroup { group: DockGroupId },
    #[error("group {group} selects an item that is not in the group")]
    SelectedItemMissing { group: DockGroupId },
    #[error("group {group} MRU contains item {item} that is not in the group")]
    MruItemMissing {
        group: DockGroupId,
        item: DockItemId,
    },
    #[error("group {group} MRU contains duplicate item {item}")]
    DuplicateMruItem {
        group: DockGroupId,
        item: DockItemId,
    },
    #[error("item {item} belongs to more than one group")]
    DuplicateItem { item: DockItemId },
    #[error("branch ratio must be finite and between zero and one inclusive, got {ratio}")]
    InvalidRatio { ratio: f32 },
    #[error("node size reference must be finite and non-negative")]
    InvalidSizeRef,
    #[error("assembly node {node:?} does not exist")]
    MissingAssemblyNode { node: AssemblyNodeId },
    #[error("canonical node {node:?} does not exist")]
    MissingNode { node: CanonicalNodeId },
    #[error("canonical node {node:?} participates in a cycle")]
    CanonicalCycle { node: CanonicalNodeId },
    #[error("canonical node {node:?} is owned by more than one root")]
    SharedCanonicalNode { node: CanonicalNodeId },
    #[error("assembly node {node:?} participates in a cycle")]
    AssemblyCycle { node: AssemblyNodeId },
    #[error("assembly node {node:?} is attached more than once")]
    SharedAssemblyNode { node: AssemblyNodeId },
    #[error("assembly node {node:?} is not attached to a space or floating root")]
    UnattachedAssemblyNode { node: AssemblyNodeId },
    #[error("group {group} is referenced by more than one structural node")]
    SharedGroup { group: DockGroupId },
    #[error("structural node references missing group {group}")]
    MissingGroup { group: DockGroupId },
    #[error("space {space} does not exist")]
    MissingSpace { space: DockSpaceId },
    #[error("floating root {floating} belongs to missing space {space}")]
    FloatingSpaceMissing {
        floating: DockFloatingId,
        space: DockSpaceId,
    },
    #[error("floating root {floating} has invalid bounds")]
    InvalidFloatingBounds { floating: DockFloatingId },
    #[error("dock space {space} has duplicate floating z-order {z_order}")]
    DuplicateFloatingZOrder { space: DockSpaceId, z_order: i32 },
    #[error("dock space {space} floating roots are not in canonical z-order")]
    NonCanonicalFloatingOrder { space: DockSpaceId },
    #[error("floating root {floating} references a node already owned by another root")]
    SharedFloatingRoot { floating: DockFloatingId },
    #[error("floating root {floating} references a missing node")]
    FloatingRootMissing { floating: DockFloatingId },
    #[error("floating root {floating} is not owned by any dock space")]
    UnreferencedFloating { floating: DockFloatingId },
    #[error("group {group} is not attached to a published root")]
    UnreferencedGroup { group: DockGroupId },
    #[error("central node {node:?} is not reachable from space {space}")]
    CentralNodeOutsideSpace {
        space: DockSpaceId,
        node: CanonicalNodeId,
    },
    #[error("central node {node:?} in space {space} must be a tab group leaf")]
    CentralNodeNotGroup {
        space: DockSpaceId,
        node: CanonicalNodeId,
    },
    #[error("canonical graph reverse indexes do not match the published topology")]
    IndexMismatch,
}

#[derive(Debug, Clone, PartialEq)]
enum AssemblyNode {
    Group {
        group: DockGroupId,
        size_ref: Option<CanonicalSizeRef>,
    },
    Branch {
        axis: SplitAxis,
        first: AssemblyNodeId,
        second: AssemblyNodeId,
        ratio: f32,
        size_ref: Option<CanonicalSizeRef>,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct AssemblySpace {
    root: Option<AssemblyNodeId>,
    central: AssemblyCentralPolicy,
    floating_roots: Vec<DockFloatingId>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct AssemblyCentralPolicy {
    node: Option<AssemblyNodeId>,
    keep_alive_when_empty: bool,
    passthrough_when_empty: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct AssemblyFloatingRoot {
    space: DockSpaceId,
    root: AssemblyNodeId,
    bounds: DockLayoutRect,
    z_order: i32,
}

/// Unvalidated construction state. It cannot be installed as a live graph directly.
#[derive(Debug, Clone, Default)]
pub(crate) struct DockGraphAssembly {
    nodes: Vec<AssemblyNode>,
    groups: BTreeMap<DockGroupId, CanonicalGroup>,
    spaces: BTreeMap<DockSpaceId, AssemblySpace>,
    floating_roots: BTreeMap<DockFloatingId, AssemblyFloatingRoot>,
}

/// Opaque assembly node reference used while constructing a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AssemblyNodeId(usize);

impl DockGraphAssembly {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_space(
        &mut self,
        space: impl Into<DockSpaceId>,
    ) -> Result<(), CanonicalGraphError> {
        let space = space.into();
        if space.as_str().is_empty() {
            return Err(CanonicalGraphError::EmptySpaceId);
        }
        if self.spaces.contains_key(&space) {
            return Err(CanonicalGraphError::DuplicateSpace { space });
        }
        self.spaces.insert(
            space.clone(),
            AssemblySpace {
                root: None,
                central: AssemblyCentralPolicy {
                    keep_alive_when_empty: true,
                    ..AssemblyCentralPolicy::default()
                },
                floating_roots: Vec::new(),
            },
        );
        Ok(())
    }

    pub(crate) fn add_group(
        &mut self,
        id: impl Into<DockGroupId>,
        items: impl IntoIterator<Item = DockItemId>,
        selected: DockItemId,
    ) -> Result<AssemblyNodeId, CanonicalGraphError> {
        self.add_group_with_size_ref(id, items, selected, None)
    }

    pub(crate) fn add_group_with_size_ref(
        &mut self,
        id: impl Into<DockGroupId>,
        items: impl IntoIterator<Item = DockItemId>,
        selected: DockItemId,
        size_ref: Option<CanonicalSizeRef>,
    ) -> Result<AssemblyNodeId, CanonicalGraphError> {
        let id = id.into();
        if id.as_str().is_empty() {
            return Err(CanonicalGraphError::EmptyGroupId);
        }
        if self.groups.contains_key(&id) {
            return Err(CanonicalGraphError::DuplicateGroup { group: id });
        }
        let items = items.into_iter().collect::<Vec<_>>();
        if items.is_empty() {
            return Err(CanonicalGraphError::EmptyGroup { group: id });
        }
        if !items.contains(&selected) {
            return Err(CanonicalGraphError::SelectedItemMissing { group: id });
        }
        validate_size_ref(size_ref)?;
        self.groups.insert(
            id.clone(),
            CanonicalGroup {
                items,
                selected,
                mru: Vec::new(),
            },
        );
        Ok(self.push_node(AssemblyNode::Group {
            group: id,
            size_ref,
        }))
    }

    pub(crate) fn add_branch(
        &mut self,
        axis: SplitAxis,
        first: AssemblyNodeId,
        second: AssemblyNodeId,
        ratio: f32,
    ) -> Result<AssemblyNodeId, CanonicalGraphError> {
        self.add_branch_with_size_ref(axis, first, second, ratio, None)
    }

    pub(crate) fn add_branch_with_size_ref(
        &mut self,
        axis: SplitAxis,
        first: AssemblyNodeId,
        second: AssemblyNodeId,
        ratio: f32,
        size_ref: Option<CanonicalSizeRef>,
    ) -> Result<AssemblyNodeId, CanonicalGraphError> {
        validate_ratio(ratio)?;
        validate_size_ref(size_ref)?;
        if !self.node_exists(first) {
            return Err(CanonicalGraphError::MissingAssemblyNode { node: first });
        }
        if !self.node_exists(second) {
            return Err(CanonicalGraphError::MissingAssemblyNode { node: second });
        }
        Ok(self.push_node(AssemblyNode::Branch {
            axis,
            first,
            second,
            ratio,
            size_ref,
        }))
    }

    pub(crate) fn set_root(
        &mut self,
        space: impl Into<DockSpaceId>,
        root: AssemblyNodeId,
    ) -> Result<(), CanonicalGraphError> {
        let space = space.into();
        if !self.spaces.contains_key(&space) {
            return Err(CanonicalGraphError::MissingSpace { space });
        }
        if !self.node_exists(root) {
            return Err(CanonicalGraphError::MissingAssemblyNode { node: root });
        }
        self.spaces
            .get_mut(&space)
            .expect("space checked above")
            .root = Some(root);
        Ok(())
    }

    pub(crate) fn set_central_node(
        &mut self,
        space: impl Into<DockSpaceId>,
        node: Option<AssemblyNodeId>,
        keep_alive_when_empty: bool,
        passthrough_when_empty: bool,
    ) -> Result<(), CanonicalGraphError> {
        let space = space.into();
        if !self.spaces.contains_key(&space) {
            return Err(CanonicalGraphError::MissingSpace { space });
        }
        if let Some(node) = node
            && !self.node_exists(node)
        {
            return Err(CanonicalGraphError::MissingAssemblyNode { node });
        }
        self.spaces
            .get_mut(&space)
            .expect("space checked above")
            .central = AssemblyCentralPolicy {
            node,
            keep_alive_when_empty,
            passthrough_when_empty,
        };
        Ok(())
    }

    pub(crate) fn add_floating_root(
        &mut self,
        id: impl Into<DockFloatingId>,
        space: impl Into<DockSpaceId>,
        root: AssemblyNodeId,
        bounds: DockLayoutRect,
        z_order: i32,
    ) -> Result<(), CanonicalGraphError> {
        let id = id.into();
        let space = space.into();
        if id.as_str().is_empty() {
            return Err(CanonicalGraphError::EmptyFloatingId);
        }
        if self.floating_roots.contains_key(&id) {
            return Err(CanonicalGraphError::DuplicateFloating { floating: id });
        }
        if !self.spaces.contains_key(&space) {
            return Err(CanonicalGraphError::MissingSpace { space });
        }
        if !self.node_exists(root) {
            return Err(CanonicalGraphError::MissingAssemblyNode { node: root });
        }
        if !bounds.is_finite_with_non_negative_size() || bounds.width <= 0.0 || bounds.height <= 0.0
        {
            return Err(CanonicalGraphError::InvalidFloatingBounds { floating: id });
        }
        self.floating_roots.insert(
            id.clone(),
            AssemblyFloatingRoot {
                space: space.clone(),
                root,
                bounds,
                z_order,
            },
        );
        self.spaces
            .get_mut(&space)
            .expect("space checked above")
            .floating_roots
            .push(id);
        Ok(())
    }

    pub(crate) fn publish<P: CanonicalGraphPublication>(
        self,
        mut publication: P,
    ) -> Result<CanonicalDockGraph, CanonicalGraphError> {
        let mut graph = CanonicalDockGraph {
            nodes: SlotMap::with_key(),
            groups: self.groups.clone(),
            spaces: self
                .spaces
                .iter()
                .map(|(space_id, space)| {
                    (
                        space_id.clone(),
                        CanonicalSpace {
                            root: None,
                            central: CanonicalCentralPolicy {
                                node: None,
                                keep_alive_when_empty: space.central.keep_alive_when_empty,
                                passthrough_when_empty: space.central.passthrough_when_empty,
                            },
                            floating_roots: space.floating_roots.clone(),
                        },
                    )
                })
                .collect(),
            floating_roots: BTreeMap::new(),
            index: CanonicalGraphIndex::default(),
            version: publication.current_version(),
        };
        let mut attached = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        let mut published = BTreeMap::new();
        let mut used_groups = BTreeSet::new();

        for (space_id, space) in &self.spaces {
            if let Some(root) = space.root {
                let published_root = self.materialize_node(
                    root,
                    &mut graph,
                    &mut attached,
                    &mut visiting,
                    &mut published,
                    &mut used_groups,
                )?;
                graph
                    .spaces
                    .get_mut(space_id)
                    .expect("space copied above")
                    .root = Some(published_root);
            }
        }

        for (floating_id, floating) in &self.floating_roots {
            let root = self.materialize_node(
                floating.root,
                &mut graph,
                &mut attached,
                &mut visiting,
                &mut published,
                &mut used_groups,
            )?;
            graph.floating_roots.insert(
                floating_id.clone(),
                CanonicalFloatingRoot {
                    space: floating.space.clone(),
                    root,
                    bounds: floating.bounds,
                    z_order: floating.z_order,
                },
            );
        }

        let floating_order = graph
            .floating_roots
            .iter()
            .map(|(id, root)| (id.clone(), root.z_order))
            .collect::<BTreeMap<_, _>>();
        for space in graph.spaces.values_mut() {
            space.floating_roots.sort_by(|left, right| {
                let left_z = floating_order
                    .get(left)
                    .copied()
                    .expect("floating root copied above");
                let right_z = floating_order
                    .get(right)
                    .copied()
                    .expect("floating root copied above");
                left_z.cmp(&right_z).then_with(|| left.cmp(right))
            });
        }

        if attached.len() != self.nodes.len() {
            let unattached = (0..self.nodes.len())
                .map(AssemblyNodeId)
                .find(|node| !attached.contains(node))
                .expect("length mismatch implies an unattached node");
            return Err(CanonicalGraphError::UnattachedAssemblyNode { node: unattached });
        }

        for (space_id, space) in &self.spaces {
            if let Some(central) = space.central.node {
                let node = published
                    .get(&central)
                    .copied()
                    .ok_or(CanonicalGraphError::MissingAssemblyNode { node: central })?;
                graph
                    .spaces
                    .get_mut(space_id)
                    .expect("space copied above")
                    .central
                    .node = Some(node);
            }
        }

        graph.index = CanonicalGraphIndex::build(&graph)?;
        graph.validate()?;
        graph.version = publication.next_version();
        Ok(graph)
    }

    fn push_node(&mut self, node: AssemblyNode) -> AssemblyNodeId {
        let id = AssemblyNodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    fn node_exists(&self, node: AssemblyNodeId) -> bool {
        node.0 < self.nodes.len()
    }

    fn materialize_node(
        &self,
        node: AssemblyNodeId,
        graph: &mut CanonicalDockGraph,
        attached: &mut BTreeSet<AssemblyNodeId>,
        visiting: &mut BTreeSet<AssemblyNodeId>,
        published: &mut BTreeMap<AssemblyNodeId, CanonicalNodeId>,
        used_groups: &mut BTreeSet<DockGroupId>,
    ) -> Result<CanonicalNodeId, CanonicalGraphError> {
        if published.contains_key(&node) {
            return Err(CanonicalGraphError::SharedAssemblyNode { node });
        }
        if !self.node_exists(node) {
            return Err(CanonicalGraphError::MissingAssemblyNode { node });
        }
        if !visiting.insert(node) {
            return Err(CanonicalGraphError::AssemblyCycle { node });
        }
        if !attached.insert(node) {
            return Err(CanonicalGraphError::SharedAssemblyNode { node });
        }

        let result = match self.nodes.get(node.0).expect("node checked above") {
            AssemblyNode::Group { group, size_ref } => {
                if !used_groups.insert(group.clone()) {
                    return Err(CanonicalGraphError::SharedGroup {
                        group: group.clone(),
                    });
                }
                if !graph.groups.contains_key(group) {
                    return Err(CanonicalGraphError::MissingGroup {
                        group: group.clone(),
                    });
                }
                Ok(graph.nodes.insert(CanonicalNode::Group {
                    group: group.clone(),
                    size_ref: *size_ref,
                }))
            }
            AssemblyNode::Branch {
                axis,
                first,
                second,
                ratio,
                size_ref,
            } => {
                validate_ratio(*ratio)?;
                let first = self.materialize_node(
                    *first,
                    graph,
                    attached,
                    visiting,
                    published,
                    used_groups,
                )?;
                let second = self.materialize_node(
                    *second,
                    graph,
                    attached,
                    visiting,
                    published,
                    used_groups,
                )?;
                Ok(graph.nodes.insert(CanonicalNode::Branch {
                    axis: *axis,
                    first,
                    second,
                    ratio: *ratio,
                    size_ref: *size_ref,
                }))
            }
        };
        visiting.remove(&node);
        if let Ok(published_node) = result {
            published.insert(node, published_node);
            Ok(published_node)
        } else {
            result
        }
    }
}

pub(crate) trait CanonicalGraphPublication {
    fn current_version(&self) -> CanonicalGraphVersion;
    fn next_version(&mut self) -> CanonicalGraphVersion;
}

impl CanonicalGraphPublication for &mut CanonicalGraphAuthority {
    fn current_version(&self) -> CanonicalGraphVersion {
        (**self).current()
    }

    fn next_version(&mut self) -> CanonicalGraphVersion {
        (**self).publish()
    }
}

#[cfg(test)]
impl CanonicalGraphPublication for CanonicalGraphVersion {
    fn current_version(&self) -> CanonicalGraphVersion {
        *self
    }

    fn next_version(&mut self) -> CanonicalGraphVersion {
        *self
    }
}

fn validate_ratio(ratio: f32) -> Result<(), CanonicalGraphError> {
    if ratio.is_finite() && (0.0..=1.0).contains(&ratio) {
        Ok(())
    } else {
        Err(CanonicalGraphError::InvalidRatio { ratio })
    }
}

fn validate_size_ref(size_ref: Option<CanonicalSizeRef>) -> Result<(), CanonicalGraphError> {
    if size_ref.map_or(true, CanonicalSizeRef::is_valid) {
        Ok(())
    } else {
        Err(CanonicalGraphError::InvalidSizeRef)
    }
}
