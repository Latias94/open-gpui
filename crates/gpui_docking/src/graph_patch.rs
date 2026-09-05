#![allow(dead_code)]

use crate::binary_layout_policy::StructuralIdentityRemap;
use crate::graph_assembly::{CanonicalDockGraph, CanonicalGroup};
use crate::{
    CanonicalGraphVersion, CanonicalNodeId, DockFloatingId, DockGroupId, DockItemId,
    DockLayoutRect, DockSpaceId, SplitAxis,
};
use arrayvec::ArrayVec;
use smallvec::SmallVec;
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

/// Typed revision observed by a prepared graph patch.
///
/// The authority epoch is intentionally kept alongside the individual state revisions. A local
/// revision can repeat after a graph authority is replaced, while the epoch makes that ABA case
/// impossible for a prepared command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GraphRevision {
    authority_epoch: u64,
    topology: u64,
    allocator: u64,
    selection: u64,
    interaction: u64,
}

impl GraphRevision {
    pub(crate) const fn new(
        authority_epoch: u64,
        topology: u64,
        selection: u64,
        interaction: u64,
    ) -> Self {
        Self {
            authority_epoch,
            topology,
            allocator: topology,
            selection,
            interaction,
        }
    }

    pub(crate) fn from_version(version: CanonicalGraphVersion) -> Self {
        Self {
            authority_epoch: version.authority_epoch(),
            topology: version.topology_revision(),
            allocator: version.allocator_revision(),
            selection: version.selection_revision(),
            interaction: version.interaction_revision(),
        }
    }

    pub(crate) const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub(crate) const fn topology(self) -> u64 {
        self.topology
    }

    pub(crate) const fn allocator(self) -> u64 {
        self.allocator
    }

    pub(crate) const fn selection(self) -> u64 {
        self.selection
    }

    pub(crate) const fn interaction(self) -> u64 {
        self.interaction
    }

    pub(crate) const fn with_topology(self, topology: u64) -> Self {
        Self { topology, ..self }
    }

    pub(crate) const fn with_allocator(self, allocator: u64) -> Self {
        Self { allocator, ..self }
    }

    pub(crate) const fn with_selection(self, selection: u64) -> Self {
        Self { selection, ..self }
    }

    pub(crate) const fn with_interaction(self, interaction: u64) -> Self {
        Self {
            interaction,
            ..self
        }
    }
}

/// Opaque identity allocated for one prepared graph patch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct GraphCommitId {
    authority_epoch: u64,
    transaction_epoch: u64,
    generation: u64,
}

impl GraphCommitId {
    pub(crate) const fn new(authority_epoch: u64, transaction_epoch: u64, generation: u64) -> Self {
        Self {
            authority_epoch,
            transaction_epoch,
            generation,
        }
    }

    pub(crate) const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub(crate) const fn transaction_epoch(self) -> u64 {
        self.transaction_epoch
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }
}

impl fmt::Display for GraphCommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.authority_epoch, self.transaction_epoch, self.generation
        )
    }
}

/// Small immutable group fact captured by a read set.
///
/// This is deliberately limited to the touched group's membership and selection. It is not a
/// graph snapshot and therefore does not make preparation cost proportional to unrelated nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphGroupRead {
    pub(crate) group: DockGroupId,
    pub(crate) space: DockSpaceId,
    pub(crate) floating: Option<DockFloatingId>,
    pub(crate) is_owner_root: bool,
    pub(crate) items: Vec<DockItemId>,
    pub(crate) selected: DockItemId,
    pub(crate) mru: Vec<DockItemId>,
}

/// Small immutable floating-root fact captured by a prepared structural patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphFloatingRootRead {
    pub(crate) floating: DockFloatingId,
    pub(crate) space: DockSpaceId,
    pub(crate) bounds: GraphFloatingBounds,
    pub(crate) z_order: i32,
}

/// Small immutable dock-space fact captured by a prepared structural operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphSpaceRead {
    pub(crate) space: DockSpaceId,
    pub(crate) has_main_root: bool,
    pub(crate) floating_roots: Vec<DockFloatingId>,
    pub(crate) floating_z_orders: Vec<(DockFloatingId, i32)>,
    pub(crate) central_group: Option<DockGroupId>,
    pub(crate) keep_central_alive_when_empty: bool,
    pub(crate) passthrough_when_empty: bool,
}

