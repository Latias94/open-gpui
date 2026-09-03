#![allow(dead_code)]

use crate::graph_assembly::{CanonicalDockGraph, CanonicalGraphError, DockGraphAssembly};
use crate::graph_patch::{
    GraphAppliedPatch, GraphCommitOutcome, GraphPatchOperation, GraphPatchTarget,
    GraphPrepareError, GraphPrepareOutcome, GraphPreparedMutation, GraphReadView, GraphRevision,
};
use crate::graph_transaction::GraphTransactionEngine;
use crate::{
    CanonicalGraphAuthority, CanonicalGraphRevisionChange, CanonicalGraphVersion, DockFloatingId,
    DockGroupId, DockItemId, DockLayoutRect, DockSpaceId, SplitAxis,
};
use thiserror::Error;

/// Production owner for one published canonical docking graph.
///
/// The owner keeps the graph, its authority revisions, and the linear transaction receipt store
/// together. No caller can mutate the graph without going through a prepared patch target.
#[derive(Debug)]
pub(crate) struct CanonicalGraphRuntime {
    graph: CanonicalDockGraph,
    authority: CanonicalGraphAuthority,
    transactions: GraphTransactionEngine,
}

struct CanonicalGraphCommitTarget<'a> {
    graph: &'a mut CanonicalDockGraph,
    authority: &'a mut CanonicalGraphAuthority,
}

#[derive(Debug, Error)]
pub(crate) enum CanonicalGraphRuntimeError {
    #[error("canonical graph publication version does not belong to the supplied authority")]
    VersionMismatch,
    #[error(transparent)]
    Graph(#[from] CanonicalGraphError),
}

impl CanonicalGraphRuntime {
    /// Publishes an assembly into a fresh authority-owned runtime.
    pub(crate) fn from_assembly(assembly: DockGraphAssembly) -> Result<Self, CanonicalGraphError> {
        let mut authority = CanonicalGraphAuthority::new();
        let graph = assembly.publish(&mut authority)?;
        Ok(Self {
            graph,
            authority,
            transactions: GraphTransactionEngine::new(),
        })
    }

    /// Takes ownership of an already published graph and verifies its authority lineage.
    pub(crate) fn from_published(
        graph: CanonicalDockGraph,
        authority: CanonicalGraphAuthority,
    ) -> Result<Self, CanonicalGraphRuntimeError> {
        if graph.version() != authority.current() {
            return Err(CanonicalGraphRuntimeError::VersionMismatch);
        }
        graph.validate()?;
        Ok(Self {
            graph,
            authority,
            transactions: GraphTransactionEngine::new(),
        })
    }

    pub(crate) fn graph(&self) -> &CanonicalDockGraph {
        &self.graph
    }

    pub(crate) fn graph_revision(&self) -> GraphRevision {
        GraphRevision::from_version(self.graph.version())
    }

    pub(crate) fn authority_epoch(&self) -> u64 {
        self.authority.current().authority_epoch()
    }

