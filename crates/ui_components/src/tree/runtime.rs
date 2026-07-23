use std::collections::BTreeMap;

use open_gpui::{
    Context, FocusClaimOutcome, FocusHandle, RevealTargetHandle, ScrollChainFence, Window,
};

use crate::collection_typeahead::CollectionTypeaheadSession;
use crate::scroll_surface::ScrollSurfaceRuntime;

use super::TreeState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TreeFocusOperationStage {
    Materializing,
    AwaitingMount,
    InFlight,
}

#[derive(Debug, Clone)]
pub(super) struct TreeFocusOperation {
    pub(super) sequence: u64,
    pub(super) value: String,
    pub(super) stage: TreeFocusOperationStage,
    focus_revision: u64,
    pub(super) claim_revision: Option<u64>,
    materialization_fence: Option<ScrollChainFence>,
    retried_after_rejection: bool,
}

impl TreeFocusOperation {
    pub(super) fn materialization_fence(&self) -> Option<ScrollChainFence> {
        self.materialization_fence.clone()
    }
}

fn may_retry_rejected_focus(
    operation: &TreeFocusOperation,
    outcome: FocusClaimOutcome,
    current_claim_revision: u64,
) -> bool {
    outcome == FocusClaimOutcome::Rejected
        && !operation.retried_after_rejection
        && operation.claim_revision == Some(current_claim_revision)
}

#[derive(Debug, Clone, Default)]
pub(super) struct TreeRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) selected_value: Option<String>,
    pub(super) focused_value: Option<String>,
    pub(super) expanded_values: BTreeMap<String, bool>,
    pub(super) focus_handles: BTreeMap<String, FocusHandle>,
    scroll_chain_anchor: Option<RevealTargetHandle>,
    pending_focus: Option<TreeFocusOperation>,
    next_focus_sequence: u64,
    virtualized: bool,
    pub(super) typeahead: CollectionTypeaheadSession,
}

impl TreeRuntime {
    pub(super) fn new(
        scroll_surface: ScrollSurfaceRuntime,
        selected_value: Option<String>,
        focused_value: Option<String>,
    ) -> Self {
        Self {
            scroll_surface,
            selected_value,
            focused_value,
            ..Self::default()
        }
    }

