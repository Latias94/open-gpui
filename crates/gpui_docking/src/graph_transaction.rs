#![allow(dead_code)]

use crate::graph_patch::{
    GraphCommitId, GraphCommitOutcome, GraphCommitReceipt, GraphPatch,
    GraphPatchOperation, GraphPatchTarget, GraphPrepareError, GraphPrepareOutcome, GraphReadSet,
    GraphReadView, GraphSupersededReceipt,
};
use crate::{DockGroupId, DockItemId};

/// Prepared-patch owner for the canonical graph transaction boundary.
///
/// This engine retains receipts by commit id and never retains a complete graph. It only allocates
/// a symbolic local patch during preparation; the caller supplies the write target at commit time.
#[derive(Debug, Default)]
pub(crate) struct GraphTransactionEngine {
    next_generation: u64,
    receipts: Vec<GraphCommitReceipt>,
}

impl GraphTransactionEngine {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Prepares a selection change after validating the target group and item.
    pub(crate) fn prepare_select_tab<T: GraphReadView>(
        &mut self,
        view: &T,
        group: impl Into<DockGroupId>,
        item: impl Into<DockItemId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let group = group.into();
        let item = item.into();
        let current = view
            .read_group(&group)
            .ok_or_else(|| GraphPrepareError::MissingGroup {
                group: group.clone(),
            })?;
        if !current.items.contains(&item) {
            return Err(GraphPrepareError::ItemNotInGroup { group, item });
        }

        let operation = GraphPatchOperation::SelectTab {
            group: group.clone(),
            item: item.clone(),
        };
        let revision = view.graph_revision();

        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_select_tab(revision, current);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Commits a prepared patch or returns a typed stale/replay observation.
    ///
    /// Every business check happens before [`GraphPatchTarget::apply_select_tab`]. The target
    /// method is deliberately infallible, so this method has no business-error branch after the
    /// patch has passed its read set.
    pub(crate) fn commit<T: GraphPatchTarget>(
        &mut self,
        target: &mut T,
        prepared: crate::graph_patch::PreparedGraphPatch,
    ) -> GraphCommitOutcome {
        let commit_id = prepared.commit_id();
        if let Some(receipt) = self
            .receipts
            .iter()
            .find(|receipt| receipt.commit_id() == commit_id)
        {
            return GraphCommitOutcome::AlreadyCommitted(receipt.clone());
        }

        let before = target.graph_revision();
        if let Err(reason) = prepared.read_set().check(target) {
            return GraphCommitOutcome::Superseded(GraphSupersededReceipt::new(
                commit_id,
                prepared.read_set().base_revision(),
                before,
                reason,
            ));
        }

        let operation = prepared.operation();
        let after = match operation {
            GraphPatchOperation::SelectTab { group, item } => target.apply_select_tab(group, item),
        };
        let receipt = GraphCommitReceipt::new(
            commit_id,
            before,
            after,
            match operation {
                GraphPatchOperation::SelectTab { group, item } => GraphPatchOperation::SelectTab {
                    group: group.clone(),
                    item: item.clone(),
                },
            },
        );
        self.receipts.push(receipt.clone());
        GraphCommitOutcome::Committed(receipt)
    }

    /// Replays a retained receipt without touching the target.
    pub(crate) fn replay(&self, commit_id: GraphCommitId) -> Option<GraphCommitOutcome> {
        self.receipts
            .iter()
            .find(|receipt| receipt.commit_id() == commit_id)
            .cloned()
            .map(GraphCommitOutcome::AlreadyCommitted)
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.len()
    }
}
