#![allow(dead_code)]

use crate::graph_assembly::CanonicalDockGraph;
use crate::{
    CanonicalGraphVersion, DockFloatingId, DockGroupId, DockItemId, DockSpaceId, SplitAxis,
};
use std::fmt;
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
            selection,
            interaction,
        }
    }

    pub(crate) fn from_version(version: CanonicalGraphVersion) -> Self {
        Self {
            authority_epoch: version.authority_epoch(),
            topology: version.topology_revision(),
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

    pub(crate) const fn selection(self) -> u64 {
        self.selection
    }

    pub(crate) const fn interaction(self) -> u64 {
        self.interaction
    }

    pub(crate) const fn with_topology(self, topology: u64) -> Self {
        Self { topology, ..self }
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
    generation: u64,
}

impl GraphCommitId {
    pub(crate) const fn new(authority_epoch: u64, generation: u64) -> Self {
        Self {
            authority_epoch,
            generation,
        }
    }

    pub(crate) const fn authority_epoch(self) -> u64 {
        self.authority_epoch
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }
}

impl fmt::Display for GraphCommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.authority_epoch, self.generation)
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
}

/// Small immutable dock-space fact captured by an empty-space move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphSpaceRead {
    pub(crate) space: DockSpaceId,
    pub(crate) has_main_root: bool,
    pub(crate) floating_roots: Vec<DockFloatingId>,
    pub(crate) central_group: Option<DockGroupId>,
    pub(crate) keep_central_alive_when_empty: bool,
    pub(crate) passthrough_when_empty: bool,
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

/// Declared revision domains that a patch depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphReadDomains {
    pub(crate) topology: bool,
    pub(crate) selection: bool,
    pub(crate) interaction: bool,
}

