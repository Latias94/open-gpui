#![allow(dead_code)]

use crate::legacy::binary_layout_policy::{StructuralIdentityRemap, plan_sibling_promotion};
use crate::legacy::graph_index::{CanonicalGraphIndex, CanonicalNodeOwner};
use crate::legacy::graph_patch::{
    GraphCapacityProof, GraphCommitEffects, GraphCommitId, GraphFloatingBounds,
    GraphGroupCapacityRequirement, GraphPatchOperation, GraphPrepareError, GraphPreparedMutation,
    GraphRevision, GraphSpaceCapacityRequirement, GraphSupersededReason, mru_changes_after_select,
    selected_item_after_close,
};
use crate::legacy::{
    CanonicalGraphAuthority, CanonicalGraphVersion, CanonicalNodeId, DockFloatingId, DockGroupId,
    DockItemId, DockLayoutRect, DockSpaceId, SplitAxis,
};
use slotmap::SlotMap;
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    groups: HashMap<DockGroupId, CanonicalGroup>,
    spaces: BTreeMap<DockSpaceId, CanonicalSpace>,
    floating_roots: HashMap<DockFloatingId, CanonicalFloatingRoot>,
    index: CanonicalGraphIndex,
    version: CanonicalGraphVersion,
}

#[derive(Debug, Clone, PartialEq)]
struct CanonicalLeafDetach {
    size_ref: Option<CanonicalSizeRef>,
    remap: Option<StructuralIdentityRemap>,
    effects: GraphCommitEffects,
}

#[derive(Debug)]
struct CanonicalGroupCapacityRequest {
    group: DockGroupId,
    additional_items: usize,
    additional_mru: usize,
}

#[derive(Debug)]
struct CanonicalSpaceCapacityRequest {
    space: DockSpaceId,
    additional_floating_roots: usize,
}

#[derive(Debug, Default)]
struct CanonicalGraphCapacityPlan {
    additional_nodes: usize,
    additional_groups: usize,
    additional_floatings: usize,
    additional_group_index: usize,
    additional_item_index: usize,
    additional_floating_item_index: usize,
    groups: SmallVec<[CanonicalGroupCapacityRequest; 3]>,
    spaces: SmallVec<[CanonicalSpaceCapacityRequest; 2]>,
}

impl CanonicalGraphCapacityPlan {
    fn reserve_group_vectors(
        &mut self,
        group: DockGroupId,
        additional_items: usize,
        additional_mru: usize,
    ) {
        if let Some(request) = self
            .groups
            .iter_mut()
            .find(|request| request.group == group)
        {
            request.additional_items = request.additional_items.max(additional_items);
            request.additional_mru = request.additional_mru.max(additional_mru);
            return;
        }
        self.groups.push(CanonicalGroupCapacityRequest {
            group,
            additional_items,
            additional_mru,
        });
        debug_assert!(!self.groups.spilled());
    }

    fn reserve_space_floatings(&mut self, space: DockSpaceId, additional: usize) {
        if let Some(request) = self
            .spaces
            .iter_mut()
            .find(|request| request.space == space)
        {
            request.additional_floating_roots = request.additional_floating_roots.max(additional);
            return;
        }
        self.spaces.push(CanonicalSpaceCapacityRequest {
            space,
            additional_floating_roots: additional,
        });
        debug_assert!(!self.spaces.spilled());
    }
}

fn is_prepared_single_item_group(group: &CanonicalGroup, item: &DockItemId) -> bool {
    group.items.as_slice() == std::slice::from_ref(item)
        && group.selected == *item
        && group.mru.as_slice() == std::slice::from_ref(item)
}

impl CanonicalLeafDetach {
    fn remap_node(&self, node: CanonicalNodeId) -> CanonicalNodeId {
        match self.remap {
            Some(remap) if remap.from == node => remap.to,
            Some(_) | None => node,
        }
    }
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

    pub(crate) fn groups(&self) -> &HashMap<DockGroupId, CanonicalGroup> {
        &self.groups
    }

    pub(crate) fn spaces(&self) -> &BTreeMap<DockSpaceId, CanonicalSpace> {
        &self.spaces
    }

