use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockController, DockItemId, DockSpaceId,
    DockViewportAdapter, DockViewportCloseOutcome, DockViewportClosePolicy,
    DockViewportOpenOutcome, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreOutcome, DockViewportRuntimeHandle, DockViewportShouldCloseOutcome,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason, DockViewportTearOffCancelled,
    DockViewportTearOffCommitFailure, DockViewportTearOffCompleted,
    DockViewportTearOffCompletionOutcome, DockViewportTearOffCompletionPending,
    DockViewportTearOffMachine, DockViewportTearOffOpenOutcome, DockViewportTearOffPending,
    DockViewportTearOffRequest, DockViewportTearOffTick,
    viewport_close_gate::DockViewportCloseGate,
};
use open_gpui::{AnyWindowHandle, App, Entity, Result, WindowId, WindowOptions};
use std::rc::Rc;

/// Owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so applications do not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// coordinate snapshots, and placement import/export.
pub struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_gate: DockViewportCloseGate,
    tear_off: DockViewportTearOffMachine,
    tear_off_tick: DockViewportTearOffTick,
}

fn install_should_close_hook(
    window: AnyWindowHandle,
    cx: &mut App,
    should_close: Rc<dyn Fn(WindowId) -> bool>,
) -> Result<()> {
    let window_id = window.window_id();
    window.update(cx, move |_, window, cx| {
        window.on_window_should_close(cx, move |_, _| should_close(window_id));
    })
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy(controller, DockViewportClosePolicy::default())
    }

    /// Creates a runtime with an explicit close policy.
    pub fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter: DockViewportAdapter::new(),
            close_gate: DockViewportCloseGate::new(close_policy),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
        }
    }

    /// Creates a runtime from an existing adapter.
    pub fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        let close_gate = DockViewportCloseGate::new(close_policy);
        close_gate.sync_adapter(&adapter);
        Self {
            controller,
            adapter,
            close_gate,
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    /// Returns the shared docking controller.
    pub fn controller(&self) -> &Entity<DockController> {
        &self.controller
    }

    /// Returns the low-level viewport adapter.
    pub fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_gate.close_policy()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_gate.set_close_policy(close_policy);
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// Runtime-opened windows install a GPUI should-close hook so
    /// [`DockViewportClosePolicy::Prevent`] can veto a platform close before
    /// [`Self::handle_window_closed`] performs post-close cleanup.
    pub fn open_viewport(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let close_gate = self.close_gate.clone();
        self.open_viewport_with_should_close(space, options, cx, move |window_id| {
            close_gate.should_allow_close(window_id)
        })
    }

    /// Returns pending tear-off item ids in stable order.
    pub fn pending_tear_off_items(&self) -> Vec<DockItemId> {
        self.tear_off.pending_items()
    }

    /// Returns the number of pending tear-off transactions.
    pub fn pending_tear_off_len(&self) -> usize {
        self.tear_off.len()
    }

    /// Returns the pending tear-off request for an item.
    pub fn pending_tear_off(&self, item: &DockItemId) -> Option<&DockViewportTearOffPending> {
        self.tear_off.pending(item)
    }

    /// Records a tear-off request without opening a window yet.
    ///
    /// This supports platform integrations where the release path requests a window and a later
    /// window-created callback completes the transaction.
    pub fn begin_tear_off_request(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
    ) -> DockViewportTearOffBeginOutcome {
        let now = self.next_tear_off_tick();
        self.begin_tear_off_request_at(request, target_space, now)
    }

    /// Records a tear-off request at an explicit logical clock value.
    pub fn begin_tear_off_request_at(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.tear_off.begin(request, target_space.into(), now)
    }

    /// Cancels a pending tear-off request for an item.
    pub fn cancel_tear_off_request(
        &mut self,
        item: &DockItemId,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.tear_off.cancel(item, reason)
    }

    /// Removes stale pending tear-off requests at an explicit logical clock value.
    pub fn expire_tear_off_requests_at(
        &mut self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        self.tear_off.expire(now)
    }

    /// Completes a pending tear-off request after a platform viewport window exists.
    ///
    /// The runtime validates that the source item still belongs to its recorded source tabs,
    /// registers the destination viewport, commits the graph move, and clears pending state.
    pub fn complete_tear_off_viewport(
        &mut self,
        item: &DockItemId,
        window: impl Into<AnyWindowHandle>,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let now = self.next_tear_off_tick();
        self.complete_tear_off_viewport_at(item, window, now, cx)
    }

    /// Completes a pending tear-off request at an explicit logical clock value.
    pub fn complete_tear_off_viewport_at(
        &mut self,
        item: &DockItemId,
        window: impl Into<AnyWindowHandle>,
        now: DockViewportTearOffTick,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let item = item.clone();
        let readiness = self.prepare_tear_off_completion(&item, now, cx);
        let pending = match readiness {
            DockViewportTearOffCompletionPending::Pending(pending) => pending,
            DockViewportTearOffCompletionPending::Cancelled(cancelled) => {
                return DockViewportTearOffCompletionOutcome::Cancelled(cancelled);
            }
            DockViewportTearOffCompletionPending::Missing => {
                return DockViewportTearOffCompletionOutcome::MissingPending { item };
            }
        };

        let registration = self
            .adapter
            .register_viewport_with_outcome(pending.target_space.clone(), window);
        self.close_gate.sync_adapter(&self.adapter);
        match self.commit_tear_off_move(&pending, cx) {
            Ok(action) => {
                DockViewportTearOffCompletionOutcome::Completed(DockViewportTearOffCompleted {
                    pending,
                    registration,
                    action,
                })
            }
            Err(error) => {
                self.adapter.unregister_space(&pending.target_space);
                self.close_gate.sync_adapter(&self.adapter);
                DockViewportTearOffCompletionOutcome::CommitFailed(
                    DockViewportTearOffCommitFailure {
                        pending,
                        registration,
                        error,
                    },
                )
            }
        }
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    ///
    /// The graph is not mutated until the destination viewport has opened and registered
    /// successfully. Duplicate requests for the same item are idempotent and do not open another
    /// window.
    pub fn open_tear_off_viewport(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let item = request.item.clone();
        let begin = self.begin_tear_off_request(request, target_space);
        let pending = match begin {
            DockViewportTearOffBeginOutcome::Pending(pending) => pending,
            DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                return Ok(DockViewportTearOffOpenOutcome::Duplicate(pending));
            }
        };

        let opened = match self.open_viewport(pending.target_space.clone(), options, cx) {
            Ok(opened) => opened,
            Err(error) => {
                self.tear_off
                    .cancel(&item, DockViewportTearOffCancelReason::Cancelled);
                return Err(error);
            }
        };

        Ok(
            match self.complete_tear_off_viewport(&item, opened.window, cx) {
                DockViewportTearOffCompletionOutcome::Completed(completed) => {
                    DockViewportTearOffOpenOutcome::Completed(completed)
                }
                DockViewportTearOffCompletionOutcome::Cancelled(cancelled) => {
                    self.adapter.unregister_space(&pending.target_space);
                    self.close_gate.sync_adapter(&self.adapter);
                    DockViewportTearOffOpenOutcome::Cancelled(cancelled)
                }
                DockViewportTearOffCompletionOutcome::MissingPending { item } => {
                    DockViewportTearOffOpenOutcome::Cancelled(DockViewportTearOffCancelled {
                        pending,
                        reason: if self.controller.read(cx).graph().contains_item(&item) {
                            DockViewportTearOffCancelReason::SourceMoved
                        } else {
                            DockViewportTearOffCancelReason::SourceMissing
                        },
                    })
                }
                DockViewportTearOffCompletionOutcome::CommitFailed(failure) => {
                    DockViewportTearOffOpenOutcome::CommitFailed(failure)
                }
            },
        )
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    pub fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let outcome = self.adapter.handle_window_closed(window_id);
        self.close_gate.sync_adapter(&self.adapter);
        outcome
    }

    /// Handles a GPUI window should-close query by applying this runtime's close policy.
    pub fn handle_window_should_close(
        &self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        self.adapter
            .should_close_viewport(window_id, self.close_policy())
    }

    pub(crate) fn open_viewport_with_should_close(
        &mut self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
        should_close: impl Fn(WindowId) -> bool + 'static,
    ) -> Result<DockViewportOpenOutcome> {
        let should_close = Rc::new(should_close);
        let outcome = self
            .adapter
            .open_viewport(self.controller.clone(), space, options, cx);
        self.close_gate.sync_adapter(&self.adapter);
        let outcome = outcome?;
        if let Err(error) = install_should_close_hook(outcome.window, cx, should_close) {
            self.adapter
                .handle_window_closed(outcome.window.window_id());
            self.close_gate.sync_adapter(&self.adapter);
            return Err(error);
        }

        Ok(outcome)
    }

    fn next_tear_off_tick(&mut self) -> DockViewportTearOffTick {
        let tick = self.tear_off_tick;
        self.tear_off_tick = self.tear_off_tick.saturating_add(1);
        tick
    }

    fn prepare_tear_off_completion(
        &mut self,
        item: &DockItemId,
        now: DockViewportTearOffTick,
        cx: &App,
    ) -> DockViewportTearOffCompletionPending {
        let Some(pending) = self.tear_off.pending(item).cloned() else {
            return DockViewportTearOffCompletionPending::Missing;
        };
        if pending.is_expired_at(now) {
            return DockViewportTearOffCompletionPending::Cancelled(
                self.tear_off
                    .cancel(item, DockViewportTearOffCancelReason::Expired)
                    .expect("pending item should still be present"),
            );
        }

        match self.tear_off_source_status(&pending, cx) {
            DockViewportTearOffSourceStatus::Ready => self.tear_off.take_for_completion(item, now),
            DockViewportTearOffSourceStatus::Missing => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(item, DockViewportTearOffCancelReason::SourceMissing)
                        .expect("pending item should still be present"),
                )
            }
            DockViewportTearOffSourceStatus::Moved => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(item, DockViewportTearOffCancelReason::SourceMoved)
                        .expect("pending item should still be present"),
                )
            }
        }
    }

    fn tear_off_source_status(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &App,
    ) -> DockViewportTearOffSourceStatus {
        let graph = self.controller.read(cx).graph();
        graph
            .find_item_in_space(&pending.request.source_space, &pending.request.item)
            .map(|(tabs, _)| {
                if tabs == pending.request.source_tabs {
                    DockViewportTearOffSourceStatus::Ready
                } else {
                    DockViewportTearOffSourceStatus::Moved
                }
            })
            .unwrap_or_else(|| {
                if graph.contains_item(&pending.request.item) {
                    DockViewportTearOffSourceStatus::Moved
                } else {
                    DockViewportTearOffSourceStatus::Missing
                }
            })
    }

    fn commit_tear_off_move(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.controller.update(cx, |controller, cx| {
            let outcome = controller.apply_action(&DockAction::MoveItemToEmptyDockSpace {
                source_space: pending.request.source_space.clone(),
                item: pending.request.item.clone(),
                target_space: pending.target_space.clone(),
            });
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })
    }

    /// Exports serializable placement snapshots from the adapter.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Applies saved placement snapshots to registered viewport windows.
    pub fn apply_placement(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreOutcome, DockViewportPlacementValidationError> {
        self.adapter.apply_placement(placement)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockViewportTearOffSourceStatus {
    Ready,
    Missing,
    Moved,
}