impl GraphSpaceRead {
    pub(crate) fn next_floating_z_order_excluding(
        &self,
        excluded: Option<&DockFloatingId>,
    ) -> Option<i32> {
        self.floating_z_orders
            .iter()
            .filter(|(floating, _)| Some(floating) != excluded)
            .map(|(_, z_order)| *z_order)
            .max()
            .map_or(Some(0), |z_order| z_order.checked_add(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GraphSplitRatio(u32);

impl GraphSplitRatio {
    pub(crate) fn new(value: f32) -> Option<Self> {
        (value.is_finite() && value > 0.0 && value < 1.0).then_some(Self(value.to_bits()))
    }

    pub(crate) fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Finite positive floating bounds frozen into an equality-safe prepared patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GraphFloatingBounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl GraphFloatingBounds {
    pub(crate) fn new(bounds: DockLayoutRect) -> Option<Self> {
        (bounds.is_finite_with_non_negative_size() && bounds.width > 0.0 && bounds.height > 0.0)
            .then_some(Self {
                x: bounds.x.to_bits(),
                y: bounds.y.to_bits(),
                width: bounds.width.to_bits(),
                height: bounds.height.to_bits(),
            })
    }

    pub(crate) fn get(self) -> DockLayoutRect {
        DockLayoutRect {
            x: f32::from_bits(self.x),
            y: f32::from_bits(self.y),
            width: f32::from_bits(self.width),
            height: f32::from_bits(self.height),
        }
    }
}

/// Resolves durable selection after closing one item from a non-empty group.
pub(crate) fn selected_item_after_close(
    items: &[DockItemId],
    selected: &DockItemId,
    mru: &[DockItemId],
    closing: &DockItemId,
) -> Option<DockItemId> {
    if !items.contains(closing) || items.len() <= 1 {
        return None;
    }
    if selected != closing && items.contains(selected) {
        return Some(selected.clone());
    }
    mru.iter()
        .find(|candidate| *candidate != closing && items.contains(candidate))
        .or_else(|| items.iter().find(|candidate| *candidate != closing))
        .cloned()
}

/// Returns whether selecting an item would change the transient MRU ordering.
pub(crate) fn mru_changes_after_select(mru: &[DockItemId], item: &DockItemId) -> bool {
    mru.first() != Some(item) || mru.iter().skip(1).any(|candidate| candidate == item)
}

/// Commit-time headroom required from one durable group's mutable vectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphGroupCapacityRequirement {
    pub(crate) group: DockGroupId,
    pub(crate) additional_items: usize,
    pub(crate) additional_mru: usize,
}

/// Commit-time headroom required from one dock-space floating-root list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphSpaceCapacityRequirement {
    pub(crate) space: DockSpaceId,
    pub(crate) additional_floating_roots: usize,
}

/// Proof that live canonical collections touched by a prepared operation have spare capacity.
///
/// Allocator revision and the ordinary read set provide identity/liveness checks. The additional
/// counts prove remaining headroom against current lengths, while `required_node_key_capacity`
/// proves that the secondary indexes can address every slot the reserved node map may allocate.
/// Owned scratch and new payload values live in the sealed mutation carried by the patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphCapacityProof {
    pub(crate) commit_id: GraphCommitId,
    pub(crate) authority_epoch: u64,
    pub(crate) allocator_revision: u64,
    pub(crate) required_node_key_capacity: usize,
    pub(crate) additional_nodes: usize,
    pub(crate) additional_groups: usize,
    pub(crate) additional_floatings: usize,
    pub(crate) additional_group_index: usize,
    pub(crate) additional_item_index: usize,
    pub(crate) additional_floating_item_index: usize,
    pub(crate) group_vectors: SmallVec<[GraphGroupCapacityRequirement; 3]>,
    pub(crate) space_vectors: SmallVec<[GraphSpaceCapacityRequirement; 2]>,
}

impl GraphCapacityProof {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        commit_id: GraphCommitId,
        authority_epoch: u64,
        allocator_revision: u64,
        required_node_key_capacity: usize,
        additional_nodes: usize,
        additional_groups: usize,
        additional_floatings: usize,
        additional_group_index: usize,
        additional_item_index: usize,
        additional_floating_item_index: usize,
        group_vectors: SmallVec<[GraphGroupCapacityRequirement; 3]>,
        space_vectors: SmallVec<[GraphSpaceCapacityRequirement; 2]>,
    ) -> Self {
        Self {
            commit_id,
            authority_epoch,
            allocator_revision,
            required_node_key_capacity,
            additional_nodes,
            additional_groups,
            additional_floatings,
            additional_group_index,
            additional_item_index,
            additional_floating_item_index,
            group_vectors,
            space_vectors,
        }
    }
}

/// Declared revision domains that a patch depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphReadDomains {
    pub(crate) topology: bool,
    pub(crate) allocator: bool,
    pub(crate) selection: bool,
    pub(crate) interaction: bool,
}

impl GraphReadDomains {
    pub(crate) const fn select_tab() -> Self {
        Self {
            topology: true,
            allocator: false,
            // Selection is checked locally on the touched group, so an unrelated group can
            // change selection without invalidating this patch.
            selection: false,
            interaction: false,
        }
    }
}

/// Explicit precondition set captured while preparing a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphReadSet {
    base_revision: GraphRevision,
    domains: GraphReadDomains,
    group: Option<GraphGroupRead>,
    additional_groups: Vec<GraphGroupRead>,
    floating_roots: Vec<GraphFloatingRootRead>,
    spaces: Vec<GraphSpaceRead>,
    group_selection_sensitive: bool,
    group_mru_sensitive: bool,
}

impl GraphReadSet {
    pub(crate) fn for_select_tab(base_revision: GraphRevision, group: GraphGroupRead) -> Self {
        Self {
            base_revision,
            domains: GraphReadDomains::select_tab(),
            group: Some(group),
            additional_groups: Vec::new(),
            floating_roots: Vec::new(),
            spaces: Vec::new(),
            group_selection_sensitive: true,
            group_mru_sensitive: true,
        }
    }

    pub(crate) fn for_groups(
        base_revision: GraphRevision,
        groups: impl IntoIterator<Item = GraphGroupRead>,
    ) -> Self {
        let mut groups = groups.into_iter();
        let group = groups
            .next()
            .expect("a graph patch must read at least one group");
        Self {
            base_revision,
            domains: GraphReadDomains {
                topology: true,
                allocator: true,
                selection: false,
                interaction: false,
            },
            group: Some(group),
            additional_groups: groups.collect(),
            floating_roots: Vec::new(),
            spaces: Vec::new(),
            group_selection_sensitive: false,
            group_mru_sensitive: false,
        }
    }

