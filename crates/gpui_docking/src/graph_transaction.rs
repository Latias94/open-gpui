#![allow(dead_code)]

use crate::graph_patch::{
    GraphCommitId, GraphCommitOutcome, GraphCommitReceipt, GraphNoOpReceipt, GraphPatch,
    GraphPatchOperation, GraphPatchTarget, GraphPrepareError, GraphPrepareOutcome, GraphReadSet,
    GraphReadView, GraphSplitRatio, GraphSupersededReceipt,
};
use crate::{DockFloatingId, DockGroupId, DockItemId, DockSpaceId, SplitAxis};
use std::collections::{BTreeMap, BTreeSet};

/// Prepared-patch owner for the canonical graph transaction boundary.
///
/// This engine retains receipts by commit id and never retains a complete graph. It only allocates
/// a symbolic local patch during preparation; the caller supplies the write target at commit time.
#[derive(Debug, Default)]
pub(crate) struct GraphTransactionEngine {
    next_generation: u64,
    receipts: BTreeMap<GraphCommitId, GraphCommitReceipt>,
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

    /// Prepares merging a complete source tab group into a target tab group.
    pub(crate) fn prepare_move_group_to_center<T: GraphReadView>(
        &mut self,
        view: &T,
        source_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let source_group = source_group.into();
        let target_group = target_group.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if target.space != target_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space: target_space,
            });
        }

        let operation = GraphPatchOperation::MoveGroupToCenter {
            source_group: source_group.clone(),
            target_group: target_group.clone(),
            source_space,
            target_space,
        };
        let revision = view.graph_revision();
        if source_group == target_group {
            return Ok(GraphPrepareOutcome::NoOp(GraphNoOpReceipt::new(
                revision, operation,
            )));
        }

        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set =
            GraphReadSet::for_group_state_sensitive_operation(revision, [source, target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares moving a durable group leaf into an empty dock-space main root.
    pub(crate) fn prepare_move_group_to_empty_space<T: GraphReadView>(
        &mut self,
        view: &T,
        source_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let source_group = source_group.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        let target =
            view.read_space(&target_space)
                .ok_or_else(|| GraphPrepareError::MissingSpace {
                    space: target_space.clone(),
                })?;
        let target_is_empty = if source_space == target_space {
            !target.has_main_root
                && source.is_owner_root
                && source
                    .floating
                    .as_ref()
                    .is_some_and(|floating| target.floating_roots.as_slice() == [floating.clone()])
        } else {
            !target.has_main_root && target.floating_roots.is_empty()
        };
        if !target_is_empty {
            return Err(GraphPrepareError::TargetSpaceNotEmpty {
                space: target_space,
            });
        }

        let operation = GraphPatchOperation::MoveGroupToEmptySpace {
            source_group,
            source_space,
            target_space,
        };
        let revision = view.graph_revision();
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_group_to_empty_space(revision, source, target);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares moving one item into a target group or tab insertion position.
    pub(crate) fn prepare_move_item_to_group<T: GraphReadView>(
        &mut self,
        view: &T,
        item: impl Into<DockItemId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        insert_index: Option<usize>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let item = item.into();
        let target_group = target_group.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        let source_group = view
            .read_item_group(&item)
            .ok_or_else(|| GraphPrepareError::MissingItem { item: item.clone() })?;
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        let source_index = source
            .items
            .iter()
            .position(|candidate| candidate == &item)
            .ok_or_else(|| GraphPrepareError::ItemNotInGroup {
                group: source_group.clone(),
                item: item.clone(),
            })?;
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if target.space != target_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space: target_space,
            });
        }
        let insert_index = insert_index.map(|index| index.min(target.items.len()));
        let operation = GraphPatchOperation::MoveItemToGroup {
            item,
            source_group: source_group.clone(),
            target_group: target_group.clone(),
            source_space,
            target_space,
            insert_index,
        };
        let revision = view.graph_revision();
        if source_group == target_group
            && (insert_index.is_none()
                || insert_index.is_some_and(|index| {
                    index == source_index || index == source_index.saturating_add(1)
                }))
        {
            return Ok(GraphPrepareOutcome::NoOp(GraphNoOpReceipt::new(
                revision, operation,
            )));
        }

        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_group_state_sensitive_operation(
            revision,
            if source_group == target_group {
                vec![source]
            } else {
                vec![source, target]
            },
        );
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares moving one item beside a target group.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_move_item_to_edge<T: GraphReadView>(
        &mut self,
        view: &T,
        item: impl Into<DockItemId>,
        new_group: impl Into<DockGroupId>,
        target_group: impl Into<DockGroupId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let item = item.into();
        let new_group = new_group.into();
        let target_group = target_group.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        let source_group = view
            .read_item_group(&item)
            .ok_or_else(|| GraphPrepareError::MissingItem { item: item.clone() })?;
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        if !source.items.iter().any(|candidate| candidate == &item) {
            return Err(GraphPrepareError::ItemNotInGroup {
                group: source_group,
                item,
            });
        }
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if target.space != target_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space: target_space,
            });
        }
        let ratio = GraphSplitRatio::new(ratio).ok_or(GraphPrepareError::InvalidSplitRatio)?;
        if new_group.as_str().is_empty() {
            return Err(GraphPrepareError::EmptyGroupId);
        }
        if new_group != source_group && view.read_group(&new_group).is_some() {
            return Err(GraphPrepareError::GroupAlreadyExists { group: new_group });
        }
        if (source.items.len() == 1 && new_group != source_group)
            || (source.items.len() > 1 && new_group == source_group)
        {
            return Err(GraphPrepareError::InvalidItemMoveGroupIdentity {
                source_group,
                new_group,
            });
        }

        let operation = GraphPatchOperation::MoveItemToEdge {
            item,
            source_group: source_group.clone(),
            new_group,
            target_group: target_group.clone(),
            source_space,
            target_space,
            axis,
            insert_before,
            ratio,
        };
        let revision = view.graph_revision();
        if source_group == target_group && source.items.len() == 1 {
            return Ok(GraphPrepareOutcome::NoOp(GraphNoOpReceipt::new(
                revision, operation,
            )));
        }

        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_group_state_sensitive_operation(
            revision,
            if source_group == target_group {
                vec![source]
            } else {
                vec![source, target]
            },
        );
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares moving one item into an empty dock-space root.
    pub(crate) fn prepare_move_item_to_empty_space<T: GraphReadView>(
        &mut self,
        view: &T,
        item: impl Into<DockItemId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        new_group: impl Into<DockGroupId>,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let item = item.into();
        let source_space = source_space.into();
        let target_space = target_space.into();
        let new_group = new_group.into();
        let source_group = view
            .read_item_group(&item)
            .ok_or_else(|| GraphPrepareError::MissingItem { item: item.clone() })?;
        let source =
            view.read_group(&source_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: source_group.clone(),
                })?;
        if !source.items.iter().any(|candidate| candidate == &item) {
            return Err(GraphPrepareError::ItemNotInGroup {
                group: source_group,
                item,
            });
        }
        if source.space != source_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: source_group,
                space: source_space,
            });
        }
        let target =
            view.read_space(&target_space)
                .ok_or_else(|| GraphPrepareError::MissingSpace {
                    space: target_space.clone(),
                })?;
        let target_is_empty = if source_space == target_space {
            !target.has_main_root
                && source.items.len() == 1
                && source.is_owner_root
                && source.floating.as_ref().is_some_and(|floating| {
                    target.floating_roots.as_slice() == std::slice::from_ref(floating)
                })
        } else {
            !target.has_main_root && target.floating_roots.is_empty()
        };
        if !target_is_empty {
            return Err(GraphPrepareError::TargetSpaceNotEmpty {
                space: target_space,
            });
        }
        if new_group.as_str().is_empty() {
            return Err(GraphPrepareError::EmptyGroupId);
        }
        if new_group != source_group && view.read_group(&new_group).is_some() {
            return Err(GraphPrepareError::GroupAlreadyExists { group: new_group });
        }
        if source.items.len() == 1 && new_group != source_group {
            return Err(GraphPrepareError::InvalidItemMoveGroupIdentity {
                source_group,
                new_group,
            });
        }
        if source.items.len() > 1 && new_group == source_group {
            return Err(GraphPrepareError::InvalidItemMoveGroupIdentity {
                source_group,
                new_group,
            });
        }

        let operation = GraphPatchOperation::MoveItemToEmptySpace {
            item,
            source_group,
            new_group,
            source_space,
            target_space,
        };
        let revision = view.graph_revision();
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        self.next_generation = generation;
        let read_set = GraphReadSet::for_group_to_empty_space(revision, source, target);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                GraphCommitId::new(revision.authority_epoch(), generation),
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares docking an entire floating root beside a target group.
    pub(crate) fn prepare_move_floating_root_to_edge<T: GraphReadView>(
        &mut self,
        view: &T,
        floating: impl Into<DockFloatingId>,
        source_space: impl Into<DockSpaceId>,
        target_group: impl Into<DockGroupId>,
        target_space: impl Into<DockSpaceId>,
        axis: SplitAxis,
        insert_before: bool,
        ratio: f32,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let floating = floating.into();
        let source_space = source_space.into();
        let target_group = target_group.into();
        let target_space = target_space.into();
        let floating_read = view.read_floating_root(&floating).ok_or_else(|| {
            GraphPrepareError::MissingFloating {
                floating: floating.clone(),
            }
        })?;
        if floating_read.space != source_space {
            return Err(GraphPrepareError::FloatingOutsideSpace {
                floating,
                space: source_space,
            });
        }
        let target =
            view.read_group(&target_group)
                .ok_or_else(|| GraphPrepareError::MissingGroup {
                    group: target_group.clone(),
                })?;
        if target.space != target_space {
            return Err(GraphPrepareError::GroupOutsideSpace {
                group: target_group,
                space: target_space,
            });
        }
        let ratio = GraphSplitRatio::new(ratio).ok_or(GraphPrepareError::InvalidSplitRatio)?;
        let operation = GraphPatchOperation::MoveFloatingRootToEdge {
            floating,
            source_space,
            target_group,
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
        let read_set = GraphReadSet::for_floating_and_groups(revision, floating_read, [target]);
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
        if let Some(receipt) = self.receipts.get(&commit_id) {
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

        let operation = prepared.operation().clone();
        let after = match &operation {
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
            GraphPatchOperation::MoveGroupToCenter {
                source_group,
                target_group,
                source_space,
                target_space,
            } => target.apply_move_group_to_center(
                source_group,
                target_group,
                source_space,
                target_space,
            ),
            GraphPatchOperation::MoveGroupToEmptySpace {
                source_group,
                source_space,
                target_space,
            } => target.apply_move_group_to_empty_space(source_group, source_space, target_space),
            GraphPatchOperation::MoveItemToGroup {
                item,
                source_group,
                target_group,
                source_space,
                target_space,
                insert_index,
            } => target.apply_move_item_to_group(
                item,
                source_group,
                target_group,
                source_space,
                target_space,
                *insert_index,
            ),
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
            } => target.apply_move_item_to_edge(
                item,
                source_group,
                new_group,
                target_group,
                source_space,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
            ),
            GraphPatchOperation::MoveItemToEmptySpace {
                item,
                source_group,
                new_group,
                source_space,
                target_space,
            } => target.apply_move_item_to_empty_space(
                item,
                source_group,
                new_group,
                source_space,
                target_space,
            ),
            GraphPatchOperation::MoveFloatingRootToEdge {
                floating,
                source_space,
                target_group,
                target_space,
                axis,
                insert_before,
                ratio,
            } => target.apply_move_floating_root_to_edge(
                floating,
                source_space,
                target_group,
                target_space,
                *axis,
                *insert_before,
                ratio.get(),
            ),
        };
        let receipt = GraphCommitReceipt::new(commit_id, before, after, operation);
        self.receipts.insert(commit_id, receipt.clone());
        GraphCommitOutcome::Committed(receipt)
    }

    /// Replays a retained receipt without touching the target.
    pub(crate) fn replay(&self, commit_id: GraphCommitId) -> Option<GraphCommitOutcome> {
        self.receipts
            .get(&commit_id)
            .cloned()
            .map(GraphCommitOutcome::AlreadyCommitted)
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.len()
    }
}