    pub(crate) fn floating_roots(&self) -> &HashMap<DockFloatingId, CanonicalFloatingRoot> {
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

    pub(crate) fn try_reserve_for_patch(
        &mut self,
        commit_id: GraphCommitId,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
    ) -> Result<GraphCapacityProof, GraphPrepareError> {
        let plan = self.capacity_plan(operation, mutation)?;
        let additional_nodes = plan.additional_nodes;
        let additional_groups = plan.additional_groups;
        let additional_floatings = plan.additional_floatings;
        let additional_group_index = plan.additional_group_index;
        let additional_item_index = plan.additional_item_index;
        let additional_floating_item_index = plan.additional_floating_item_index;

        self.nodes
            .try_reserve(additional_nodes)
            .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
        self.groups
            .try_reserve(additional_groups)
            .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
        self.floating_roots
            .try_reserve(additional_floatings)
            .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
        self.index
            .try_reserve_for_graph_growth(
                self.nodes.capacity(),
                additional_group_index,
                additional_item_index,
                additional_floating_item_index,
            )
            .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;

        let mut group_vectors = SmallVec::new();
        for request in plan.groups {
            let group = self.groups.get_mut(&request.group).ok_or_else(|| {
                GraphPrepareError::MissingGroup {
                    group: request.group.clone(),
                }
            })?;
            group
                .items
                .try_reserve(request.additional_items)
                .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
            group
                .mru
                .try_reserve(request.additional_mru)
                .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
            group_vectors.push(GraphGroupCapacityRequirement {
                group: request.group,
                additional_items: request.additional_items,
                additional_mru: request.additional_mru,
            });
        }
        debug_assert!(!group_vectors.spilled());

        let mut space_vectors = SmallVec::new();
        for request in plan.spaces {
            let space = self.spaces.get_mut(&request.space).ok_or_else(|| {
                GraphPrepareError::MissingSpace {
                    space: request.space.clone(),
                }
            })?;
            space
                .floating_roots
                .try_reserve(request.additional_floating_roots)
                .map_err(|_| GraphPrepareError::GraphCapacityExhausted)?;
            space_vectors.push(GraphSpaceCapacityRequirement {
                space: request.space,
                additional_floating_roots: request.additional_floating_roots,
            });
        }
        debug_assert!(!space_vectors.spilled());

        let revision = GraphRevision::from_version(self.version);
        Ok(GraphCapacityProof::new(
            commit_id,
            revision.authority_epoch(),
            revision.allocator(),
            self.nodes.capacity(),
            additional_nodes,
            additional_groups,
            additional_floatings,
            additional_group_index,
            additional_item_index,
            additional_floating_item_index,
            group_vectors,
            space_vectors,
        ))
    }

    pub(crate) fn validate_capacity_proof(
        &self,
        commit_id: GraphCommitId,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
        proof: &GraphCapacityProof,
    ) -> Result<(), GraphSupersededReason> {
        if proof.commit_id != commit_id {
            return Err(GraphSupersededReason::TransactionOwnerChanged);
        }
        let revision = GraphRevision::from_version(self.version);
        if proof.authority_epoch != revision.authority_epoch() {
            return Err(GraphSupersededReason::AuthorityChanged);
        }
        if proof.allocator_revision != revision.allocator() {
            return Err(GraphSupersededReason::AllocatorChanged);
        }

        let Ok(plan) = self.capacity_plan(operation, mutation) else {
            return Err(GraphSupersededReason::CapacityProofMismatch);
        };
        let requirements_match = proof.required_node_key_capacity <= self.nodes.capacity()
            && proof.additional_nodes == plan.additional_nodes
            && proof.additional_groups == plan.additional_groups
            && proof.additional_floatings == plan.additional_floatings
            && proof.additional_group_index == plan.additional_group_index
            && proof.additional_item_index == plan.additional_item_index
            && proof.additional_floating_item_index == plan.additional_floating_item_index
            && proof.group_vectors.len() == plan.groups.len()
            && proof.space_vectors.len() == plan.spaces.len()
            && proof
                .group_vectors
                .iter()
                .zip(plan.groups.iter())
                .all(|(proof, request)| {
                    proof.group == request.group
                        && proof.additional_items == request.additional_items
                        && proof.additional_mru == request.additional_mru
                })
            && proof
                .space_vectors
                .iter()
                .zip(plan.spaces.iter())
                .all(|(proof, request)| {
                    proof.space == request.space
                        && proof.additional_floating_roots == request.additional_floating_roots
                });
        if !requirements_match {
            return Err(GraphSupersededReason::CapacityProofMismatch);
        }

        fn has_headroom(len: usize, additional: usize, capacity: usize) -> bool {
            len.checked_add(additional)
                .is_some_and(|required| required <= capacity)
        }

        if !has_headroom(
            self.nodes.len(),
            proof.additional_nodes,
            self.nodes.capacity(),
        ) || !has_headroom(
            self.groups.len(),
            proof.additional_groups,
            self.groups.capacity(),
        ) || !has_headroom(
            self.floating_roots.len(),
            proof.additional_floatings,
            self.floating_roots.capacity(),
        ) || !self.index.has_capacity_for_graph_growth(
            self.nodes.capacity(),
            proof.additional_group_index,
            proof.additional_item_index,
            proof.additional_floating_item_index,
        ) {
            return Err(GraphSupersededReason::AllocatorChanged);
        }
        for requirement in &proof.group_vectors {
            let Some(group) = self.groups.get(&requirement.group) else {
                return Err(GraphSupersededReason::GroupMissing);
            };
            if !has_headroom(
                group.items.len(),
                requirement.additional_items,
                group.items.capacity(),
            ) || !has_headroom(
                group.mru.len(),
                requirement.additional_mru,
                group.mru.capacity(),
            ) {
                return Err(GraphSupersededReason::AllocatorChanged);
            }
        }
        for requirement in &proof.space_vectors {
            let Some(space) = self.spaces.get(&requirement.space) else {
                return Err(GraphSupersededReason::SpaceMissing);
            };
            if !has_headroom(
                space.floating_roots.len(),
                requirement.additional_floating_roots,
                space.floating_roots.capacity(),
            ) {
                return Err(GraphSupersededReason::AllocatorChanged);
            }
        }
        Ok(())
    }

    fn capacity_plan(
        &self,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
    ) -> Result<CanonicalGraphCapacityPlan, GraphPrepareError> {
        self.validate_prepared_mutation(operation, mutation)?;
        let mut plan = CanonicalGraphCapacityPlan::default();
        match operation {
            GraphPatchOperation::SelectTab { group, .. } => {
                self.require_group(group)?;
                plan.reserve_group_vectors(group.clone(), 0, 1);
            }
            GraphPatchOperation::CloseItem { group, .. } => {
                self.require_group(group)?;
            }
            GraphPatchOperation::SplitGroup {
                target_group,
                items,
                ..
            } => {
                plan.additional_nodes = 2;
                plan.additional_groups = 1;
                plan.additional_group_index = 1;
                plan.additional_item_index = items.len();
                if self.group_is_floating(target_group)? {
                    plan.additional_floating_item_index = items.len();
                }
            }
            GraphPatchOperation::MoveGroupToEdge {
                source_group,
                target_group,
                ..
            } => {
                plan.additional_nodes = 2;
                if !self.group_is_floating(source_group)? && self.group_is_floating(target_group)? {
                    plan.additional_floating_item_index =
                        self.require_group(source_group)?.items.len();
                }
            }
            GraphPatchOperation::MoveGroupToCenter {
                source_group,
                target_group,
                ..
            } => {
                let source = self.require_group(source_group)?;
                let target = self.require_group(target_group)?;
                plan.reserve_group_vectors(target_group.clone(), source.items.len(), 0);
                plan.reserve_group_vectors(
                    source_group.clone(),
                    0,
                    target.mru.len().saturating_add(2),
                );
                if !self.group_is_floating(source_group)? && self.group_is_floating(target_group)? {
                    plan.additional_floating_item_index = source.items.len();
                }
            }
            GraphPatchOperation::MoveGroupToEmptySpace { source_group, .. } => {
                self.require_group(source_group)?;
                plan.additional_nodes = 1;
            }
            GraphPatchOperation::MoveItemToGroup {
                source_group,
                target_group,
                ..
            } => {
                self.require_group(source_group)?;
                self.require_group(target_group)?;
                if source_group == target_group {
                    plan.reserve_group_vectors(source_group.clone(), 0, 1);
                } else {
                    plan.reserve_group_vectors(target_group.clone(), 1, 1);
                    if !self.group_is_floating(source_group)?
                        && self.group_is_floating(target_group)?
                    {
                        plan.additional_floating_item_index = 1;
                    }
                }
            }
            GraphPatchOperation::MoveItemToEdge {
                source_group,
                target_group,
                ..
            } => {
                let source_len = self.require_group(source_group)?.items.len();
                self.require_group(target_group)?;
                plan.additional_nodes = 2;
                if source_len > 1 {
                    plan.additional_groups = 1;
                    plan.additional_group_index = 1;
                }
                if !self.group_is_floating(source_group)? && self.group_is_floating(target_group)? {
                    plan.additional_floating_item_index = 1;
                }
            }
            GraphPatchOperation::MoveItemToEmptySpace { source_group, .. } => {
                let source_len = self.require_group(source_group)?.items.len();
                plan.additional_nodes = 1;
                if source_len > 1 {
                    plan.additional_groups = 1;
                    plan.additional_group_index = 1;
                }
            }
            GraphPatchOperation::FloatItemInWindow {
                source_group,
                target_space,
                ..
            } => {
                let source_len = self.require_group(source_group)?.items.len();
                plan.additional_nodes = 1;
                plan.additional_floatings = 1;
                if source_len > 1 {
                    plan.additional_groups = 1;
                    plan.additional_group_index = 1;
                }
                if !self.group_is_floating(source_group)? {
                    plan.additional_floating_item_index = 1;
                }
                plan.reserve_space_floatings(target_space.clone(), 1);
            }
            GraphPatchOperation::SetFloatingBounds {
                floating, space, ..
            }
            | GraphPatchOperation::RaiseFloating {
                floating, space, ..
            } => {
                let root = self.floating_roots.get(floating).ok_or_else(|| {
                    GraphPrepareError::MissingFloating {
                        floating: floating.clone(),
                    }
                })?;
                if root.space != *space {
                    return Err(GraphPrepareError::FloatingOutsideSpace {
                        floating: floating.clone(),
                        space: space.clone(),
                    });
                }
                if !self.spaces.contains_key(space) {
                    return Err(GraphPrepareError::MissingSpace {
                        space: space.clone(),
                    });
                }
            }
            GraphPatchOperation::MoveFloatingRootToEdge { target_group, .. } => {
                self.require_group(target_group)?;
                plan.additional_nodes = 1;
            }
        }
        Ok(plan)
    }

    fn validate_prepared_mutation(
        &self,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
    ) -> Result<(), GraphPrepareError> {
        let valid = match (operation, mutation) {
            (
                GraphPatchOperation::SelectTab { group, item },
                GraphPreparedMutation::SelectTab {
                    selection_changed,
                    interaction_changed,
                },
            ) => self.groups.get(group).is_some_and(|state| {
                state.items.contains(item)
                    && *selection_changed == (state.selected != *item)
                    && *interaction_changed == mru_changes_after_select(&state.mru, item)
            }),
            (
                GraphPatchOperation::MoveGroupToEdge { .. },
                GraphPreparedMutation::MoveGroupToEdge,
            )
            | (
                GraphPatchOperation::MoveGroupToCenter { .. },
                GraphPreparedMutation::MoveGroupToCenter,
            )
            | (
                GraphPatchOperation::MoveGroupToEmptySpace { .. },
                GraphPreparedMutation::MoveGroupToEmptySpace,
            )
            | (
                GraphPatchOperation::MoveItemToGroup { .. },
                GraphPreparedMutation::MoveItemToGroup,
            ) => true,
            (
                GraphPatchOperation::CloseItem { group, item, .. },
                GraphPreparedMutation::CloseItem {
                    next_selected,
                    selection_changed,
                    interaction_changed,
                },
            ) => self.groups.get(group).is_some_and(|state| {
                let expected_next =
                    selected_item_after_close(&state.items, &state.selected, &state.mru, item);
                state.items.contains(item)
                    && expected_next.as_ref() == next_selected.as_ref()
                    && *selection_changed == (expected_next.as_ref() != Some(&state.selected))
                    && *interaction_changed == state.mru.contains(item)
            }),
            (
                GraphPatchOperation::SetFloatingBounds {
                    floating,
                    space,
                    bounds,
                },
                GraphPreparedMutation::SetFloatingBounds,
            ) => self.floating_roots.get(floating).is_some_and(|root| {
                root.space == *space
                    && GraphFloatingBounds::new(root.bounds)
                        .is_some_and(|current| current != *bounds)
            }),
            (
                GraphPatchOperation::RaiseFloating {
                    floating,
                    space,
                    z_order,
                },
                GraphPreparedMutation::RaiseFloating,
            ) => self
                .floating_roots
                .get(floating)
                .zip(self.spaces.get(space))
                .is_some_and(|(root, space_state)| {
                    let expected_z_order = space_state
                        .floating_roots
                        .iter()
                        .filter_map(|candidate| self.floating_roots.get(candidate))
                        .map(|candidate| candidate.z_order)
                        .max()
                        .and_then(|max| max.checked_add(1));
                    root.space == *space
                        && space_state
                            .floating_roots
                            .iter()
                            .position(|candidate| candidate == floating)
                            .is_some_and(|index| {
                                index + 1 < space_state.floating_roots.len()
                                    && expected_z_order == Some(*z_order)
                            })
                }),
            (
                GraphPatchOperation::SplitGroup {
                    items, selected, ..
                },
                GraphPreparedMutation::SplitGroup { new_group },
            ) => {
                new_group.items == *items
                    && new_group.selected == *selected
                    && new_group.mru.is_empty()
            }
            (
                GraphPatchOperation::MoveItemToEdge {
                    item, source_group, ..
                },
                GraphPreparedMutation::MoveItemToEdge { new_group },
            )
            | (
                GraphPatchOperation::MoveItemToEmptySpace {
                    item, source_group, ..
                },
                GraphPreparedMutation::MoveItemToEmptySpace { new_group },
            )
            | (
                GraphPatchOperation::FloatItemInWindow {
                    item, source_group, ..
                },
                GraphPreparedMutation::FloatItemInWindow { new_group },
            ) => self.groups.get(source_group).is_some_and(|source| {
                if source.items.len() == 1 {
                    new_group.is_none()
                } else {
                    new_group
                        .as_ref()
                        .is_some_and(|group| is_prepared_single_item_group(group, item))
                }
            }),
            (
                GraphPatchOperation::MoveFloatingRootToEdge {
                    floating,
                    source_space,
                    ..
                },
                GraphPreparedMutation::MoveFloatingRootToEdge { owner_nodes },
            ) => self.floating_roots.get(floating).is_some_and(|root| {
                let expected_count = self
                    .nodes
                    .keys()
                    .filter(|node| {
                        matches!(
                            self.index.owner_for_node(*node),
                            Some(CanonicalNodeOwner::Floating {
                                space,
                                floating: owner_floating,
                            }) if space == source_space && owner_floating == floating
                        )
                    })
                    .count();
                root.space == *source_space
                    && owner_nodes.first() == Some(&root.root)
                    && owner_nodes.len() == expected_count
                    && owner_nodes.iter().enumerate().all(|(index, node)| {
                        !owner_nodes[..index].contains(node)
                            && matches!(
                                self.index.owner_for_node(*node),
                                Some(CanonicalNodeOwner::Floating {
                                    space,
                                    floating: owner_floating,
                                }) if space == source_space && owner_floating == floating
                            )
                    })
            }),
            _ => false,
        };
        valid
            .then_some(())
            .ok_or(GraphPrepareError::PreparedMutationMismatch)
    }

    fn require_group(&self, group: &DockGroupId) -> Result<&CanonicalGroup, GraphPrepareError> {
        self.groups
            .get(group)
            .ok_or_else(|| GraphPrepareError::MissingGroup {
                group: group.clone(),
            })
    }

    fn group_is_floating(&self, group: &DockGroupId) -> Result<bool, GraphPrepareError> {
        let node =
            self.index
                .node_for_group(group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: group.clone(),
                })?;
        let owner =
            self.index
                .owner_for_node(node)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: group.clone(),
                })?;
        Ok(matches!(owner, CanonicalNodeOwner::Floating { .. }))
    }

    /// Applies a selection/MRU update after the transaction boundary has revalidated its read set.
    ///
    /// This is intentionally the only production mutation entry point for the canonical group
    /// state in the first migration slice. Callers must prove membership before entering it; the
    /// optional result is retained as an invariant check so future patch kinds can share the same
    /// boundary.
    pub(crate) fn apply_selection_for_commit(
        &mut self,
        group: &DockGroupId,
        item: &DockItemId,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let Some(group_state) = self.groups.get_mut(group) else {
            return None;
        };
        if !group_state.items.contains(item) {
            return None;
        }
        group_state.mru.retain(|entry| entry != item);
        group_state.mru.insert(0, item.clone());
        group_state.selected = item.clone();
        self.version = version;
        Some(GraphCommitEffects::default())
    }

    /// Closes one item and locally collapses its leaf when the durable group becomes empty.
    pub(crate) fn apply_close_item_for_commit(
        &mut self,
        group: &DockGroupId,
        item: &DockItemId,
        space: &DockSpaceId,
        next_selected: Option<DockItemId>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let source_node = self.index.node_for_group(group)?;
        let source_owner = self.index.owner_for_node(source_node)?.clone();
        let owner_space = match &source_owner {
            CanonicalNodeOwner::Main(owner_space)
            | CanonicalNodeOwner::Floating {
                space: owner_space, ..
            } => owner_space,
        };
        if owner_space != space {
            return None;
        }
        let source = self.groups.get(group)?;
        let source_index = source
            .items
            .iter()
            .position(|candidate| candidate == item)?;
        let expected_next =
            selected_item_after_close(&source.items, &source.selected, &source.mru, item);
        if expected_next != next_selected {
            return None;
        }

        let effects = if source.items.len() == 1 {
            let detach = self.detach_group_leaf_for_commit(source_node, &source_owner)?;
            self.index.remove_item_membership(item, group);
            self.groups
                .remove(group)
                .expect("validated closed group must remain present");
            let mut effects = detach.effects;
            effects.record_retired_group(group.clone());
            effects
        } else {
            let next_selected =
                next_selected.expect("validated non-empty close must retain one selected item");
            let source = self
                .groups
                .get_mut(group)
                .expect("validated closed group must remain present");
            source.items.remove(source_index);
            source.mru.retain(|candidate| candidate != item);
            source.selected = next_selected;
            self.index.remove_item_membership(item, group);
            GraphCommitEffects::default()
        };
        self.version = version;
        Some(effects)
    }

    /// Updates one floating root's durable bounds after the read set is revalidated.
    pub(crate) fn apply_set_floating_bounds_for_commit(
        &mut self,
        floating: &DockFloatingId,
        space: &DockSpaceId,
        bounds: DockLayoutRect,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let root = self.floating_roots.get_mut(floating)?;
        if root.space != *space
            || GraphFloatingBounds::new(bounds).is_none()
            || root.bounds == bounds
        {
            return None;
        }
        root.bounds = bounds;
        self.version = version;
        Some(GraphCommitEffects::default())
    }

    /// Raises one floating root while preserving the canonical z-order list.
    pub(crate) fn apply_raise_floating_for_commit(
        &mut self,
        floating: &DockFloatingId,
        space: &DockSpaceId,
        z_order: i32,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let root = self.floating_roots.get(floating)?;
        if root.space != *space {
            return None;
        }
        let space_state = self.spaces.get_mut(space)?;
        let index = space_state
            .floating_roots
            .iter()
            .position(|candidate| candidate == floating)?;
        if index + 1 == space_state.floating_roots.len() {
            return None;
        }
        let expected_z_order = space_state
            .floating_roots
            .iter()
            .filter_map(|candidate| self.floating_roots.get(candidate))
            .map(|candidate| candidate.z_order)
            .max()?
            .checked_add(1)?;
        if expected_z_order != z_order {
            return None;
        }
        self.floating_roots
            .get_mut(floating)
            .expect("validated floating root must remain present")
            .z_order = z_order;
        let entry = space_state.floating_roots.remove(index);
        space_state.floating_roots.push(entry);
        self.version = version;
        Some(GraphCommitEffects::default())
    }

    /// Inserts a new group beside an existing group and rewires only its local parent/root path.
    pub(crate) fn apply_split_group_for_commit(
        &mut self,
        target_group: &DockGroupId,
        new_group: &DockGroupId,
        new_group_state: CanonicalGroup,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        space: &DockSpaceId,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let owner_space = match &owner {
            CanonicalNodeOwner::Main(owner_space)
            | CanonicalNodeOwner::Floating {
                space: owner_space, ..
            } => owner_space,
        };
        if owner_space != space
            || self.groups.contains_key(new_group)
            || new_group_state.items.is_empty()
            || !new_group_state.items.contains(&new_group_state.selected)
            || new_group_state
                .mru
                .iter()
                .any(|item| !new_group_state.items.contains(item))
            || new_group_state
                .mru
                .iter()
                .enumerate()
                .any(|(index, item)| new_group_state.mru[..index].contains(item))
            || !ratio.is_finite()
            || !(0.0..=1.0).contains(&ratio)
            || ratio <= 0.0
            || ratio >= 1.0
        {
            return None;
        }
        if new_group_state
            .items
            .iter()
            .any(|item| self.index.group_for_item(item).is_some())
        {
            return None;
        }

        let parent = self.index.parent_of(target_node).flatten();
        if let Some(parent) = parent
            && !matches!(self.nodes.get(parent), Some(CanonicalNode::Branch { .. }))
        {
            return None;
        }

        self.groups.insert(new_group.clone(), new_group_state);
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
            match &owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    self.spaces
                        .get_mut(owner_space)
                        .expect("validated main owner must remain present")
                        .root = Some(branch_id);
                }
                CanonicalNodeOwner::Floating { floating, .. } => {
                    self.floating_roots
                        .get_mut(floating)
                        .expect("validated floating owner must remain present")
                        .root = branch_id;
                }
            }
        }

        let new_group_state = self
            .groups
            .get(new_group)
            .expect("validated new group must remain present");
        self.index.insert_group(
            new_leaf,
            new_group,
            new_group_state,
            Some(branch_id),
            owner.clone(),
        );
        self.index.insert_branch(branch_id, parent, owner.clone());
        self.index.set_parent(target_node, Some(branch_id));
        self.version = version;
        let mut effects = GraphCommitEffects::default();
        effects.record_created_group(new_group.clone());
        effects.record_created_node(new_leaf);
        effects.record_created_node(branch_id);
        Some(effects)
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
    ) -> Option<GraphCommitEffects> {
        if source_group == target_group
            || !ratio.is_finite()
            || !(0.0..=1.0).contains(&ratio)
            || ratio <= 0.0
            || ratio >= 1.0
        {
            return None;
        }
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space || target_owner_space != target_space {
            return None;
        }
        let Some(detach) = self.detach_group_leaf_for_commit(source_node, &source_owner) else {
            return None;
        };
        let target_node = detach.remap_node(target_node);

        let target_parent = self
            .index
            .parent_of(target_node)
            .expect("validated target group must remain indexed");
        if let Some(parent) = target_parent {
            assert!(
                matches!(
                    self.nodes.get(parent),
                    Some(CanonicalNode::Branch { first, second, .. })
                        if *first == target_node || *second == target_node
                ),
                "reachable target parent must reference the target node"
            );
        }
        let source_leaf = self.bind_detached_group_leaf(
            source_group,
            detach.size_ref,
            None,
            target_owner.clone(),
        );
        let branch_id = self.nodes.insert(if insert_before {
            CanonicalNode::Branch {
                axis,
                first: source_leaf,
                second: target_node,
                ratio,
                size_ref: None,
            }
        } else {
            CanonicalNode::Branch {
                axis,
                first: target_node,
                second: source_leaf,
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
                _ => unreachable!("validated target parent must reference the target node"),
            }
        } else {
            match &target_owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    self.spaces
                        .get_mut(owner_space)
                        .expect("validated target space must remain present")
                        .root = Some(branch_id);
                }
                CanonicalNodeOwner::Floating { floating, .. } => {
                    self.floating_roots
                        .get_mut(floating)
                        .expect("validated target floating root must remain present")
                        .root = branch_id;
                }
            }
        }
        self.index
            .insert_branch(branch_id, target_parent, target_owner.clone());
        self.index.set_parent(target_node, Some(branch_id));
        self.index.set_parent(source_leaf, Some(branch_id));
        self.version = version;
        let mut effects = detach.effects;
        effects.record_created_node(source_leaf);
        effects.record_created_node(branch_id);
        Some(effects)
    }

    /// Merges a target group into a complete source payload and preserves the source identity.
    ///
    /// The target structural leaf remains at the destination and is rebound to the source group.
    /// Structural leaves are replaceable, while a whole-group payload carries durable content
    /// identity across the move.
    pub(crate) fn apply_move_group_to_center_for_commit(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        if source_group == target_group {
            return None;
        }
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space || target_owner_space != target_space {
            return None;
        }
        let Some(detach) = self.detach_group_leaf_for_commit(source_node, &source_owner) else {
            return None;
        };
        let target_node = detach.remap_node(target_node);

        let mut target_state = self
            .groups
            .remove(target_group)
            .expect("validated target group must remain present");
        let source_state = self
            .groups
            .get_mut(source_group)
            .expect("validated source group must remain present");
        self.index.merge_target_group_into_source_at_target_leaf(
            source_node,
            source_group,
            &source_state.items,
            target_node,
            target_group,
            &target_state.items,
            &target_owner,
        );

        debug_assert!(
            target_state.items.capacity()
                >= target_state
                    .items
                    .len()
                    .saturating_add(source_state.items.len())
        );
        target_state.items.extend(source_state.items.drain(..));
        source_state.items = target_state.items;

        let source_selected = source_state.selected.clone();
        source_state
            .mru
            .retain(|candidate| candidate != &source_selected);
        source_state.mru.insert(0, source_selected);
        for candidate in std::iter::once(target_state.selected).chain(target_state.mru) {
            if !source_state.mru.contains(&candidate) {
                source_state.mru.push(candidate);
            }
        }

        let CanonicalNode::Group { group, .. } = self
            .nodes
            .get_mut(target_node)
            .expect("validated target node must remain present")
        else {
            unreachable!("validated target group must reference a group leaf");
        };
        *group = source_group.clone();

        self.version = version;
        let mut effects = detach.effects;
        effects.record_retired_group(target_group.clone());
        Some(effects)
    }

    /// Rehomes one durable group leaf as the main root of an empty dock space.
    pub(crate) fn apply_move_group_to_empty_space_for_commit(
        &mut self,
        source_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space {
            return None;
        }
        let Some(target_state) = self.spaces.get(target_space) else {
            return None;
        };
        if target_state.root.is_some() {
            return None;
        }
        if source_space == target_space {
            let CanonicalNodeOwner::Floating { floating, .. } = &source_owner else {
                return None;
            };
            if target_state.floating_roots.as_slice() != [floating.clone()]
                || !self
                    .floating_roots
                    .get(floating)
                    .is_some_and(|root| root.root == source_node)
            {
                return None;
            }
        } else if !target_state.floating_roots.is_empty() {
            return None;
        }
        let bind_central =
            target_state.central.keep_alive_when_empty && target_state.central.node.is_none();

        let Some(detach) = self.detach_group_leaf_for_commit(source_node, &source_owner) else {
            return None;
        };
        let target_owner = CanonicalNodeOwner::Main(target_space.clone());
        let target_leaf =
            self.bind_detached_group_leaf(source_group, detach.size_ref, None, target_owner);
        let target_state = self
            .spaces
            .get_mut(target_space)
            .expect("validated target space must remain present");
        target_state.root = Some(target_leaf);
        if bind_central {
            target_state.central.node = Some(target_leaf);
        }
        self.version = version;
        let mut effects = detach.effects;
        effects.record_created_node(target_leaf);
        Some(effects)
    }

    /// Moves one item into a target group, retiring an empty source leaf when necessary.
    pub(crate) fn apply_move_item_to_group_for_commit(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        insert_index: Option<usize>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space || target_owner_space != target_space {
            return None;
        }

        let Some(source_index) = self
            .groups
            .get(source_group)
            .and_then(|group| group.items.iter().position(|candidate| candidate == item))
        else {
            return None;
        };

        if source_group == target_group {
            let group = self
                .groups
                .get_mut(source_group)
                .expect("validated source group must remain present");
            let moved = group.items.remove(source_index);
            let mut insert_at = insert_index.unwrap_or(group.items.len());
            if insert_index.is_some_and(|index| index > source_index) {
                insert_at = insert_at.saturating_sub(1);
            }
            insert_at = insert_at.min(group.items.len());
            group.items.insert(insert_at, moved);
            group.selected = item.clone();
            group.mru.retain(|entry| entry != item);
            group.mru.insert(0, item.clone());
            self.version = version;
            return Some(GraphCommitEffects::default());
        }

        let source_empty = {
            let source = self
                .groups
                .get_mut(source_group)
                .expect("validated source group must remain present");
            source.items.remove(source_index);
            source.mru.retain(|entry| entry != item);
            if source.selected == *item {
                source.selected = source
                    .items
                    .first()
                    .cloned()
                    .unwrap_or_else(|| item.clone());
            }
            source.items.is_empty()
        };
        let target = self
            .groups
            .get_mut(target_group)
            .expect("validated target group must remain present");
        let insert_at = insert_index
            .unwrap_or(target.items.len())
            .min(target.items.len());
        target.items.insert(insert_at, item.clone());
        target.selected = item.clone();
        target.mru.retain(|entry| entry != item);
        target.mru.insert(0, item.clone());

        let mut effects = GraphCommitEffects::default();
        if source_empty {
            let Some(detach) = self.detach_group_leaf_for_commit(source_node, &source_owner) else {
                return None;
            };
            effects.merge(detach.effects);
            self.groups.remove(source_group);
            effects.record_retired_group(source_group.clone());
        }
        self.index
            .move_item_to_group(item, source_group, target_group, &target_owner);
        self.version = version;
        Some(effects)
    }

    /// Moves one item beside a target group, preserving a singleton source group while replacing
    /// its structural leaf or creating a durable group for a multi-item extraction.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_move_item_to_edge_for_commit(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        new_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        prepared_new_group: Option<CanonicalGroup>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) || ratio <= 0.0 || ratio >= 1.0 {
            return None;
        }
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space || target_owner_space != target_space {
            return None;
        }
        let Some(source_state) = self.groups.get(source_group) else {
            return None;
        };
        let source_len = source_state.items.len();
        let Some(source_index) = source_state
            .items
            .iter()
            .position(|candidate| candidate == item)
        else {
            return None;
        };

        if source_len == 1 {
            if new_group != source_group || prepared_new_group.is_some() {
                return None;
            }
            return self.apply_move_group_to_edge_for_commit(
                source_group,
                target_group,
                source_space,
                target_space,
                axis,
                insert_before,
                ratio,
                version,
            );
        }
        if new_group == source_group || self.groups.contains_key(new_group) {
            return None;
        }
        let Some(new_group_state) = prepared_new_group else {
            return None;
        };
        if !is_prepared_single_item_group(&new_group_state, item) {
            return None;
        }
        let Some(target_parent) = self.index.parent_of(target_node) else {
            return None;
        };
        if let Some(parent) = target_parent
            && !matches!(
                self.nodes.get(parent),
                Some(CanonicalNode::Branch { first, second, .. })
                    if *first == target_node || *second == target_node
            )
        {
            return None;
        }

        let source = self
            .groups
            .get_mut(source_group)
            .expect("validated source group must remain present");
        source.items.remove(source_index);
        source.mru.retain(|entry| entry != item);
        if source.selected == *item {
            source.selected = source
                .items
                .first()
                .cloned()
                .expect("a multi-item source group retains one item");
        }
        self.index.remove_item_membership(item, source_group);

        let effects = self
            .apply_split_group_for_commit(
                target_group,
                new_group,
                new_group_state,
                axis,
                insert_before,
                ratio,
                target_space,
                version,
            )
            .expect("validated item edge split must be applicable after local extraction");
        Some(effects)
    }

    /// Moves one item into an empty main root, either retaining a singleton group behind a new
    /// structural leaf or creating a durable group while retaining the remaining source group.
    pub(crate) fn apply_move_item_to_empty_space_for_commit(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        new_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        prepared_new_group: Option<CanonicalGroup>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space {
            return None;
        }
        let Some(source_state) = self.groups.get(source_group) else {
            return None;
        };
        let source_len = source_state.items.len();
        if !source_state.items.iter().any(|candidate| candidate == item) {
            return None;
        }
        let Some(target_state) = self.spaces.get(target_space) else {
            return None;
        };
        let target_is_empty = if source_space == target_space {
            target_state.root.is_none()
                && source_len == 1
                && self.index.parent_of(source_node) == Some(None)
                && matches!(
                    &source_owner,
                    CanonicalNodeOwner::Floating { floating, .. }
                        if target_state.floating_roots.len() == 1
                            && target_state.floating_roots.contains(floating)
                )
        } else {
            target_state.root.is_none() && target_state.floating_roots.is_empty()
        };
        if !target_is_empty {
            return None;
        }
        if source_len == 1 && new_group != source_group {
            return None;
        }
        if source_len > 1 && (new_group == source_group || self.groups.contains_key(new_group)) {
            return None;
        }
        let bind_central =
            target_state.central.keep_alive_when_empty && target_state.central.node.is_none();

        if source_len == 1 {
            if prepared_new_group.is_some() {
                return None;
            }
            return self.apply_move_group_to_empty_space_for_commit(
                source_group,
                source_space,
                target_space,
                version,
            );
        }
        let Some(new_group_state) = prepared_new_group else {
            return None;
        };
        if !is_prepared_single_item_group(&new_group_state, item) {
            return None;
        }

        let source = self
            .groups
            .get_mut(source_group)
            .expect("validated source group must remain present");
        source.items.retain(|candidate| candidate != item);
        source.mru.retain(|entry| entry != item);
        if source.selected == *item {
            source.selected = source
                .items
                .first()
                .cloned()
                .expect("a multi-item source group retains one item");
        }

        let new_leaf = self.nodes.insert(CanonicalNode::Group {
            group: new_group.clone(),
            size_ref: None,
        });
        self.groups.insert(new_group.clone(), new_group_state);
        let target_state = self
            .spaces
            .get_mut(target_space)
            .expect("validated target space must remain present");
        target_state.root = Some(new_leaf);
        if bind_central {
            target_state.central.node = Some(new_leaf);
        }
        self.index.remove_item_membership(item, source_group);
        self.index.insert_group(
            new_leaf,
            new_group,
            self.groups
                .get(new_group)
                .expect("new group inserted above"),
            None,
            CanonicalNodeOwner::Main(target_space.clone()),
        );
        self.version = version;
        let mut effects = GraphCommitEffects::default();
        effects.record_created_group(new_group.clone());
        effects.record_created_node(new_leaf);
        Some(effects)
    }

    /// Extracts one item into a new first-class floating root.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_float_item_in_window_for_commit(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        new_group: &DockGroupId,
        floating: &DockFloatingId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        retired_floating: Option<&DockFloatingId>,
        bounds: DockLayoutRect,
        z_order: i32,
        prepared_new_group: Option<CanonicalGroup>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        if !bounds.is_finite_with_non_negative_size()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || floating.as_str().is_empty()
            || new_group.as_str().is_empty()
            || self.floating_roots.contains_key(floating)
        {
            return None;
        }
        let Some(source_node) = self.index.node_for_group(source_group) else {
            return None;
        };
        let Some(source_owner) = self.index.owner_for_node(source_node).cloned() else {
            return None;
        };
        let source_owner_space = match &source_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if source_owner_space != source_space {
            return None;
        }
        let Some(source_state) = self.groups.get(source_group) else {
            return None;
        };
        let source_len = source_state.items.len();
        let actual_retired_floating = match &source_owner {
            CanonicalNodeOwner::Floating { floating, .. }
                if source_len == 1 && self.index.parent_of(source_node) == Some(None) =>
            {
                Some(floating)
            }
            CanonicalNodeOwner::Main(_) | CanonicalNodeOwner::Floating { .. } => None,
        };
        if actual_retired_floating != retired_floating {
            return None;
        }
        let Some(source_index) = source_state
            .items
            .iter()
            .position(|candidate| candidate == item)
        else {
            return None;
        };
        if (source_len == 1 && new_group != source_group)
            || (source_len > 1
                && (new_group == source_group || self.groups.contains_key(new_group)))
        {
            return None;
        }
        if source_len == 1 && prepared_new_group.is_some()
            || source_len > 1
                && prepared_new_group
                    .as_ref()
                    .is_none_or(|group| !is_prepared_single_item_group(group, item))
        {
            return None;
        }
        let Some(target_state) = self.spaces.get(target_space) else {
            return None;
        };
        let expected_z_order = target_state
            .floating_roots
            .iter()
            .filter(|candidate| {
                source_space != target_space || Some(*candidate) != retired_floating
            })
            .map(|candidate| {
                self.floating_roots
                    .get(candidate)
                    .expect("validated target space must reference a floating root")
                    .z_order
            })
            .max()
            .map_or(Some(0), |current| current.checked_add(1));
        if expected_z_order != Some(z_order) {
            return None;
        }

        let owner = CanonicalNodeOwner::Floating {
            space: target_space.clone(),
            floating: floating.clone(),
        };
        let (floating_root, mut effects) = if source_len == 1 {
            let Some(detach) = self.detach_group_leaf_for_commit(source_node, &source_owner) else {
                return None;
            };
            let new_leaf =
                self.bind_detached_group_leaf(source_group, detach.size_ref, None, owner.clone());
            let mut effects = detach.effects;
            effects.record_created_node(new_leaf);
            (new_leaf, effects)
        } else {
            let source = self
                .groups
                .get_mut(source_group)
                .expect("validated source group must remain present");
            source.items.remove(source_index);
            source.mru.retain(|entry| entry != item);
            if source.selected == *item {
                source.selected = source
                    .items
                    .first()
                    .cloned()
                    .expect("a multi-item source group retains one item");
            }
            self.index.remove_item_membership(item, source_group);

            let new_leaf = self.nodes.insert(CanonicalNode::Group {
                group: new_group.clone(),
                size_ref: None,
            });
            self.groups.insert(
                new_group.clone(),
                prepared_new_group.expect("validated multi-item extraction must carry a group"),
            );
            let mut effects = GraphCommitEffects::default();
            effects.record_created_group(new_group.clone());
            effects.record_created_node(new_leaf);
            (new_leaf, effects)
        };

        self.floating_roots.insert(
            floating.clone(),
            CanonicalFloatingRoot {
                space: target_space.clone(),
                root: floating_root,
                bounds,
                z_order,
            },
        );
        self.spaces
            .get_mut(target_space)
            .expect("validated target space must remain present")
            .floating_roots
            .push(floating.clone());
        if source_len > 1 {
            self.index.insert_group(
                floating_root,
                new_group,
                self.groups
                    .get(new_group)
                    .expect("new floating group inserted above"),
                None,
                owner,
            );
        }
        self.version = version;
        effects.record_created_floating(floating.clone());
        Some(effects)
    }

    /// Moves a complete floating subtree beside a target group and retires its floating owner.
    pub(crate) fn apply_move_floating_root_to_edge_for_commit(
        &mut self,
        floating: &DockFloatingId,
        source_space: &DockSpaceId,
        target_group: &DockGroupId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        owner_nodes: Vec<CanonicalNodeId>,
        version: CanonicalGraphVersion,
    ) -> Option<GraphCommitEffects> {
        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) || ratio <= 0.0 || ratio >= 1.0 {
            return None;
        }
        let Some(source_floating) = self.floating_roots.get(floating).cloned() else {
            return None;
        };
        if source_floating.space != *source_space {
            return None;
        }
        let Some(source_space_state) = self.spaces.get(source_space) else {
            return None;
        };
        if !source_space_state.floating_roots.contains(floating) {
            return None;
        }
        let Some(target_node) = self.index.node_for_group(target_group) else {
            return None;
        };
        let Some(target_owner) = self.index.owner_for_node(target_node).cloned() else {
            return None;
        };
        let target_owner_space = match &target_owner {
            CanonicalNodeOwner::Main(space) | CanonicalNodeOwner::Floating { space, .. } => space,
        };
        if target_owner_space != target_space {
            return None;
        }
        if matches!(
            &target_owner,
            CanonicalNodeOwner::Floating {
                floating: owner_floating,
                ..
            } if owner_floating == floating
        ) {
            return None;
        }
        if owner_nodes.first() != Some(&source_floating.root)
            || owner_nodes.iter().any(|node| {
                !matches!(
                    self.index.owner_for_node(*node),
                    Some(CanonicalNodeOwner::Floating {
                        space,
                        floating: owner_floating,
                    }) if space == source_space && owner_floating == floating
                )
            })
        {
            return None;
        }

        let target_parent = self
            .index
            .parent_of(target_node)
            .expect("validated target group must remain indexed");

        self.floating_roots.remove(floating);
        self.spaces
            .get_mut(source_space)
            .expect("validated source space must remain present")
            .floating_roots
            .retain(|candidate| candidate != floating);

        let branch_id = self.nodes.insert(if insert_before {
            CanonicalNode::Branch {
                axis,
                first: source_floating.root,
                second: target_node,
                ratio,
                size_ref: None,
            }
        } else {
            CanonicalNode::Branch {
                axis,
                first: target_node,
                second: source_floating.root,
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
                _ => unreachable!("validated target parent must reference the target node"),
            }
        } else {
            match &target_owner {
                CanonicalNodeOwner::Main(owner_space) => {
                    self.spaces
                        .get_mut(owner_space)
                        .expect("validated target space must remain present")
                        .root = Some(branch_id);
                }
                CanonicalNodeOwner::Floating { floating, .. } => {
                    self.floating_roots
                        .get_mut(floating)
                        .expect("validated target floating root must remain present")
                        .root = branch_id;
                }
            }
        }
        self.index
            .insert_branch(branch_id, target_parent, target_owner.clone());
        self.index.set_parent(target_node, Some(branch_id));
        self.index.set_parent(source_floating.root, Some(branch_id));
        self.index
            .update_owner_nodes(&self.nodes, &self.groups, owner_nodes, target_owner);
        self.version = version;
        let mut effects = GraphCommitEffects::default();
        effects.record_created_node(branch_id);
        effects.record_retired_floating(floating.clone());
        Some(effects)
    }

    /// Detaches a group payload while retiring its structural leaf.
    fn detach_group_leaf_for_commit(
        &mut self,
        source_node: CanonicalNodeId,
        source_owner: &CanonicalNodeOwner,
    ) -> Option<CanonicalLeafDetach> {
        let (source_group, source_size_ref) = match self.nodes.get(source_node)? {
            CanonicalNode::Group { group, size_ref } => (group.clone(), *size_ref),
            CanonicalNode::Branch { .. } => return None,
        };
        let Some(source_parent) = self.index.parent_of(source_node) else {
            return None;
        };
        let source_is_central = match source_owner {
            CanonicalNodeOwner::Main(space) => self
                .spaces
                .get(space)
                .is_some_and(|state| state.central.node == Some(source_node)),
            CanonicalNodeOwner::Floating { .. } => false,
        };

        let Some(parent) = source_parent else {
            match source_owner {
                CanonicalNodeOwner::Main(space) => {
                    let Some(state) = self.spaces.get_mut(space) else {
                        return None;
                    };
                    if state.root != Some(source_node) {
                        return None;
                    }
                    state.root = None;
                    if source_is_central {
                        state.central.node = None;
                    }
                }
                CanonicalNodeOwner::Floating { space, floating } => {
                    if !self
                        .floating_roots
                        .get(floating)
                        .is_some_and(|root| root.root == source_node)
                    {
                        return None;
                    }
                    self.floating_roots.remove(floating);
                    self.spaces
                        .get_mut(space)
                        .expect("validated source space must remain present")
                        .floating_roots
                        .retain(|candidate| candidate != floating);
                }
            }
            let mut effects = GraphCommitEffects::default();
            if let CanonicalNodeOwner::Floating { floating, .. } = source_owner {
                effects.record_retired_floating(floating.clone());
            }
            self.nodes
                .remove(source_node)
                .expect("validated source leaf must remain present");
            self.index.retire_group_leaf(source_node, &source_group);
            effects.record_retired_node(source_node);
            return Some(CanonicalLeafDetach {
                size_ref: source_size_ref,
                remap: None,
                effects,
            });
        };

        let (first, second, parent_size_ref, sibling) = match self.nodes.get(parent) {
            Some(CanonicalNode::Branch {
                first,
                second,
                size_ref,
                ..
            }) if *first == source_node => (*first, *second, *size_ref, *second),
            Some(CanonicalNode::Branch {
                first,
                second,
                size_ref,
                ..
            }) if *second == source_node => (*first, *second, *size_ref, *first),
            Some(CanonicalNode::Branch { .. }) | Some(CanonicalNode::Group { .. }) | None => {
                return None;
            }
        };
        if self.index.parent_of(sibling) != Some(Some(parent))
            || self.index.owner_for_node(parent) != Some(source_owner)
            || self.index.owner_for_node(sibling) != Some(source_owner)
        {
            return None;
        }
        let promotion =
            plan_sibling_promotion(parent, first, second, sibling, parent_size_ref).ok()?;
        let mut promoted = self.nodes.get(sibling)?.clone();
        match &mut promoted {
            CanonicalNode::Branch { size_ref, .. } | CanonicalNode::Group { size_ref, .. } => {
                *size_ref = promotion.preserved_size_ref;
            }
        }

        *self
            .nodes
            .get_mut(parent)
            .expect("validated source parent must remain present") = promoted.clone();
        self.nodes
            .remove(sibling)
            .expect("validated sibling must remain present");
        self.nodes
            .remove(source_node)
            .expect("validated source leaf must remain present");
        self.index.promote_sibling_into_parent(
            parent,
            source_node,
            &source_group,
            sibling,
            &promoted,
        );

        if let CanonicalNodeOwner::Main(space) = source_owner {
            let state = self
                .spaces
                .get_mut(space)
                .expect("validated source space must remain present");
            if state.central.node == Some(sibling) {
                state.central.node = Some(parent);
            } else if source_is_central {
                state.central.node =
                    matches!(promoted, CanonicalNode::Group { .. }).then_some(parent);
            }
        }
        let mut effects = GraphCommitEffects::default();
        for retired in promotion.retired {
            effects.record_retired_node(retired);
        }
        effects.record_identity_remap(promotion.remap);
        Some(CanonicalLeafDetach {
            size_ref: source_size_ref,
            remap: Some(promotion.remap),
            effects,
        })
    }

    fn bind_detached_group_leaf(
        &mut self,
        group: &DockGroupId,
        size_ref: Option<CanonicalSizeRef>,
        parent: Option<CanonicalNodeId>,
        owner: CanonicalNodeOwner,
    ) -> CanonicalNodeId {
        let node = self.nodes.insert(CanonicalNode::Group {
            group: group.clone(),
            size_ref,
        });
        self.index.bind_existing_group(
            node,
            group,
            self.groups
                .get(group)
                .expect("detached durable group must remain present"),
            parent,
            owner,
        );
        node
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
            groups: self.groups.clone().into_iter().collect(),
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
            floating_roots: HashMap::new(),
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