    pub(crate) fn for_floating_and_groups(
        base_revision: GraphRevision,
        floating: GraphFloatingRootRead,
        groups: impl IntoIterator<Item = GraphGroupRead>,
    ) -> Self {
        let mut read_set = Self::for_groups(base_revision, groups);
        read_set.floating_roots.push(floating);
        read_set
    }

    pub(crate) fn for_floating(
        base_revision: GraphRevision,
        floating: GraphFloatingRootRead,
    ) -> Self {
        Self {
            base_revision,
            domains: GraphReadDomains {
                topology: true,
                allocator: false,
                selection: false,
                interaction: false,
            },
            group: None,
            additional_groups: Vec::new(),
            floating_roots: vec![floating],
            spaces: Vec::new(),
            group_selection_sensitive: false,
            group_mru_sensitive: false,
        }
    }

    pub(crate) fn for_floating_and_space(
        base_revision: GraphRevision,
        floating: GraphFloatingRootRead,
        space: GraphSpaceRead,
    ) -> Self {
        let mut read_set = Self::for_floating(base_revision, floating);
        read_set.spaces.push(space);
        read_set
    }

    pub(crate) fn for_group_and_space(
        base_revision: GraphRevision,
        group: GraphGroupRead,
        target_space: GraphSpaceRead,
    ) -> Self {
        let mut read_set = Self::for_groups(base_revision, [group]);
        read_set.spaces.push(target_space);
        read_set
    }

    pub(crate) fn for_group_and_space_state_sensitive(
        base_revision: GraphRevision,
        group: GraphGroupRead,
        target_space: GraphSpaceRead,
    ) -> Self {
        let mut read_set = Self::for_group_state_sensitive_operation(base_revision, [group]);
        read_set.spaces.push(target_space);
        read_set
    }

    pub(crate) fn for_group_state_sensitive_operation(
        base_revision: GraphRevision,
        groups: impl IntoIterator<Item = GraphGroupRead>,
    ) -> Self {
        let mut read_set = Self::for_groups(base_revision, groups);
        read_set.group_selection_sensitive = true;
        read_set.group_mru_sensitive = true;
        read_set
    }

    pub(crate) const fn base_revision(&self) -> GraphRevision {
        self.base_revision
    }

    pub(crate) const fn domains(&self) -> GraphReadDomains {
        self.domains
    }

    pub(crate) fn group(&self) -> Option<&GraphGroupRead> {
        self.group.as_ref()
    }

    pub(crate) fn groups(&self) -> impl Iterator<Item = &GraphGroupRead> {
        self.group.iter().chain(self.additional_groups.iter())
    }

    pub(crate) fn floating_roots(&self) -> impl Iterator<Item = &GraphFloatingRootRead> {
        self.floating_roots.iter()
    }

    pub(crate) fn spaces(&self) -> impl Iterator<Item = &GraphSpaceRead> {
        self.spaces.iter()
    }
}

/// Symbolic operation recorded by a prepared patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphPatchOperation {
    /// Select an existing item in an existing durable group.
    SelectTab {
        group: DockGroupId,
        item: DockItemId,
    },
    /// Closes one item and retires its durable group when it becomes empty.
    CloseItem {
        group: DockGroupId,
        item: DockItemId,
        space: DockSpaceId,
    },
    /// Inserts a newly created group beside an existing group in its current root.
    SplitGroup {
        target_group: DockGroupId,
        new_group: DockGroupId,
        items: Vec<DockItemId>,
        selected: DockItemId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: GraphSplitRatio,
        space: DockSpaceId,
    },
    /// Moves an existing durable group beside a target group while preserving its identity.
    MoveGroupToEdge {
        source_group: DockGroupId,
        target_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: GraphSplitRatio,
    },
    /// Merges every target tab into the moved source group and preserves its durable identity.
    MoveGroupToCenter {
        source_group: DockGroupId,
        target_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
    },
    /// Moves one durable group leaf into an empty main dock-space root.
    MoveGroupToEmptySpace {
        source_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
    },
    /// Moves one item into a target group, optionally at an explicit tab insertion index.
    MoveItemToGroup {
        item: DockItemId,
        source_group: DockGroupId,
        target_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        insert_index: Option<usize>,
    },
    /// Moves one item beside a target group, preserving or creating its durable group leaf.
    MoveItemToEdge {
        item: DockItemId,
        source_group: DockGroupId,
        new_group: DockGroupId,
        target_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: GraphSplitRatio,
    },
    /// Moves one item into an empty dock-space root, preserving or creating its group leaf.
    MoveItemToEmptySpace {
        item: DockItemId,
        source_group: DockGroupId,
        new_group: DockGroupId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
    },
    /// Extracts one item into a new first-class floating root.
    FloatItemInWindow {
        item: DockItemId,
        source_group: DockGroupId,
        new_group: DockGroupId,
        floating: DockFloatingId,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        retired_floating: Option<DockFloatingId>,
        bounds: GraphFloatingBounds,
        z_order: i32,
    },
    /// Updates the durable bounds of one floating root without changing its subtree.
    SetFloatingBounds {
        floating: DockFloatingId,
        space: DockSpaceId,
        bounds: GraphFloatingBounds,
    },
    /// Raises one floating root above the other roots in its dock space.
    RaiseFloating {
        floating: DockFloatingId,
        space: DockSpaceId,
        z_order: i32,
    },
    /// Moves one complete floating root beside a target group and retires its floating chrome.
    MoveFloatingRootToEdge {
        floating: DockFloatingId,
        source_space: DockSpaceId,
        target_group: DockGroupId,
        target_space: DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: GraphSplitRatio,
    },
}

