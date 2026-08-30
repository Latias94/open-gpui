#![allow(dead_code)]

use crate::graph_patch::{
    GraphCommitId, GraphCommitOutcome, GraphCommitReceipt, GraphFloatingBounds, GraphNoOpReceipt,
    GraphPatch, GraphPatchOperation, GraphPatchTarget, GraphPrepareError, GraphPrepareOutcome,
    GraphReadSet, GraphReadView, GraphReceiptReservation, GraphRevision, GraphSplitRatio,
    GraphSupersededReceipt,
};
use crate::{DockFloatingId, DockGroupId, DockItemId, DockLayoutRect, DockSpaceId, SplitAxis};
use std::collections::BTreeSet;

/// Prepared-patch owner for the canonical graph transaction boundary.
///
/// This engine retains receipts in generation-indexed slots and never retains a complete graph. It
/// reserves each receipt slot while preparing a symbolic local patch; the caller supplies the write
/// target at commit time.
#[derive(Debug, Default)]
pub(crate) struct GraphTransactionEngine {
    next_generation: u64,
    receipts: Vec<Option<Box<GraphReceiptReservation>>>,
}

impl GraphTransactionEngine {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn reserve_commit_id(
        &mut self,
        revision: GraphRevision,
    ) -> Result<GraphCommitId, GraphPrepareError> {
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(GraphPrepareError::CommitIdExhausted)?;
        let slot =
            usize::try_from(generation - 1).map_err(|_| GraphPrepareError::CommitIdExhausted)?;
        debug_assert_eq!(slot, self.receipts.len());
        self.receipts
            .try_reserve(1)
            .map_err(|_| GraphPrepareError::CommitReceiptCapacityExhausted)?;
        self.receipts.push(None);
        self.next_generation = generation;
        Ok(GraphCommitId::new(revision.authority_epoch(), generation))
    }

    fn receipt(&self, commit_id: GraphCommitId) -> Option<&GraphCommitReceipt> {
        let slot = usize::try_from(commit_id.generation().checked_sub(1)?).ok()?;
        self.receipts
            .get(slot)?
            .as_deref()?
            .receipt()
            .filter(|receipt| receipt.commit_id() == commit_id)
    }

    fn install_receipt(&mut self, reservation: Box<GraphReceiptReservation>) {
        let commit_id = reservation
            .receipt()
            .expect("committed receipt reservation must be filled")
            .commit_id();
        let slot = usize::try_from(commit_id.generation() - 1)
            .expect("prepared receipt slot must fit in usize");
        let stored = self
            .receipts
            .get_mut(slot)
            .expect("prepared patch must own a reserved receipt slot");
        debug_assert!(stored.is_none());
        *stored = Some(reservation);
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
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = GraphReadSet::for_select_tab(revision, current);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = GraphReadSet::for_groups(revision, [target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = GraphReadSet::for_groups(revision, [source, target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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

        let commit_id = self.reserve_commit_id(revision)?;
        let read_set =
            GraphReadSet::for_group_state_sensitive_operation(revision, [source, target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = GraphReadSet::for_group_and_space(revision, source, target);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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

        let commit_id = self.reserve_commit_id(revision)?;
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
                commit_id,
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
        if !source.items.contains(&item) {
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

        let commit_id = self.reserve_commit_id(revision)?;
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
                commit_id,
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
        if !source.items.contains(&item) {
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

        let source_state_sensitive = source.items.len() > 1;
        let operation = GraphPatchOperation::MoveItemToEmptySpace {
            item,
            source_group,
            new_group,
            source_space,
            target_space,
        };
        let revision = view.graph_revision();
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = if source_state_sensitive {
            GraphReadSet::for_group_and_space_state_sensitive(revision, source, target)
        } else {
            GraphReadSet::for_group_and_space(revision, source, target)
        };
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
                GraphPatch::new(operation, read_set),
            ),
        ))
    }

    /// Prepares extracting one item into a new first-class floating root.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_float_item_in_window<T: GraphReadView>(
        &mut self,
        view: &T,
        item: impl Into<DockItemId>,
        new_group: impl Into<DockGroupId>,
        floating: impl Into<DockFloatingId>,
        source_space: impl Into<DockSpaceId>,
        target_space: impl Into<DockSpaceId>,
        bounds: DockLayoutRect,
    ) -> Result<GraphPrepareOutcome, GraphPrepareError> {
        let item = item.into();
        let new_group = new_group.into();
        let floating = floating.into();
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
        if !source.items.contains(&item) {
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
        if floating.as_str().is_empty() {
            return Err(GraphPrepareError::EmptyFloatingId);
        }
        if view.read_floating_root(&floating).is_some() {
            return Err(GraphPrepareError::FloatingAlreadyExists { floating });
        }
        let bounds =
            GraphFloatingBounds::new(bounds).ok_or(GraphPrepareError::InvalidFloatingBounds)?;
        let retired_floating = (source.items.len() == 1 && source.is_owner_root)
            .then(|| source.floating.clone())
            .flatten();
        let excluded_from_target = (source_space == target_space)
            .then_some(retired_floating.as_ref())
            .flatten();
        let z_order = target
            .next_floating_z_order_excluding(excluded_from_target)
            .ok_or_else(|| GraphPrepareError::FloatingZOrderExhausted {
                space: target_space.clone(),
            })?;

        let source_state_sensitive = source.items.len() > 1;
        let operation = GraphPatchOperation::FloatItemInWindow {
            item,
            source_group,
            new_group,
            floating,
            source_space,
            target_space,
            retired_floating,
            bounds,
            z_order,
        };
        let revision = view.graph_revision();
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = if source_state_sensitive {
            GraphReadSet::for_group_and_space_state_sensitive(revision, source, target)
        } else {
            GraphReadSet::for_group_and_space(revision, source, target)
        };
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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
        if target.floating.as_ref() == Some(&floating) {
            return Err(GraphPrepareError::FloatingTargetInsidePayload {
                floating,
                group: target_group,
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
        let commit_id = self.reserve_commit_id(revision)?;
        let read_set = GraphReadSet::for_floating_and_groups(revision, floating_read, [target]);
        Ok(GraphPrepareOutcome::Prepared(
            crate::graph_patch::PreparedGraphPatch::new(
                commit_id,
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
        if let Some(receipt) = self.receipt(commit_id) {
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

        let (operation, mut receipt_reservation) = prepared.into_commit_parts();
        let applied = match operation.as_ref() {
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
            } => target.apply_float_item_in_window(
                item,
                source_group,
                new_group,
                floating,
                source_space,
                target_space,
                retired_floating.as_ref(),
                bounds.get(),
                *z_order,
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
        let after = applied.after();
        let receipt =
            GraphCommitReceipt::new(commit_id, before, after, operation, applied.into_effects());
        receipt_reservation.install(receipt.clone());
        self.install_receipt(receipt_reservation);
        GraphCommitOutcome::Committed(receipt)
    }

    /// Replays a retained receipt without touching the target.
    pub(crate) fn replay(&self, commit_id: GraphCommitId) -> Option<GraphCommitOutcome> {
        self.receipt(commit_id)
            .cloned()
            .map(GraphCommitOutcome::AlreadyCommitted)
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts
            .iter()
            .filter(|receipt| receipt.is_some())
            .count()
    }
}
