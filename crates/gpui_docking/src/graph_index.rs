#![allow(dead_code)]

use crate::graph_assembly::{
    CanonicalDockGraph, CanonicalGraphError, CanonicalGroup, CanonicalNode, CanonicalSizeRef,
};
use crate::{CanonicalNodeId, DockFloatingId, DockGroupId, DockItemId, DockSpaceId};
use slotmap::SlotMap;
use std::collections::{BTreeMap, BTreeSet};

/// The root authority that owns a canonical structural node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CanonicalNodeOwner {
    Main(DockSpaceId),
    Floating {
        space: DockSpaceId,
        floating: DockFloatingId,
    },
}

impl CanonicalNodeOwner {
    fn space(&self) -> &DockSpaceId {
        match self {
            Self::Main(space) | Self::Floating { space, .. } => space,
        }
    }
}

/// Reverse indexes derived from one canonical graph publication.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CanonicalGraphIndex {
    parent_by_node: BTreeMap<CanonicalNodeId, Option<CanonicalNodeId>>,
    owner_by_node: BTreeMap<CanonicalNodeId, CanonicalNodeOwner>,
    group_node: BTreeMap<DockGroupId, CanonicalNodeId>,
    item_group: BTreeMap<DockItemId, DockGroupId>,
    floating_by_item: BTreeMap<DockItemId, DockFloatingId>,
}