/// Allocation-complete mutation payload paired with one symbolic operation.
///
/// The semantic operation is retained in receipts. This linear value owns every payload or
/// traversal buffer that could otherwise allocate after the canonical graph has received its
/// first write.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GraphPreparedMutation {
    SelectTab {
        selection_changed: bool,
        interaction_changed: bool,
    },
    CloseItem {
        next_selected: Option<DockItemId>,
        selection_changed: bool,
        interaction_changed: bool,
    },
    SplitGroup {
        new_group: CanonicalGroup,
    },
    MoveGroupToEdge,
    MoveGroupToCenter,
    MoveGroupToEmptySpace,
    MoveItemToGroup,
    MoveItemToEdge {
        new_group: Option<CanonicalGroup>,
    },
    MoveItemToEmptySpace {
        new_group: Option<CanonicalGroup>,
    },
    FloatItemInWindow {
        new_group: Option<CanonicalGroup>,
    },
    SetFloatingBounds,
    RaiseFloating,
    MoveFloatingRootToEdge {
        owner_nodes: Vec<CanonicalNodeId>,
    },
}

impl GraphPreparedMutation {
    pub(crate) fn matches_operation(&self, operation: &GraphPatchOperation) -> bool {
        matches!(
            (operation, self),
            (
                GraphPatchOperation::SelectTab { .. },
                Self::SelectTab { .. }
            ) | (
                GraphPatchOperation::CloseItem { .. },
                Self::CloseItem { .. }
            ) | (
                GraphPatchOperation::SplitGroup { .. },
                Self::SplitGroup { .. }
            ) | (
                GraphPatchOperation::MoveGroupToEdge { .. },
                Self::MoveGroupToEdge
            ) | (
                GraphPatchOperation::MoveGroupToCenter { .. },
                Self::MoveGroupToCenter
            ) | (
                GraphPatchOperation::MoveGroupToEmptySpace { .. },
                Self::MoveGroupToEmptySpace
            ) | (
                GraphPatchOperation::MoveItemToGroup { .. },
                Self::MoveItemToGroup
            ) | (
                GraphPatchOperation::MoveItemToEdge { .. },
                Self::MoveItemToEdge { .. }
            ) | (
                GraphPatchOperation::MoveItemToEmptySpace { .. },
                Self::MoveItemToEmptySpace { .. }
            ) | (
                GraphPatchOperation::FloatItemInWindow { .. },
                Self::FloatItemInWindow { .. }
            ) | (
                GraphPatchOperation::SetFloatingBounds { .. },
                Self::SetFloatingBounds
            ) | (
                GraphPatchOperation::RaiseFloating { .. },
                Self::RaiseFloating
            ) | (
                GraphPatchOperation::MoveFloatingRootToEdge { .. },
                Self::MoveFloatingRootToEdge { .. }
            )
        )
    }
}

/// A local symbolic patch and its declared read set.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GraphPatch {
    operation: Arc<GraphPatchOperation>,
    read_set: GraphReadSet,
}

impl GraphPatch {
    pub(crate) fn new(operation: GraphPatchOperation, read_set: GraphReadSet) -> Self {
        Self {
            operation: Arc::new(operation),
            read_set,
        }
    }

    pub(crate) fn operation(&self) -> &GraphPatchOperation {
        self.operation.as_ref()
    }

    pub(crate) fn read_set(&self) -> &GraphReadSet {
        &self.read_set
    }
}

/// A prepared patch is a linear proof and is intentionally not cloneable.
///
/// Consuming this value is the only way to submit the patch. Rust ownership therefore prevents a
/// caller from applying the same prepared value twice; replay uses the commit id and retained
/// receipt instead.
#[must_use = "a prepared graph patch must be committed or explicitly discarded"]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PreparedGraphPatch {
    commit_id: GraphCommitId,
    patch: GraphPatch,
    mutation: GraphPreparedMutation,
    capacity_proof: Option<GraphCapacityProof>,
    receipt_reservation: Box<GraphReceiptReservation>,
}

impl PreparedGraphPatch {
    pub(crate) fn new(
        commit_id: GraphCommitId,
        patch: GraphPatch,
        mutation: GraphPreparedMutation,
    ) -> Self {
        debug_assert!(mutation.matches_operation(patch.operation()));
        Self {
            commit_id,
            patch,
            mutation,
            capacity_proof: None,
            receipt_reservation: Box::new(GraphReceiptReservation::new(commit_id)),
        }
    }

    pub(crate) fn commit_id(&self) -> GraphCommitId {
        self.commit_id
    }

    pub(crate) fn operation(&self) -> &GraphPatchOperation {
        self.patch.operation()
    }

    pub(crate) fn read_set(&self) -> &GraphReadSet {
        self.patch.read_set()
    }

    pub(crate) fn mutation(&self) -> &GraphPreparedMutation {
        &self.mutation
    }

    pub(crate) fn with_capacity_proof(mut self, proof: GraphCapacityProof) -> Self {
        debug_assert!(self.capacity_proof.is_none());
        debug_assert_eq!(proof.commit_id, self.commit_id);
        self.capacity_proof = Some(proof);
        self
    }

    pub(crate) fn capacity_proof(&self) -> Option<&GraphCapacityProof> {
        self.capacity_proof.as_ref()
    }

    pub(crate) fn has_receipt_reservation_for(&self, commit_id: GraphCommitId) -> bool {
        self.receipt_reservation.commit_id() == commit_id
    }

    pub(crate) fn into_commit_parts(
        self,
    ) -> (
        Arc<GraphPatchOperation>,
        GraphPreparedMutation,
        Option<GraphCapacityProof>,
        Box<GraphReceiptReservation>,
    ) {
        (
            self.patch.operation,
            self.mutation,
            self.capacity_proof,
            self.receipt_reservation,
        )
    }
}