impl GraphReadDomains {
    pub(crate) const fn select_tab() -> Self {
        Self {
            topology: true,
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
    group: GraphGroupRead,
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
            group,
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
                selection: false,
                interaction: false,
            },
            group,
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

    pub(crate) fn for_group_to_empty_space(
        base_revision: GraphRevision,
        group: GraphGroupRead,
        target_space: GraphSpaceRead,
    ) -> Self {
        let mut read_set = Self::for_groups(base_revision, [group]);
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

    pub(crate) fn group(&self) -> &GraphGroupRead {
        &self.group
    }

    pub(crate) fn groups(&self) -> impl Iterator<Item = &GraphGroupRead> {
        std::iter::once(&self.group).chain(self.additional_groups.iter())
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
    /// Merges every source tab into the target group and retires the source group leaf.
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

/// A local symbolic patch and its declared read set.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GraphPatch {
    operation: GraphPatchOperation,
    read_set: GraphReadSet,
}

impl GraphPatch {
    pub(crate) const fn new(operation: GraphPatchOperation, read_set: GraphReadSet) -> Self {
        Self {
            operation,
            read_set,
        }
    }

    pub(crate) fn operation(&self) -> &GraphPatchOperation {
        &self.operation
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
}

impl PreparedGraphPatch {
    pub(crate) const fn new(commit_id: GraphCommitId, patch: GraphPatch) -> Self {
        Self { commit_id, patch }
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

/// Receipt retained for idempotent commit replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphCommitReceipt {
    commit_id: GraphCommitId,
    before: GraphRevision,
    after: GraphRevision,
    operation: GraphPatchOperation,
}

impl GraphCommitReceipt {
    pub(crate) const fn new(
        commit_id: GraphCommitId,
        before: GraphRevision,
        after: GraphRevision,
        operation: GraphPatchOperation,
    ) -> Self {
        Self {
            commit_id,
            before,
            after,
            operation,
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
        &self.operation
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

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        let _ = item;
        None
    }

    fn read_floating_root(&self, floating: &DockFloatingId) -> Option<GraphFloatingRootRead> {
        let _ = floating;
        None
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<GraphSpaceRead> {
        let _ = space;
        None
    }
}

/// Infallible write boundary for a proven symbolic patch.
///
/// The trait intentionally has no `Result` return. All domain validation belongs to preparation
/// and read-set revalidation; once either method is called, the target has promised that the local
/// write is structurally valid.
pub(crate) trait GraphPatchTarget: GraphReadView {
    fn apply_select_tab(&mut self, group: &DockGroupId, item: &DockItemId) -> GraphRevision;

    fn apply_split_group(
        &mut self,
        target_group: &DockGroupId,
        new_group: &DockGroupId,
        items: &[DockItemId],
        selected: &DockItemId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        space: &DockSpaceId,
    ) -> GraphRevision;

    fn apply_move_group_to_edge(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> GraphRevision;

    fn apply_move_group_to_center(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> GraphRevision;

    fn apply_move_group_to_empty_space(
        &mut self,
        source_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> GraphRevision;

    fn apply_move_item_to_group(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        insert_index: Option<usize>,
    ) -> GraphRevision;

    fn apply_move_item_to_edge(
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
    ) -> GraphRevision;

    fn apply_move_item_to_empty_space(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        new_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> GraphRevision;

    fn apply_move_floating_root_to_edge(
        &mut self,
        floating: &DockFloatingId,
        source_space: &DockSpaceId,
        target_group: &DockGroupId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> GraphRevision;
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

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        self.index().group_for_item(item).cloned()
    }

    fn read_floating_root(&self, floating: &DockFloatingId) -> Option<GraphFloatingRootRead> {
        self.floating_roots()
            .get(floating)
            .map(|root| GraphFloatingRootRead {
                floating: floating.clone(),
                space: root.space.clone(),
            })
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<GraphSpaceRead> {
        let state = self.space(space)?;
        let central_group = state.central.node.and_then(|node| match self.node(node) {
            Some(crate::graph_assembly::CanonicalNode::Group { group, .. }) => {
                Some(group.clone())
            }
            Some(crate::graph_assembly::CanonicalNode::Branch { .. }) | None => None,
        });
        Some(GraphSpaceRead {
            space: space.clone(),
            has_main_root: state.root.is_some(),
            floating_roots: state.floating_roots.clone(),
            central_group,
            keep_central_alive_when_empty: state.central.keep_alive_when_empty,
            passthrough_when_empty: state.central.passthrough_when_empty,
        })
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
    #[error("dock space {space} does not exist")]
    MissingSpace { space: DockSpaceId },
    #[error("dock space {space} is not empty")]
    TargetSpaceNotEmpty { space: DockSpaceId },
    #[error("source and target groups must be different")]
    SameGroupMove,
    #[error("graph commit id space is exhausted")]
    CommitIdExhausted,
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
        if self.domains.selection && actual.selection() != self.base_revision.selection() {
            return Err(GraphSupersededReason::SelectionChanged);
        }
        if self.domains.interaction && actual.interaction() != self.base_revision.interaction() {
            return Err(GraphSupersededReason::InteractionChanged);
        }

        for expected in self.groups() {
            let Some(group) = target.read_group(&expected.group) else {
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
            if self.group_selection_sensitive && group.selected != expected.selected {
                return Err(GraphSupersededReason::GroupSelectionChanged);
            }
            if self.group_mru_sensitive && group.mru != expected.mru {
                return Err(GraphSupersededReason::GroupSelectionChanged);
            }
        }
        for expected in self.floating_roots() {
            let Some(floating) = target.read_floating_root(&expected.floating) else {
                return Err(GraphSupersededReason::FloatingMissing);
            };
            if floating.space != expected.space {
                return Err(GraphSupersededReason::FloatingOwnerChanged);
            }
        }
        for expected in self.spaces() {
            let Some(space) = target.read_space(&expected.space) else {
                return Err(GraphSupersededReason::SpaceMissing);
            };
            if &space != expected {
                return Err(GraphSupersededReason::SpaceTopologyChanged);
            }
        }
        Ok(actual)
    }
}
