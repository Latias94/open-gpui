#![allow(dead_code)]

use crate::graph_patch::{
    GraphCommitId, GraphCommitOutcome, GraphCommitReceipt, GraphPatch, GraphPatchOperation,
    GraphPatchTarget, GraphPrepareError, GraphPrepareOutcome, GraphReadSet, GraphReadView,
    GraphSplitRatio, GraphSupersededReceipt,
};
use crate::{DockGroupId, DockItemId, DockSpaceId, SplitAxis};
use std::collections::BTreeSet;

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

    /// Prepares insertion of a newly created group beside an existing target group.
    pub(crate) fn prepare_split_group<T: GraphReadView>(
        &mut self,
        view: &T,
        space: impl Into<DockSpaceId>,
        target_group: impl Into<DockGroupId>,
        new_group: impl Into<DockGroupId>,
        items: impl IntoIterator<Item = DockItemId>,
        selected: impl Into<DockItemId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let space = space.into();
        let target_group = target_group.into();
        let new_group = new_group.into();
        let items = items.into_iter().collect::<Vec<_>>();
        let selected = selected.into();
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if target.space != space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space,
            });
        }
        if view.read_group(&new_group).is_some() {
            return Err(GraphPrepareError::GroupAlreadyExists { group: new_group });
        }
        if new_group.as_str().is_empty() {
            return Err(GraphPrepareError::EmptyGroupId);
        }
        if items.is_empty() || !items.contains(&selected) {
            return Err(GraphPrepareError::ItemNotInGroup {
                group: new_group,
                item: selected,
            });
        }
        let mut unique_items = BTreeSet::new();
        for item in &items {
            if !unique_items.insert(item.clone()) {
                return Err(GraphPrepareError::DuplicateNewGroupItem {
                    group: new_group,
                    item: item.clone(),
                });
            }
            if let Some(group) = view.read_item_group(item) {
                return Err(GraphPrepareError::ItemAlreadyExists {
                    item: item.clone(),
                    group,
                });
            }
        }
        let ratio = GraphSplitRatio::new(ratio).ok_or(GraphPrepareError::InvalidSplitRatio)?;

        let operation = GraphPatchOperation::SplitGroup {
            target_group,
            new_group,
            items,
            selected,
            axis,
            insert_before,
            ratio,
            space,
        };
        let revision = view.graph_revision();
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_groups(revision, [target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares moving an existing group beside a target group in a binary edge split.
    pub(crate) fn prepare_move_group_to_edge<T: GraphReadView>(
        &mut self,
        view: &T,
        source_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let source_group = source_group.into();
        let target_group = target_group.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        if source_group == target_group {
            return Err(GraphPrepareError::SameGroupMove);
        }
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        if target.space != target_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space: target_space,
            });
        }
        let ratio = GraphSplitRatio::new(ratio).ok_or(GraphPrepareError::InvalidSplitRatio)?;
        let operation = GraphPatchOperation::MoveGroupToEdge {
            source_group,
            target_group,
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        };
        let revision = view.graph_revision();
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_groups(revision, [source, target]);
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
            GraphPatchOperation::SplitGroup {
                target_group,
                new_group,
                items,
                selected,
                axis,
                insert_before,
                ratio,
                space,
            } => target.apply_split_group(
                target_group,
                new_group,
                items,
                selected,
                *axis,
                *insert_before,
                ratio.get(),
                space,
            ),
            GraphPatchOperation::MoveGroupToEdge {
                source_group,
                target_group,
                source_space,
                target_space,
                axis,
                insert_before,
                ratio,
            } => target.apply_move_group_to_edge(
                source_group,
                target_group,
                source_space,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
            ),
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
                GraphPatchOperation::SplitGroup {
                    target_group,
                    new_group,
                    items,
                    selected,
                    axis,
                    insert_before,
                    ratio,
                    space,
                } => GraphPatchOperation::SplitGroup {
                    target_group: target_group.clone(),
                    new_group: new_group.clone(),
                    items: items.clone(),
                    selected: selected.clone(),
                    axis: *axis,
                    insert_before: *insert_before,
                    ratio: *ratio,
                    space: space.clone(),
                },
                GraphPatchOperation::MoveGroupToEdge {
                    source_group,
                    target_group,
                    source_space,
                    target_space,
                    axis,
                    insert_before,
                    ratio,
                } => GraphPatchOperation::MoveGroupToEdge {
                    source_group: source_group.clone(),
                    target_group: target_group.clone(),
                    source_space: source_space.clone(),
                    target_space: target_space.clone(),
                    axis: *axis,
                    insert_before: *insert_before,
                    ratio: *ratio,
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