/// Preparation result for the first local operation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GraphPrepareOutcome {
    Prepared(PreparedGraphPatch),
    NoOp(GraphNoOpReceipt),
}

/// A typed no-op observation produced before any patch is allocated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphNoOpReceipt {
    revision: GraphRevision,
    operation: GraphPatchOperation,
}

impl GraphNoOpReceipt {
    pub(crate) const fn new(revision: GraphRevision, operation: GraphPatchOperation) -> Self {
        Self {
            revision,
            operation,
        }
    }

    pub(crate) const fn revision(&self) -> GraphRevision {
        self.revision
    }

    pub(crate) fn operation(&self) -> &GraphPatchOperation {
        &self.operation
    }
}

/// Reason a prepared patch can no longer be committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GraphSupersededReason {
    AuthorityChanged,
    TransactionOwnerChanged,
    CapacityProofMissing,
    CapacityProofMismatch,
    AllocatorChanged,
    TopologyChanged,
    SelectionChanged,
    InteractionChanged,
    GroupMissing,
    GroupIdentityChanged,
    GroupOwnerChanged,
    GroupMembershipChanged,
    GroupSelectionChanged,
    FloatingMissing,
    FloatingOwnerChanged,
    SpaceMissing,
    SpaceTopologyChanged,
}

/// Receipt returned when a read-set check fails before mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphSupersededReceipt {
    commit_id: GraphCommitId,
    expected_revision: GraphRevision,
    actual_revision: GraphRevision,
    reason: GraphSupersededReason,
}

impl GraphSupersededReceipt {
    pub(crate) const fn new(
        commit_id: GraphCommitId,
        expected_revision: GraphRevision,
        actual_revision: GraphRevision,
        reason: GraphSupersededReason,
    ) -> Self {
        Self {
            commit_id,
            expected_revision,
            actual_revision,
            reason,
        }
    }

    pub(crate) const fn commit_id(&self) -> GraphCommitId {
        self.commit_id
    }

    pub(crate) const fn expected_revision(&self) -> GraphRevision {
        self.expected_revision
    }

    pub(crate) const fn actual_revision(&self) -> GraphRevision {
        self.actual_revision
    }

    pub(crate) const fn reason(&self) -> &GraphSupersededReason {
        &self.reason
    }
}

/// Heap-backed receipt cell reserved by a prepared patch.
///
/// The generation table retains only pointer-sized address slots. Abandoned or superseded patches
/// drop this cell, while a successful commit fills and transfers it without allocating.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GraphReceiptReservation {
    commit_id: GraphCommitId,
    receipt: Option<GraphCommitReceipt>,
}

impl GraphReceiptReservation {
    pub(crate) const fn new(commit_id: GraphCommitId) -> Self {
        Self {
            commit_id,
            receipt: None,
        }
    }

    pub(crate) const fn commit_id(&self) -> GraphCommitId {
        self.commit_id
    }

    pub(crate) fn install(&mut self, receipt: GraphCommitReceipt) {
        debug_assert!(self.receipt.is_none());
        debug_assert_eq!(receipt.commit_id(), self.commit_id);
        self.receipt = Some(receipt);
    }

    pub(crate) fn receipt(&self) -> Option<&GraphCommitReceipt> {
        self.receipt.as_ref()
    }
}

/// Identity-level effects emitted by one canonical graph commit.
///
/// Each operation has a small fixed structural footprint. Fixed-capacity storage keeps receipt
/// cloning and installation allocation-free for every currently supported patch kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GraphCommitEffects {
    created_nodes: ArrayVec<CanonicalNodeId, 4>,
    retired_nodes: ArrayVec<CanonicalNodeId, 4>,
    identity_remaps: ArrayVec<StructuralIdentityRemap, 2>,
    created_groups: ArrayVec<DockGroupId, 2>,
    retired_groups: ArrayVec<DockGroupId, 2>,
    created_floatings: ArrayVec<DockFloatingId, 2>,
    retired_floatings: ArrayVec<DockFloatingId, 2>,
}

impl GraphCommitEffects {
    pub(crate) fn record_created_node(&mut self, node: CanonicalNodeId) {
        self.created_nodes.push(node);
    }

    pub(crate) fn record_retired_node(&mut self, node: CanonicalNodeId) {
        if !self.retired_nodes.contains(&node) {
            self.retired_nodes.push(node);
        }
    }

    pub(crate) fn record_identity_remap(&mut self, remap: StructuralIdentityRemap) {
        if !self.identity_remaps.contains(&remap) {
            self.identity_remaps.push(remap);
        }
    }

    pub(crate) fn record_created_group(&mut self, group: DockGroupId) {
        self.created_groups.push(group);
    }

    pub(crate) fn record_retired_group(&mut self, group: DockGroupId) {
        self.retired_groups.push(group);
    }

    pub(crate) fn record_created_floating(&mut self, floating: DockFloatingId) {
        self.created_floatings.push(floating);
    }