    pub(crate) fn prepare_select_tab(
        &mut self,
        group: impl Into<DockGroupId>,
        item: impl Into<DockItemId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self
            .transactions
            .prepare_select_tab(&self.graph, group, item);
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_close_item(
        &mut self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self
            .transactions
            .prepare_close_item(&self.graph, space, item);
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_split_group(
        &mut self,
        space: impl Into<DockSpaceId>,
        target_group: impl Into<DockGroupId>,
        new_group: impl Into<DockGroupId>,
        items: impl IntoIterator<Item = DockItemId>,
        selected: impl Into<DockItemId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_split_group(
            &self.graph,
            space,
            target_group,
            new_group,
            items,
            selected,
            axis,
            insert_before,
            ratio,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_group_to_edge(
        &mut self,
        source_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_group_to_edge(
            &self.graph,
            source_group,
            target_group,
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_group_to_center(
        &mut self,
        source_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_group_to_center(
            &self.graph,
            source_group,
            target_group,
            source_space,
            target_space,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_group_to_empty_space(
        &mut self,
        source_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_group_to_empty_space(
            &self.graph,
            source_group,
            source_space,
            target_space,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_item_to_group(
        &mut self,
        item: impl Into<DockItemId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        insert_index: Option<usize>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_item_to_group(
            &self.graph,
            item,
            target_group,
            source_space,
            target_space,
            insert_index,
        );
        self.seal_prepare_outcome(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_move_item_to_edge(
        &mut self,
        item: impl Into<DockItemId>,
        new_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_item_to_edge(
            &self.graph,
            item,
            new_group,
            target_group,
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_item_to_empty_space(
        &mut self,
        item: impl Into<DockItemId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        new_group: impl Into<DockGroupId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_item_to_empty_space(
            &self.graph,
            item,
            source_space,
            target_space,
            new_group,
        );
        self.seal_prepare_outcome(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_float_item_in_window(
        &mut self,
        item: impl Into<DockItemId>,
        new_group: impl Into<DockGroupId>,
        floating: impl Into<DockFloatingId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        bounds: DockLayoutRect,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_float_item_in_window(
            &self.graph,
            item,
            new_group,
            floating,
            source_space,
            target_space,
            bounds,
        );
        self.seal_prepare_outcome(result)
    }

    pub(crate) fn prepare_move_floating_root_to_edge(
        &mut self,
        floating: impl Into<DockFloatingId>,
        source_space: impl Into<DockSpaceId>,
        target_group: impl Into<DockGroupId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let result = self.transactions.prepare_move_floating_root_to_edge(
            &self.graph,
            floating,
            source_space,
            target_group,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.seal_prepare_outcome(result)
    }

    fn seal_prepare_outcome(
        &mut self,
        outcome: Result<GraphPrepareOutcome, GraphPrepareError>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        match outcome? {
            GraphPrepareOutcome::Prepared(prepared) => {
                match self.graph.try_reserve_for_patch(
                    prepared.commit_id(),
                    prepared.operation(),
                    prepared.mutation(),
                ) {
                    Ok(proof) => Ok(GraphPrepareOutcome::Prepared(
                        prepared.with_capacity_proof(proof),
                    )),
                    Err(error) => {
                        let released = self.transactions.release_latest_prepared_slot(prepared);
                        debug_assert!(released);
                        Err(error)
                    }
                }
            }
            GraphPrepareOutcome::NoOp(receipt) => Ok(GraphPrepareOutcome::NoOp(receipt)),
        }
    }

    pub(crate) fn commit(
        &mut self,
        prepared: crate::graph_patch::PreparedGraphPatch,
    ) -> GraphCommitOutcome {
        let Self {
            graph,
            authority,
            transactions,
        } = self;
        transactions.commit(
            &mut CanonicalGraphCommitTarget { graph, authority },
            prepared,
        )
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.transactions.receipt_count()
    }

    #[cfg(test)]
    pub(crate) fn prepare_unsealed_select_tab_for_test(
        &mut self,
        group: impl Into<DockGroupId>,
        item: impl Into<DockItemId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        self.transactions
            .prepare_select_tab(&self.graph, group, item)
    }
}

impl GraphReadView for CanonicalGraphRuntime {
    fn graph_revision(&self) -> GraphRevision {
        self.graph_revision()
    }

    fn read_group(&self, group: &DockGroupId) -> Option<crate::graph_patch::GraphGroupRead> {
        self.graph.read_group(group)
    }

    fn validate_group_read(
        &self,
        expected: &crate::graph_patch::GraphGroupRead,
        selection_sensitive: bool,
        mru_sensitive: bool,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph
            .validate_group_read(expected, selection_sensitive, mru_sensitive)
    }

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        self.graph.index().group_for_item(item).cloned()
    }

    fn read_floating_root(
        &self,
        floating: &DockFloatingId,
    ) -> Option<crate::graph_patch::GraphFloatingRootRead> {
        self.graph.read_floating_root(floating)
    }

    fn read_floating_subtree_nodes(
        &self,
        floating: &DockFloatingId,
    ) -> Option<Vec<crate::CanonicalNodeId>> {
        self.graph.read_floating_subtree_nodes(floating)
    }

    fn validate_floating_read(
        &self,
        expected: &crate::graph_patch::GraphFloatingRootRead,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph.validate_floating_read(expected)
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<crate::graph_patch::GraphSpaceRead> {
        self.graph.read_space(space)
    }

    fn validate_space_read(
        &self,
        expected: &crate::graph_patch::GraphSpaceRead,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph.validate_space_read(expected)
    }
}

impl GraphReadView for CanonicalGraphCommitTarget<'_> {
    fn graph_revision(&self) -> GraphRevision {
        GraphRevision::from_version(self.graph.version())
    }

    fn read_group(&self, group: &DockGroupId) -> Option<crate::graph_patch::GraphGroupRead> {
        self.graph.read_group(group)
    }

    fn validate_group_read(
        &self,
        expected: &crate::graph_patch::GraphGroupRead,
        selection_sensitive: bool,
        mru_sensitive: bool,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph
            .validate_group_read(expected, selection_sensitive, mru_sensitive)
    }

    fn read_item_group(&self, item: &DockItemId) -> Option<DockGroupId> {
        self.graph.index().group_for_item(item).cloned()
    }

    fn read_floating_root(
        &self,
        floating: &DockFloatingId,
    ) -> Option<crate::graph_patch::GraphFloatingRootRead> {
        self.graph.read_floating_root(floating)
    }

    fn read_floating_subtree_nodes(
        &self,
        floating: &DockFloatingId,
    ) -> Option<Vec<crate::CanonicalNodeId>> {
        self.graph.read_floating_subtree_nodes(floating)
    }

    fn validate_floating_read(
        &self,
        expected: &crate::graph_patch::GraphFloatingRootRead,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph.validate_floating_read(expected)
    }

    fn read_space(&self, space: &DockSpaceId) -> Option<crate::graph_patch::GraphSpaceRead> {
        self.graph.read_space(space)
    }

    fn validate_space_read(
        &self,
        expected: &crate::graph_patch::GraphSpaceRead,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        self.graph.validate_space_read(expected)
    }
}

impl GraphPatchTarget for CanonicalGraphCommitTarget<'_> {
    fn validate_capacity_proof(
        &self,
        commit_id: crate::graph_patch::GraphCommitId,
        operation: &GraphPatchOperation,
        mutation: &GraphPreparedMutation,
        proof: Option<&crate::graph_patch::GraphCapacityProof>,
    ) -> Result<(), crate::graph_patch::GraphSupersededReason> {
        let Some(proof) = proof else {
            return Err(crate::graph_patch::GraphSupersededReason::CapacityProofMissing);
        };
        self.graph
            .validate_capacity_proof(commit_id, operation, mutation, proof)
    }

    fn apply_prepared_patch(
        &mut self,
        operation: &GraphPatchOperation,
        mutation: GraphPreparedMutation,
    ) -> GraphAppliedPatch {
        match (operation, mutation) {
            (
                GraphPatchOperation::SelectTab { group, item },
                GraphPreparedMutation::SelectTab {
                    selection_changed,
                    interaction_changed,
                },
            ) => self.apply_select_tab(group, item, selection_changed, interaction_changed),
            (
                GraphPatchOperation::CloseItem { group, item, space },
                GraphPreparedMutation::CloseItem {
                    next_selected,
                    selection_changed,
                    interaction_changed,
                },
            ) => self.apply_close_item(
                group,
                item,
                space,
                next_selected,
                selection_changed,
                interaction_changed,
            ),
            (
                GraphPatchOperation::SplitGroup {
                    target_group,
                    new_group,
                    axis,
                    insert_before,
                    ratio,
                    space,
                    ..
                },
                GraphPreparedMutation::SplitGroup { new_group: state },
            ) => self.apply_split_group(
                target_group,
                new_group,
                state,
                *axis,
                *insert_before,
                ratio.get(),
                space,
            ),
            (
                GraphPatchOperation::MoveGroupToEdge {
                    source_group,
                    target_group,
                    source_space,
                    target_space,
                    axis,
                    insert_before,
                    ratio,
                },
                GraphPreparedMutation::MoveGroupToEdge,
            ) => self.apply_move_group_to_edge(
                source_group,
                target_group,
                source_space,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
            ),
            (
                GraphPatchOperation::MoveGroupToCenter {
                    source_group,
                    target_group,
                    source_space,
                    target_space,
                },
                GraphPreparedMutation::MoveGroupToCenter,
            ) => self.apply_move_group_to_center(
                source_group,
                target_group,
                source_space,
                target_space,
            ),
            (
                GraphPatchOperation::MoveGroupToEmptySpace {
                    source_group,
                    source_space,
                    target_space,
                },
                GraphPreparedMutation::MoveGroupToEmptySpace,
            ) => self.apply_move_group_to_empty_space(source_group, source_space, target_space),
            (
                GraphPatchOperation::MoveItemToGroup {
                    item,
                    source_group,
                    target_group,
                    source_space,
                    target_space,
                    insert_index,
                },
                GraphPreparedMutation::MoveItemToGroup,
            ) => self.apply_move_item_to_group(
                item,
                source_group,
                target_group,
                source_space,
                target_space,
                *insert_index,
            ),
            (
                GraphPatchOperation::MoveItemToEdge {
                    item,
                    source_group,
                    new_group,
                    target_group,
                    source_space,
                    target_space,
                    axis,
                    insert_before,
                    ratio,
                },
                GraphPreparedMutation::MoveItemToEdge { new_group: state },
            ) => self.apply_move_item_to_edge(
                item,
                source_group,
                new_group,
                target_group,
                source_space,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
                state,
            ),
            (
                GraphPatchOperation::MoveItemToEmptySpace {
                    item,
                    source_group,
                    new_group,
                    source_space,
                    target_space,
                },
                GraphPreparedMutation::MoveItemToEmptySpace { new_group: state },
            ) => self.apply_move_item_to_empty_space(
                item,
                source_group,
                new_group,
                source_space,
                target_space,
                state,
            ),
            (
                GraphPatchOperation::FloatItemInWindow {
                    item,
                    source_group,
                    new_group,
                    floating,
                    source_space,
                    target_space,
                    retired_floating,
                    bounds,
                    z_order,
                },
                GraphPreparedMutation::FloatItemInWindow { new_group: state },
            ) => self.apply_float_item_in_window(
                item,
                source_group,
                new_group,
                floating,
                source_space,
                target_space,
                retired_floating.as_ref(),
                bounds.get(),
                *z_order,
                state,
            ),
            (
                GraphPatchOperation::MoveFloatingRootToEdge {
                    floating,
                    source_space,
                    target_group,
                    target_space,
                    axis,
                    insert_before,
                    ratio,
                },
                GraphPreparedMutation::MoveFloatingRootToEdge { owner_nodes },
            ) => self.apply_move_floating_root_to_edge(
                floating,
                source_space,
                target_group,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
                owner_nodes,
            ),
            _ => unreachable!("validated prepared mutation must match its symbolic operation"),
        }
    }
}

impl CanonicalGraphCommitTarget<'_> {
    fn apply_select_tab(
        &mut self,
        group: &DockGroupId,
        item: &DockItemId,
        selection_changed: bool,
        interaction_changed: bool,
    ) -> GraphAppliedPatch {
        debug_assert!(
            self.graph
                .group(group)
                .is_some_and(|group_state| group_state.items.contains(item)),
            "prepared selection patch must prove group membership before commit"
        );

        let change =
            CanonicalGraphRevisionChange::new(false, false, selection_changed, interaction_changed);
        let next_version: CanonicalGraphVersion = self
            .authority
            .next_change(change)
            .expect("canonical graph revision exhausted");
        let effects = self
            .graph
            .apply_selection_for_commit(group, item, next_version)
            .expect("validated selection patch must be applicable");
        let recorded = self.authority.record_change(change);
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_close_item(
        &mut self,
        group: &DockGroupId,
        item: &DockItemId,
        space: &DockSpaceId,
        next_selected: Option<DockItemId>,
        selection_changed: bool,
        interaction_changed: bool,
    ) -> GraphAppliedPatch {
        let change = CanonicalGraphRevisionChange::new(
            true,
            next_selected.is_none(),
            selection_changed,
            interaction_changed,
        );
        let next_version = self
            .authority
            .next_change(change)
            .expect("canonical graph revision exhausted");
        let effects = self
            .graph
            .apply_close_item_for_commit(group, item, space, next_selected, next_version)
            .expect("validated close-item patch must be applicable");
        let recorded = self.authority.record_change(change);
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_split_group(
        &mut self,
        target_group: &DockGroupId,
        new_group: &DockGroupId,
        new_group_state: crate::graph_assembly::CanonicalGroup,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        space: &DockSpaceId,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_split_group_for_commit(
                target_group,
                new_group,
                new_group_state,
                axis,
                insert_before,
                ratio,
                space,
                next_version,
            )
            .expect("validated split-group patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_group_to_edge(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_group_to_edge_for_commit(
                source_group,
                target_group,
                source_space,
                target_space,
                axis,
                insert_before,
                ratio,
                next_version,
            )
            .expect("validated group move patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_group_to_center(
        &mut self,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_group_to_center_for_commit(
                source_group,
                target_group,
                source_space,
                target_space,
                next_version,
            )
            .expect("validated group merge patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_group_to_empty_space(
        &mut self,
        source_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_group_to_empty_space_for_commit(
                source_group,
                source_space,
                target_space,
                next_version,
            )
            .expect("validated group-to-empty-space patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_item_to_group(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        target_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        insert_index: Option<usize>,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_item_to_group_for_commit(
                item,
                source_group,
                target_group,
                source_space,
                target_space,
                insert_index,
                next_version,
            )
            .expect("validated item move patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

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
        new_group_state: Option<crate::graph_assembly::CanonicalGroup>,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_item_to_edge_for_commit(
                item,
                source_group,
                new_group,
                target_group,
                source_space,
                target_space,
                axis,
                insert_before,
                ratio,
                new_group_state,
                next_version,
            )
            .expect("validated item edge patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_item_to_empty_space(
        &mut self,
        item: &DockItemId,
        source_group: &DockGroupId,
        new_group: &DockGroupId,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        new_group_state: Option<crate::graph_assembly::CanonicalGroup>,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_item_to_empty_space_for_commit(
                item,
                source_group,
                new_group,
                source_space,
                target_space,
                new_group_state,
                next_version,
            )
            .expect("validated item-to-empty-space patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_float_item_in_window(
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
        new_group_state: Option<crate::graph_assembly::CanonicalGroup>,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_float_item_in_window_for_commit(
                item,
                source_group,
                new_group,
                floating,
                source_space,
                target_space,
                retired_floating,
                bounds,
                z_order,
                new_group_state,
                next_version,
            )
            .expect("validated item float patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

    fn apply_move_floating_root_to_edge(
        &mut self,
        floating: &DockFloatingId,
        source_space: &DockSpaceId,
        target_group: &DockGroupId,
        target_space: &DockSpaceId,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
        owner_nodes: Vec<crate::CanonicalNodeId>,
    ) -> GraphAppliedPatch {
        let next_version = self
            .authority
            .next_topology_change()
            .expect("canonical graph topology revision exhausted");
        let effects = self
            .graph
            .apply_move_floating_root_to_edge_for_commit(
                floating,
                source_space,
                target_group,
                target_space,
                axis,
                insert_before,
                ratio,
                owner_nodes,
                next_version,
            )
            .expect("validated floating-root move patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }
}