    pub(super) fn sync(&mut self, state: &TreeState, cx: &mut Context<Self>) {
        self.focus_handles.retain(|value, _| {
            state
                .item_by_value(value)
                .is_some_and(|item| item.focusable())
        });

        for item in state.items().iter().filter(|item| item.focusable()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        if self.pending_focus.as_ref().is_some_and(|operation| {
            !state
                .item_by_value(&operation.value)
                .is_some_and(|item| item.focusable())
        }) {
            self.pending_focus = None;
        }

        self.selected_value = state.selected_value().map(str::to_owned);
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    pub(super) fn set_virtualized(&mut self, virtualized: bool) {
        self.virtualized = virtualized;
    }

    pub(super) const fn is_virtualized(&self) -> bool {
        self.virtualized
    }

    pub(super) fn scroll_chain_anchor(&mut self, window: &mut Window) -> RevealTargetHandle {
        let window_id = window.window_handle().window_id();
        if !self
            .scroll_chain_anchor
            .is_some_and(|anchor| anchor.window_id() == window_id)
        {
            self.scroll_chain_anchor = Some(window.new_reveal_target());
        }
        self.scroll_chain_anchor
            .expect("Tree scroll-chain anchor should be initialized for this window")
    }

    pub(super) fn set_focused(
        &mut self,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        self.pending_focus = None;
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }

    pub(super) fn queue_focus(
        &mut self,
        value: &str,
        focus_revision: u64,
        materialization_fence: Option<ScrollChainFence>,
        cx: &mut Context<Self>,
    ) -> Option<(u64, FocusHandle)> {
        let focus_handle = self.focus_handles.get(value)?.clone();
        if self.virtualized && materialization_fence.is_none() {
            return None;
        }
        self.next_focus_sequence = self
            .next_focus_sequence
            .checked_add(1)
            .expect("tree focus sequence exhausted");
        self.focused_value = Some(value.to_owned());
        self.pending_focus = Some(TreeFocusOperation {
            sequence: self.next_focus_sequence,
            value: value.to_owned(),
            stage: TreeFocusOperationStage::Materializing,
            focus_revision,
            claim_revision: None,
            materialization_fence,
            retried_after_rejection: false,
        });
        cx.notify();
        (!self.virtualized).then_some((self.next_focus_sequence, focus_handle))
    }

    pub(super) fn bind_focus_claim(&mut self, sequence: u64, claim_revision: u64) -> bool {
        let Some(operation) = self.pending_focus.as_mut() else {
            return false;
        };
        if operation.sequence != sequence {
            return false;
        }
        operation.claim_revision = Some(claim_revision);
        operation.stage = TreeFocusOperationStage::InFlight;
        true
    }

    pub(super) fn retain_current_focus_claim(&mut self, claim_revision: u64) {
        if self.pending_focus.as_ref().is_some_and(|operation| {
            operation.claim_revision.unwrap_or(operation.focus_revision) != claim_revision
        }) {
            self.pending_focus = None;
        }
    }

    pub(super) fn pending_focus(&self, claim_revision: u64) -> Option<TreeFocusOperation> {
        self.pending_focus
            .as_ref()
            .filter(|operation| {
                operation.claim_revision.unwrap_or(operation.focus_revision) == claim_revision
            })
            .cloned()
    }

    pub(super) fn prepare_virtual_materialization(
        &mut self,
        sequence: u64,
        current_focus_revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(operation) = self.pending_focus.as_ref() else {
            return false;
        };
        let is_current = operation.sequence == sequence
            && operation.stage == TreeFocusOperationStage::Materializing
            && operation.claim_revision.unwrap_or(operation.focus_revision)
                == current_focus_revision;
        if !is_current {
            return false;
        }
        let Some(fence) = operation.materialization_fence.as_ref() else {
            self.pending_focus = None;
            cx.notify();
            return false;
        };
        if window.scroll_chain_fence_was_interrupted(fence)
            || !window.scroll_chain_fence_matches_current_ancestry(fence)
        {
            self.pending_focus = None;
            cx.notify();
            return false;
        }
        true
    }

    pub(super) fn commit_virtual_materialization(
        &mut self,
        sequence: u64,
        current_focus_revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(operation) = self.pending_focus.as_ref() else {
            return false;
        };
        let is_current = operation.sequence == sequence
            && operation.stage == TreeFocusOperationStage::Materializing
            && operation.claim_revision.unwrap_or(operation.focus_revision)
                == current_focus_revision;
        let fence_is_valid = operation
            .materialization_fence
            .as_ref()
            .is_some_and(|fence| !window.scroll_chain_fence_was_interrupted(fence));
        if !is_current || !fence_is_valid {
            if is_current {
                self.pending_focus = None;
                cx.notify();
            }
            return false;
        }

        self.pending_focus
            .as_mut()
            .expect("matching Tree focus operation should remain present")
            .stage = TreeFocusOperationStage::AwaitingMount;
        true
    }

    pub(super) fn refresh_virtual_materialization_fence(
        &mut self,
        sequence: u64,
        fence: ScrollChainFence,
    ) -> bool {
        let Some(operation) = self.pending_focus.as_mut() else {
            return false;
        };
        if operation.sequence != sequence
            || operation.stage != TreeFocusOperationStage::AwaitingMount
        {
            return false;
        }
        operation.materialization_fence = Some(fence);
        true
    }

    pub(super) fn prepare_virtual_focus_submission(
        &mut self,
        sequence: u64,
        current_focus_revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(operation) = self.pending_focus.as_ref() else {
            return false;
        };
        let is_current = operation.sequence == sequence
            && operation.stage == TreeFocusOperationStage::AwaitingMount
            && operation.claim_revision.unwrap_or(operation.focus_revision)
                == current_focus_revision;
        if !is_current {
            return false;
        }
        let Some(fence) = operation.materialization_fence.as_ref() else {
            self.pending_focus = None;
            cx.notify();
            return false;
        };
        if window.scroll_chain_fence_was_interrupted(fence)
            || !window.scroll_chain_fence_matches_current_ancestry(fence)
        {
            self.pending_focus = None;
            cx.notify();
            return false;
        }
        true
    }

    pub(super) fn take_virtual_focus_submission(
        &mut self,
        sequence: u64,
        current_focus_revision: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<(FocusHandle, ScrollChainFence)> {
        let Some(operation) = self.pending_focus.as_ref() else {
            return None;
        };
        let is_current = operation.sequence == sequence
            && operation.stage == TreeFocusOperationStage::AwaitingMount
            && operation.claim_revision.unwrap_or(operation.focus_revision)
                == current_focus_revision;
        let fence = operation.materialization_fence.as_ref().cloned();
        let fence_is_valid = fence
            .as_ref()
            .is_some_and(|fence| !window.scroll_chain_fence_was_interrupted(fence));
        if !is_current || !fence_is_valid {
            if is_current {
                self.pending_focus = None;
                cx.notify();
            }
            return None;
        }
        let Some(handle) = self.focus_handles.get(&operation.value).cloned() else {
            self.pending_focus = None;
            cx.notify();
            return None;
        };
        self.pending_focus
            .as_mut()
            .expect("matching Tree focus operation should remain present")
            .stage = TreeFocusOperationStage::InFlight;
        Some((
            handle,
            fence.expect("validated Tree focus submission should retain its scroll fence"),
        ))
    }

    pub(super) fn abandon_virtual_focus(&mut self, sequence: u64, cx: &mut Context<Self>) {
        if self
            .pending_focus
            .as_ref()
            .is_some_and(|operation| operation.sequence == sequence)
        {
            self.pending_focus = None;
            cx.notify();
        }
    }

    pub(super) fn finish_focus(
        &mut self,
        sequence: u64,
        outcome: FocusClaimOutcome,
        current_claim_revision: u64,
        cx: &mut Context<Self>,
    ) -> Option<(u64, FocusHandle, Option<ScrollChainFence>)> {
        let Some(operation) = self.pending_focus.as_ref() else {
            return None;
        };
        if operation.sequence != sequence {
            return None;
        }

        if operation.claim_revision != Some(current_claim_revision) {
            self.pending_focus = None;
            return None;
        }

        if may_retry_rejected_focus(operation, outcome, current_claim_revision) {
            let value = operation.value.clone();
            let operation = self
                .pending_focus
                .as_mut()
                .expect("matching Tree focus operation should remain present");
            operation.stage = TreeFocusOperationStage::Materializing;
            operation.focus_revision = current_claim_revision;
            operation.claim_revision = None;
            operation.retried_after_rejection = true;
            let retry_fence = operation.materialization_fence();
            cx.notify();
            if self.virtualized {
                None
            } else {
                self.focus_handles
                    .get(&value)
                    .cloned()
                    .map(|handle| (sequence, handle, retry_fence))
            }
        } else {
            self.pending_focus = None;
            None
        }
    }

    pub(super) fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) {
        let changed = self.selected_value.as_deref() != Some(value);
        self.selected_value = Some(value.to_owned());
        if changed {
            cx.notify();
        }
    }

    pub(super) fn set_expanded(&mut self, value: &str, expanded: bool, cx: &mut Context<Self>) {
        let changed = self.expanded_values.get(value).copied() != Some(expanded);
        self.expanded_values.insert(value.to_owned(), expanded);
        if changed {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_focus_retry_requires_the_current_window_claim_revision() {
        let operation = TreeFocusOperation {
            sequence: 7,
            value: "target".to_owned(),
            stage: TreeFocusOperationStage::InFlight,
            focus_revision: 12,
            claim_revision: Some(12),
            materialization_fence: None,
            retried_after_rejection: false,
        };

        assert!(may_retry_rejected_focus(
            &operation,
            FocusClaimOutcome::Rejected,
            12,
        ));
        assert!(!may_retry_rejected_focus(
            &operation,
            FocusClaimOutcome::Rejected,
            13,
        ));
        assert!(!may_retry_rejected_focus(
            &operation,
            FocusClaimOutcome::Superseded,
            12,
        ));
    }
}