    pub(crate) fn record_retired_floating(&mut self, floating: DockFloatingId) {
        if !self.retired_floatings.contains(&floating) {
            self.retired_floatings.push(floating);
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        for node in other.created_nodes {
            self.record_created_node(node);
        }
        for node in other.retired_nodes {
            self.record_retired_node(node);
        }
        for remap in other.identity_remaps {
            self.record_identity_remap(remap);
        }
        for group in other.created_groups {
            self.record_created_group(group);
        }
        for group in other.retired_groups {
            self.record_retired_group(group);
        }
        for floating in other.created_floatings {
            self.record_created_floating(floating);
        }
        for floating in other.retired_floatings {
            self.record_retired_floating(floating);
        }
    }

    pub(crate) fn created_nodes(&self) -> &[CanonicalNodeId] {
        &self.created_nodes
    }

    pub(crate) fn retired_nodes(&self) -> &[CanonicalNodeId] {
        &self.retired_nodes
    }

    pub(crate) fn identity_remaps(&self) -> &[StructuralIdentityRemap] {
        &self.identity_remaps
    }

    pub(crate) fn created_groups(&self) -> &[DockGroupId] {
        &self.created_groups
    }

    pub(crate) fn retired_groups(&self) -> &[DockGroupId] {
        &self.retired_groups
    }

    pub(crate) fn created_floatings(&self) -> &[DockFloatingId] {
        &self.created_floatings
    }

    pub(crate) fn retired_floatings(&self) -> &[DockFloatingId] {
        &self.retired_floatings
    }
}

/// Result returned by the infallible graph write boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphAppliedPatch {
    after: GraphRevision,
    effects: GraphCommitEffects,
}

impl GraphAppliedPatch {
    pub(crate) const fn new(after: GraphRevision, effects: GraphCommitEffects) -> Self {
        Self { after, effects }
    }

    pub(crate) const fn after(&self) -> GraphRevision {
        self.after
    }

    pub(crate) fn into_effects(self) -> GraphCommitEffects {
        self.effects
    }
}

/// Receipt retained for idempotent commit replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphCommitReceipt {
    commit_id: GraphCommitId,
    before: GraphRevision,
    after: GraphRevision,
    operation: Arc<GraphPatchOperation>,
    effects: GraphCommitEffects,
}

impl GraphCommitReceipt {
    pub(crate) fn new(
        commit_id: GraphCommitId,
        before: GraphRevision,
        after: GraphRevision,
        operation: Arc<GraphPatchOperation>,
        effects: GraphCommitEffects,
    ) -> Self {
        Self {
            commit_id,
            before,
            after,
            operation,
            effects,
        }
    }

    pub(crate) const fn commit_id(&self) -> GraphCommitId {
        self.commit_id
    }

    pub(crate) const fn before(&self) -> GraphRevision {
        self.before
    }

    pub(crate) const fn after(&self) -> GraphRevision {
        self.after
    }

    pub(crate) fn operation(&self) -> &GraphPatchOperation {
        self.operation.as_ref()
    }

    pub(crate) fn effects(&self) -> &GraphCommitEffects {
        &self.effects
    }
}

/// Commit observation. The `Superseded` branch is produced before the target receives any write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphCommitOutcome {
    Committed(GraphCommitReceipt),
    AlreadyCommitted(GraphCommitReceipt),
    Superseded(GraphSupersededReceipt),
}

/// Read-only view required by patch preparation and precondition checks.
pub(crate) trait GraphReadView {
    fn graph_revision(&self) -> GraphRevision;

    fn read_group(&self, group: &DockGroupId) -> Option<GraphGroupRead>;

    fn validate_group_read(
        &self,
        expected: &GraphGroupRead,
        selection_sensitive: bool,
        mru_sensitive: bool,
    ) -> Result<(), GraphSupersededReason> {
        let Some(group) = self.read_group(&expected.group) else {
            return Err(GraphSupersededReason::GroupMissing);
        };
        if group.group != expected.group {
            return Err(GraphSupersededReason::GroupIdentityChanged);
        }
        if group.space != expected.space
            || group.floating != expected.floating
            || group.is_owner_root != expected.is_owner_root
        {
            return Err(GraphSupersededReason::GroupOwnerChanged);
        }
        if group.items != expected.items {
            return Err(GraphSupersededReason::GroupMembershipChanged);
        }
        if selection_sensitive && group.selected != expected.selected {
            return Err(GraphSupersededReason::GroupSelectionChanged);
        }
        if mru_sensitive && group.mru != expected.mru {
            return Err(GraphSupersededReason::GroupSelectionChanged);
        }
        Ok(())
    }

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        let _ = item;
        None
    }

    fn read_floating_root(&self, floating: &DockFloatingId) -> Option<GraphFloatingRootRead> {
        let _ = floating;
        None
    }

    fn read_floating_subtree_nodes(
        &self,
        floating: &DockFloatingId,
    ) -> Option<Vec<CanonicalNodeId>> {
        let _ = floating;
        None
    }

    fn validate_floating_read(
        &self,
        expected: &GraphFloatingRootRead,
    ) -> Result<(), GraphSupersededReason> {
        let Some(floating) = self.read_floating_root(&expected.floating) else {
            return Err(GraphSupersededReason::FloatingMissing);
        };
        if floating.space != expected.space {
            return Err(GraphSupersededReason::FloatingOwnerChanged);
        }
        if floating.bounds != expected.bounds || floating.z_order != expected.z_order {
            return Err(GraphSupersededReason::FloatingOwnerChanged);
        }
        Ok(())
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<GraphSpaceRead> {
        let _ = space;
        None
    }

    fn validate_space_read(&self, expected: &GraphSpaceRead) -> Result<(), GraphSupersededReason> {
        let Some(space) = self.read_space(&expected.space) else {
            return Err(GraphSupersededReason::SpaceMissing);
        };
        if &space != expected {
            return Err(GraphSupersededReason::SpaceTopologyChanged);
        }
        Ok(())
    }
}

