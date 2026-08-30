#![allow(dead_code)]

use crate::graph_assembly::{CanonicalDockGraph, CanonicalGraphError, DockGraphAssembly};
use crate::graph_patch::{
    GraphAppliedPatch, GraphCommitOutcome, GraphPatchTarget, GraphPrepareError,
    GraphPrepareOutcome, GraphReadView, GraphRevision,
};
use crate::graph_transaction::GraphTransactionEngine;
use crate::{
    CanonicalGraphAuthority, CanonicalGraphVersion, DockFloatingId, DockGroupId, DockItemId,
    DockLayoutRect, DockSpaceId, SplitAxis,
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_select_tab(self, group, item);
        self.transactions = transactions;
        result
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_split_group(
            self,
            space,
            target_group,
            new_group,
            items,
            selected,
            axis,
            insert_before,
            ratio,
        );
        self.transactions = transactions;
        result
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_group_to_edge(
            self,
            source_group,
            target_group,
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.transactions = transactions;
        result
    }

    pub(crate) fn prepare_move_group_to_center(
        &mut self,
        source_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_group_to_center(
            self,
            source_group,
            target_group,
            source_space,
            target_space,
        );
        self.transactions = transactions;
        result
    }

    pub(crate) fn prepare_move_group_to_empty_space(
        &mut self,
        source_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_group_to_empty_space(
            self,
            source_group,
            source_space,
            target_space,
        );
        self.transactions = transactions;
        result
    }

    pub(crate) fn prepare_move_item_to_group(
        &mut self,
        item: impl Into<DockItemId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        insert_index: Option<usize>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_item_to_group(
            self,
            item,
            target_group,
            source_space,
            target_space,
            insert_index,
        );
        self.transactions = transactions;
        result
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_item_to_edge(
            self,
            item,
            new_group,
            target_group,
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.transactions = transactions;
        result
    }

    pub(crate) fn prepare_move_item_to_empty_space(
        &mut self,
        item: impl Into<DockItemId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        new_group: impl Into<DockGroupId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_item_to_empty_space(
            self,
            item,
            source_space,
            target_space,
            new_group,
        );
        self.transactions = transactions;
        result
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_float_item_in_window(
            self,
            item,
            new_group,
            floating,
            source_space,
            target_space,
            bounds,
        );
        self.transactions = transactions;
        result
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
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.prepare_move_floating_root_to_edge(
            self,
            floating,
            source_space,
            target_group,
            target_space,
            axis,
            insert_before,
            ratio,
        );
        self.transactions = transactions;
        result
    }

    pub(crate) fn commit(
        &mut self,
        prepared: crate::graph_patch::PreparedGraphPatch,
    ) -> GraphCommitOutcome {
        let mut transactions = std::mem::take(&mut self.transactions);
        let result = transactions.commit(self, prepared);
        self.transactions = transactions;
        result
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.transactions.receipt_count()
    }
}

impl GraphReadView for CanonicalGraphRuntime {
    fn graph_revision(&self) -> GraphRevision {
        self.graph_revision()
    }

    fn read_group(&self, group: &DockGroupId) -> Option<crate::graph_patch::GraphGroupRead> {
        self.graph.read_group(group)
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

    fn read_space(&self, space: &DockSpaceId) -> Option<crate::graph_patch::GraphSpaceRead> {
        self.graph.read_space(space)
    }
}

impl GraphPatchTarget for CanonicalGraphRuntime {
    fn apply_select_tab(&mut self, group: &DockGroupId, item: &DockItemId) -> GraphAppliedPatch {
        debug_assert!(
            self.graph
                .group(group)
                .is_some_and(|group_state| group_state.items.contains(item)),
            "prepared selection patch must prove group membership before commit"
        );

        let next_version: CanonicalGraphVersion = self
            .authority
            .next_selection_change()
            .expect("canonical graph selection revision exhausted");
        let effects = self
            .graph
            .apply_selection_for_commit(group, item, next_version)
            .expect("validated selection patch must be applicable");
        let recorded = self.authority.record_selection_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }

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
                items,
                selected,
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
                next_version,
            )
            .expect("validated floating-root move patch must be applicable");
        let recorded = self.authority.record_topology_change();
        debug_assert_eq!(recorded, next_version);
        GraphAppliedPatch::new(GraphRevision::from_version(self.graph.version()), effects)
    }
}