impl CanonicalGraphIndex {
    pub(crate) fn build(graph: &CanonicalDockGraph) -> Result<Self, CanonicalGraphError> {
        let mut index = Self::default();
        let mut visited = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        let mut referenced_groups = BTreeSet::new();
        let mut referenced_floatings = BTreeSet::new();

        for (space_id, space) in graph.spaces() {
            if let Some(root) = space.root {
                let owner = CanonicalNodeOwner::Main(space_id.clone());
                index.visit_node(
                    graph,
                    root,
                    &owner,
                    None,
                    &mut visited,
                    &mut visiting,
                    &mut referenced_groups,
                )?;
            }
            for floating_id in &space.floating_roots {
                if !referenced_floatings.insert(floating_id.clone()) {
                    return Err(CanonicalGraphError::SharedFloatingRoot {
                        floating: floating_id.clone(),
                    });
                }
                let floating = graph.floating_roots().get(floating_id).ok_or_else(|| {
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
                let owner = CanonicalNodeOwner::Floating {
                    space: space_id.clone(),
                    floating: floating_id.clone(),
                };
                index.visit_node(
                    graph,
                    floating.root,
                    &owner,
                    None,
                    &mut visited,
                    &mut visiting,
                    &mut referenced_groups,
                )?;
            }
        }

        if referenced_floatings.len() != graph.floating_roots().len()
            && let Some(floating) = graph
                .floating_roots()
                .keys()
                .find(|floating| !referenced_floatings.contains(*floating))
        {
            return Err(CanonicalGraphError::UnreferencedFloating {
                floating: floating.clone(),
            });
        }

        if referenced_groups.len() != graph.groups().len() {
            if let Some(group) = graph
                .groups()
                .keys()
                .find(|group| !referenced_groups.contains(*group))
            {
                return Err(CanonicalGraphError::UnreferencedGroup {
                    group: group.clone(),
                });
            }
        }
        if visited.len() != graph.nodes().len() {
            let node = graph
                .nodes()
                .keys()
                .find(|node| !visited.contains(node))
                .ok_or(CanonicalGraphError::IndexMismatch)?;
            return Err(CanonicalGraphError::MissingNode { node });
        }
        Ok(index)
    }

    pub(crate) fn parent_of(&self, node: CanonicalNodeId) -> Option<Option<CanonicalNodeId>> {
        self.parent_by_node.get(&node).copied()
    }

    pub(crate) fn space_for_node(&self, node: CanonicalNodeId) -> Option<&DockSpaceId> {
        self.owner_by_node.get(&node).map(CanonicalNodeOwner::space)
    }

    pub(crate) fn owner_for_node(&self, node: CanonicalNodeId) -> Option<&CanonicalNodeOwner> {
        self.owner_by_node.get(&node)
    }

    pub(crate) fn node_for_group(&self, group: &DockGroupId) -> Option<CanonicalNodeId> {
        self.group_node.get(group).copied()
    }

    pub(crate) fn group_for_item(&self, item: &DockItemId) -> Option<&DockGroupId> {
        self.item_group.get(item)
    }

    pub(crate) fn floating_for_item(&self, item: &DockItemId) -> Option<&DockFloatingId> {
        self.floating_by_item.get(item)
    }

    pub(crate) fn node_for_item(&self, item: &DockItemId) -> Option<CanonicalNodeId> {
        self.group_for_item(item)
            .and_then(|group| self.node_for_group(group))
    }

    /// Updates one node's retained parent without rebuilding unrelated topology.
    pub(crate) fn set_parent(&mut self, node: CanonicalNodeId, parent: Option<CanonicalNodeId>) {
        debug_assert!(self.owner_by_node.contains_key(&node));
        self.parent_by_node.insert(node, parent);
    }

    /// Promotes a sibling's contents into its existing parent while detaching the other child.
    pub(crate) fn promote_sibling_into_parent(
        &mut self,
        parent: CanonicalNodeId,
        source: CanonicalNodeId,
        sibling: CanonicalNodeId,
        promoted: &CanonicalNode,
    ) {
        debug_assert_eq!(self.parent_by_node.get(&source), Some(&Some(parent)));
        debug_assert_eq!(self.parent_by_node.get(&sibling), Some(&Some(parent)));
        debug_assert!(self.owner_by_node.contains_key(&parent));

        self.parent_by_node.insert(source, None);
        self.parent_by_node.remove(&sibling);
        self.owner_by_node.remove(&sibling);

        match promoted {
            CanonicalNode::Group { group, .. } => {
                let previous = self.group_node.insert(group.clone(), parent);
                debug_assert_eq!(previous, Some(sibling));
            }
            CanonicalNode::Branch { first, second, .. } => {
                self.parent_by_node.insert(*first, Some(parent));
                self.parent_by_node.insert(*second, Some(parent));
            }
        }
    }

    /// Removes a retired branch from the derived index.
    pub(crate) fn remove_branch(&mut self, node: CanonicalNodeId) {
        debug_assert!(!self.group_node.values().any(|candidate| *candidate == node));
        self.parent_by_node.remove(&node);
        self.owner_by_node.remove(&node);
    }

    /// Inserts a newly allocated branch into the derived index.
    pub(crate) fn insert_branch(
        &mut self,
        node: CanonicalNodeId,
        parent: Option<CanonicalNodeId>,
        owner: CanonicalNodeOwner,
    ) {
        debug_assert!(!self.parent_by_node.contains_key(&node));
        self.parent_by_node.insert(node, parent);
        self.owner_by_node.insert(node, owner);
    }

    /// Inserts a newly allocated group leaf and its durable item membership.
    pub(crate) fn insert_group(
        &mut self,
        node: CanonicalNodeId,
        group: &DockGroupId,
        state: &CanonicalGroup,
        parent: Option<CanonicalNodeId>,
        owner: CanonicalNodeOwner,
    ) {
        debug_assert!(!self.parent_by_node.contains_key(&node));
        debug_assert!(!self.group_node.contains_key(group));
        self.parent_by_node.insert(node, parent);
        self.owner_by_node.insert(node, owner.clone());
        self.group_node.insert(group.clone(), node);
        for item in &state.items {
            debug_assert!(!self.item_group.contains_key(item));
            self.item_group.insert(item.clone(), group.clone());
            if let CanonicalNodeOwner::Floating { floating, .. } = &owner {
                self.floating_by_item.insert(item.clone(), floating.clone());
            }
        }
    }

    /// Retires one source group leaf and rebinds all of its items to an existing target group.
    pub(crate) fn merge_group_into(
        &mut self,
        source_node: CanonicalNodeId,
        source_group: &DockGroupId,
        source_items: &[DockItemId],
        target_group: &DockGroupId,
        target_owner: &CanonicalNodeOwner,
    ) {
        let removed_node = self.group_node.remove(source_group);
        debug_assert_eq!(removed_node, Some(source_node));
        self.parent_by_node.remove(&source_node);
        self.owner_by_node.remove(&source_node);

        for item in source_items {
            self.move_item_to_group(item, source_group, target_group, target_owner);
        }
    }

    /// Preserves a moved payload group while retiring the target group's content identity.
    ///
    /// The target structural leaf remains in place because structural identities are replaceable;
    /// its durable group binding changes to the source group carried by the payload.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn merge_target_group_into_source_at_target_leaf(
        &mut self,
        source_node: CanonicalNodeId,
        source_group: &DockGroupId,
        source_items: &[DockItemId],
        target_node: CanonicalNodeId,
        target_group: &DockGroupId,
        target_items: &[DockItemId],
        target_owner: &CanonicalNodeOwner,
    ) {
        let removed_source = self.group_node.remove(source_group);
        debug_assert_eq!(removed_source, Some(source_node));
        let removed_target = self.group_node.remove(target_group);
        debug_assert_eq!(removed_target, Some(target_node));
        self.parent_by_node.remove(&source_node);
        self.owner_by_node.remove(&source_node);
        let replaced = self.group_node.insert(source_group.clone(), target_node);
        debug_assert!(replaced.is_none());

        for item in source_items {
            self.move_item_to_group(item, source_group, source_group, target_owner);
        }
        for item in target_items {
            self.move_item_to_group(item, target_group, source_group, target_owner);
        }
    }

    /// Rebinds one item to an existing target group and its current root owner.
    pub(crate) fn move_item_to_group(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        target_owner: &CanonicalNodeOwner,
    ) {
        let previous = self.item_group.insert(item.clone(), target_group.clone());
        debug_assert_eq!(previous.as_ref(), Some(source_group));
        match target_owner {
            CanonicalNodeOwner::Main(_) => {
                self.floating_by_item.remove(item);
            }
            CanonicalNodeOwner::Floating { floating, .. } => {
                self.floating_by_item.insert(item.clone(), floating.clone());
            }
        }
    }

    /// Removes one item membership before inserting it into a newly allocated group leaf.
    pub(crate) fn remove_item_membership(&mut self, item: &DockItemId, source_group: &DockGroupId) {
        let previous = self.item_group.remove(item);
        debug_assert_eq!(previous.as_ref(), Some(source_group));
        self.floating_by_item.remove(item);
    }

    /// Reassigns ownership for a moved subtree and updates floating item membership locally.
    pub(crate) fn update_owner_subtree(
        &mut self,
        nodes: &SlotMap<CanonicalNodeId, CanonicalNode>,
        groups: &BTreeMap<DockGroupId, CanonicalGroup>,
        root: CanonicalNodeId,
        owner: CanonicalNodeOwner,
    ) {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            self.owner_by_node.insert(node, owner.clone());
            match nodes.get(node).expect("owned subtree node must exist") {
                CanonicalNode::Group { group, .. } => {
                    let state = groups.get(group).expect("owned group must exist");
                    match &owner {
                        CanonicalNodeOwner::Main(_) => {
                            for item in &state.items {
                                self.floating_by_item.remove(item);
                            }
                        }
                        CanonicalNodeOwner::Floating { floating, .. } => {
                            for item in &state.items {
                                self.floating_by_item.insert(item.clone(), floating.clone());
                            }
                        }
                    }
                }
                CanonicalNode::Branch { first, second, .. } => {
                    pending.push(*first);
                    pending.push(*second);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_node(
        &mut self,
        graph: &CanonicalDockGraph,
        node: CanonicalNodeId,
        owner: &CanonicalNodeOwner,
        parent: Option<CanonicalNodeId>,
        visited: &mut BTreeSet<CanonicalNodeId>,
        visiting: &mut BTreeSet<CanonicalNodeId>,
        referenced_groups: &mut BTreeSet<DockGroupId>,
    ) -> Result<(), CanonicalGraphError> {
        if !graph.nodes().contains_key(node) {
            return Err(CanonicalGraphError::MissingNode { node });
        }
        if !visiting.insert(node) {
            return Err(CanonicalGraphError::CanonicalCycle { node });
        }
        if !visited.insert(node) {
            return Err(CanonicalGraphError::SharedCanonicalNode { node });
        }
        self.parent_by_node.insert(node, parent);
        self.owner_by_node.insert(node, owner.clone());

        match graph.nodes().get(node).expect("node checked above") {
            CanonicalNode::Group { group, size_ref } => {
                validate_size_ref(*size_ref)?;
                if !referenced_groups.insert(group.clone()) {
                    return Err(CanonicalGraphError::SharedGroup {
                        group: group.clone(),
                    });
                }
                let group_data =
                    graph
                        .groups()
                        .get(group)
                        .ok_or_else(|| CanonicalGraphError::MissingGroup {
                            group: group.clone(),
                        })?;
                if group_data.items.is_empty() {
                    return Err(CanonicalGraphError::EmptyGroup {
                        group: group.clone(),
                    });
                }
                if !group_data.items.contains(&group_data.selected) {
                    return Err(CanonicalGraphError::SelectedItemMissing {
                        group: group.clone(),
                    });
                }
                let mut mru_items = BTreeSet::new();
                for item in &group_data.mru {
                    if !group_data.items.contains(item) {
                        return Err(CanonicalGraphError::MruItemMissing {
                            group: group.clone(),
                            item: item.clone(),
                        });
                    }
                    if !mru_items.insert(item.clone()) {
                        return Err(CanonicalGraphError::DuplicateMruItem {
                            group: group.clone(),
                            item: item.clone(),
                        });
                    }
                }
                self.group_node.insert(group.clone(), node);
                for item in &group_data.items {
                    if self
                        .item_group
                        .insert(item.clone(), group.clone())
                        .is_some()
                    {
                        return Err(CanonicalGraphError::DuplicateItem { item: item.clone() });
                    }
                    if let CanonicalNodeOwner::Floating { floating, .. } = owner {
                        self.floating_by_item.insert(item.clone(), floating.clone());
                    }
                }
            }
            CanonicalNode::Branch {
                first,
                second,
                ratio,
                size_ref,
                ..
            } => {
                if !ratio.is_finite() || !(0.0..=1.0).contains(ratio) {
                    return Err(CanonicalGraphError::InvalidRatio { ratio: *ratio });
                }
                validate_size_ref(*size_ref)?;
                self.visit_node(
                    graph,
                    *first,
                    owner,
                    Some(node),
                    visited,
                    visiting,
                    referenced_groups,
                )?;
                self.visit_node(
                    graph,
                    *second,
                    owner,
                    Some(node),
                    visited,
                    visiting,
                    referenced_groups,
                )?;
            }
        }
        visiting.remove(&node);
        Ok(())
    }
}

fn validate_size_ref(size_ref: Option<CanonicalSizeRef>) -> Result<(), CanonicalGraphError> {
    if size_ref.map_or(true, CanonicalSizeRef::is_valid) {
        Ok(())
    } else {
        Err(CanonicalGraphError::InvalidSizeRef)
    }
}