/// Infallible write boundary for a proven symbolic patch.
///
/// The trait intentionally has no `Result` return. All domain validation belongs to preparation
/// and read-set revalidation; once either method is called, the target has promised that the local
/// write is structurally valid.
pub(crate) trait GraphPatchTarget: GraphReadView {
    fn validate_capacity_proof(
        &self,
        commit_id: GraphCommitId,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
        proof: Option<&GraphCapacityProof>,
    ) -> Result<(), GraphSupersededReason>;

    fn apply_prepared_patch(
        &mut self,
        operation: &GraphPatchOperation,
        mutation: GraphPreparedMutation,
    ) -> GraphAppliedPatch;
}

impl GraphReadView for CanonicalDockGraph {
    fn graph_revision(&self) -> GraphRevision {
        GraphRevision::from_version(self.version())
    }

    fn read_group(&self, group: &DockGroupId) -> Option<GraphGroupRead> {
        let value = self.group(group)?;
        let node = self.index().node_for_group(group)?;
        let owner = self.index().owner_for_node(node)?;
        let (space, floating) = match owner {
            crate::graph_index::CanonicalNodeOwner::Main(space) => (space.clone(), None),
            crate::graph_index::CanonicalNodeOwner::Floating { space, floating } => {
                (space.clone(), Some(floating.clone()))
            }
        };
        Some(GraphGroupRead {
            group: group.clone(),
            space,
            floating,
            is_owner_root: self.index().parent_of(node) == Some(None),
            items: value.items.clone(),
            selected: value.selected.clone(),
            mru: value.mru.clone(),
        })
    }

    fn validate_group_read(
        &self,
        expected: &GraphGroupRead,
        selection_sensitive: bool,
        mru_sensitive: bool,
    ) -> Result<(), GraphSupersededReason> {
        let Some(value) = self.group(&expected.group) else {
            return Err(GraphSupersededReason::GroupMissing);
        };
        let Some(node) = self.index().node_for_group(&expected.group) else {
            return Err(GraphSupersededReason::GroupMissing);
        };
        if !matches!(
            self.node(node),
            Some(crate::graph_assembly::CanonicalNode::Group { group, .. })
                if group == &expected.group
        ) {
            return Err(GraphSupersededReason::GroupIdentityChanged);
        }
        let Some(owner) = self.index().owner_for_node(node) else {
            return Err(GraphSupersededReason::GroupMissing);
        };
        let (space, floating) = match owner {
            crate::graph_index::CanonicalNodeOwner::Main(space) => (space, None),
            crate::graph_index::CanonicalNodeOwner::Floating { space, floating } => {
                (space, Some(floating))
            }
        };
        if space != &expected.space
            || floating != expected.floating.as_ref()
            || (self.index().parent_of(node) == Some(None)) != expected.is_owner_root
        {
            return Err(GraphSupersededReason::GroupOwnerChanged);
        }
        if value.items != expected.items {
            return Err(GraphSupersededReason::GroupMembershipChanged);
        }
        if selection_sensitive && value.selected != expected.selected {
            return Err(GraphSupersededReason::GroupSelectionChanged);
        }
        if mru_sensitive && value.mru != expected.mru {
            return Err(GraphSupersededReason::GroupSelectionChanged);
        }
        Ok(())
    }

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        self.index().group_for_item(item).cloned()
    }

    fn read_floating_root(&self, floating: &DockFloatingId) -> Option<GraphFloatingRootRead> {
        self.floating_roots()
            .get(floating)
            .map(|root| GraphFloatingRootRead {
                floating: floating.clone(),
                space: root.space.clone(),
                bounds: GraphFloatingBounds::new(root.bounds)
                    .expect("validated floating root bounds must be finite and positive"),
                z_order: root.z_order,
            })
    }

    fn read_floating_subtree_nodes(
        &self,
        floating: &DockFloatingId,
    ) -> Option<Vec<CanonicalNodeId>> {
        let root = self.floating_roots().get(floating)?.root;
        let mut nodes = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            nodes.push(node);
            match self.node(node)? {
                crate::graph_assembly::CanonicalNode::Group { .. } => {}
                crate::graph_assembly::CanonicalNode::Branch { first, second, .. } => {
                    pending.push(*second);
                    pending.push(*first);
                }
            }
        }
        Some(nodes)
    }

    fn validate_floating_read(
        &self,
        expected: &GraphFloatingRootRead,
    ) -> Result<(), GraphSupersededReason> {
        let Some(floating) = self.floating_roots().get(&expected.floating) else {
            return Err(GraphSupersededReason::FloatingMissing);
        };
        if floating.space != expected.space {
            return Err(GraphSupersededReason::FloatingOwnerChanged);
        }
        if GraphFloatingBounds::new(floating.bounds) != Some(expected.bounds)
            || floating.z_order != expected.z_order
        {
            return Err(GraphSupersededReason::FloatingOwnerChanged);
        }
        Ok(())
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<GraphSpaceRead> {
        let state = self.space(space)?;
        let central_group = state.central.node.and_then(|node| match self.node(node) {
            Some(crate::graph_assembly::CanonicalNode::Group { group, .. }) => Some(group.clone()),
            Some(crate::graph_assembly::CanonicalNode::Branch { .. }) | None => None,
        });
        Some(GraphSpaceRead {
            space: space.clone(),
            has_main_root: state.root.is_some(),
            floating_roots: state.floating_roots.clone(),
            floating_z_orders: state
                .floating_roots
                .iter()
                .map(|floating| {
                    let root = self
                        .floating_roots()
                        .get(floating)
                        .expect("validated space must reference an existing floating root");
                    (floating.clone(), root.z_order)
                })
                .collect(),
            central_group,
            keep_central_alive_when_empty: state.central.keep_alive_when_empty,
            passthrough_when_empty: state.central.passthrough_when_empty,
        })
    }

    fn validate_space_read(&self, expected: &GraphSpaceRead) -> Result<(), GraphSupersededReason> {
        let Some(state) = self.space(&expected.space) else {
            return Err(GraphSupersededReason::SpaceMissing);
        };
        let central_group = state.central.node.and_then(|node| match self.node(node) {
            Some(crate::graph_assembly::CanonicalNode::Group { group, .. }) => Some(group),
            Some(crate::graph_assembly::CanonicalNode::Branch { .. }) | None => None,
        });
        if state.root.is_some() != expected.has_main_root
            || state.floating_roots != expected.floating_roots
            || central_group != expected.central_group.as_ref()
            || state.central.keep_alive_when_empty != expected.keep_central_alive_when_empty
            || state.central.passthrough_when_empty != expected.passthrough_when_empty
            || state.floating_roots.len() != expected.floating_z_orders.len()
        {
            return Err(GraphSupersededReason::SpaceTopologyChanged);
        }
        for (floating, (expected_floating, expected_z_order)) in state
            .floating_roots
            .iter()
            .zip(expected.floating_z_orders.iter())
        {
            let Some(root) = self.floating_roots().get(floating) else {
                return Err(GraphSupersededReason::SpaceTopologyChanged);
            };
            if floating != expected_floating || root.z_order != *expected_z_order {
                return Err(GraphSupersededReason::SpaceTopologyChanged);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum GraphPrepareError {
    #[error("group {group} does not exist")]
    MissingGroup { group: DockGroupId },
    #[error("item {item} does not exist")]
    MissingItem { item: DockItemId },
    #[error("item {item} is not a member of group {group}")]
    ItemNotInGroup {
        group: DockGroupId,
        item: DockItemId,
    },
    #[error("group {group} already exists")]
    GroupAlreadyExists { group: DockGroupId },
    #[error("new group id cannot be empty")]
    EmptyGroupId,
    #[error(
        "item-to-empty-space move has invalid group identity: source {source_group}, destination {new_group}"
    )]
    InvalidItemMoveGroupIdentity {
        source_group: DockGroupId,
        new_group: DockGroupId,
    },
    #[error("floating root {floating} already exists")]
    FloatingAlreadyExists { floating: DockFloatingId },
    #[error("new floating root id cannot be empty")]
    EmptyFloatingId,
    #[error("floating bounds must be finite with positive width and height")]
    InvalidFloatingBounds,
    #[error("dock space {space} has exhausted floating z-order values")]
    FloatingZOrderExhausted { space: DockSpaceId },
    #[error("new group {group} contains duplicate item {item}")]
    DuplicateNewGroupItem {
        group: DockGroupId,
        item: DockItemId,
    },
    #[error("item {item} already belongs to group {group}")]
    ItemAlreadyExists {
        item: DockItemId,
        group: DockGroupId,
    },
    #[error("split ratio must be finite and strictly between zero and one")]
    InvalidSplitRatio,
    #[error("group {group} is not owned by dock space {space}")]
    GroupOutsideSpace {
        group: DockGroupId,
        space: DockSpaceId,
    },
    #[error("floating root {floating} does not exist")]
    MissingFloating { floating: DockFloatingId },
    #[error("floating root {floating} is not owned by dock space {space}")]
    FloatingOutsideSpace {
        floating: DockFloatingId,
        space: DockSpaceId,
    },
    #[error("floating root {floating} cannot dock beside its own descendant group {group}")]
    FloatingTargetInsidePayload {
        floating: DockFloatingId,
        group: DockGroupId,
    },
    #[error("floating root {floating} subtree is unavailable for sealed commit preparation")]
    FloatingSubtreeUnavailable { floating: DockFloatingId },
    #[error("dock space {space} does not exist")]
    MissingSpace { space: DockSpaceId },
    #[error("dock space {space} is not empty")]
    TargetSpaceNotEmpty { space: DockSpaceId },
    #[error("source and target groups must be different")]
    SameGroupMove,
    #[error("graph commit id space is exhausted")]
    CommitIdExhausted,
    #[error("graph receipt capacity is exhausted")]
    CommitReceiptCapacityExhausted,
    #[error("canonical graph capacity reservation is exhausted")]
    GraphCapacityExhausted,
    #[error("prepared mutation does not match its symbolic graph operation")]
    PreparedMutationMismatch,
}

impl GraphReadSet {
    pub(crate) fn check<T: GraphReadView>(
        &self,
        target: &T,
    ) -> Result<GraphRevision, GraphSupersededReason> {
        let actual = target.graph_revision();
        if actual.authority_epoch() != self.base_revision.authority_epoch() {
            return Err(GraphSupersededReason::AuthorityChanged);
        }
        if self.domains.topology && actual.topology() != self.base_revision.topology() {
            return Err(GraphSupersededReason::TopologyChanged);
        }
        if self.domains.allocator && actual.allocator() != self.base_revision.allocator() {
            return Err(GraphSupersededReason::AllocatorChanged);
        }
        if self.domains.selection && actual.selection() != self.base_revision.selection() {
            return Err(GraphSupersededReason::SelectionChanged);
        }
        if self.domains.interaction && actual.interaction() != self.base_revision.interaction() {
            return Err(GraphSupersededReason::InteractionChanged);
        }

        for expected in self.groups() {
            target.validate_group_read(
                expected,
                self.group_selection_sensitive,
                self.group_mru_sensitive,
            )?;
        }
        for expected in self.floating_roots() {
            target.validate_floating_read(expected)?;
        }
        for expected in self.spaces() {
            target.validate_space_read(expected)?;
        }
        Ok(actual)
    }
}
