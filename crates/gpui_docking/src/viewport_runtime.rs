#[cfg(test)]
use crate::drop_runtime::DockHostDropScene;
use crate::viewport_close::{DockViewportCloseFinalizeKey, DockViewportShouldCloseFinalizeKey};
#[cfg(test)]
use crate::viewport_registry::DockViewportRouteUnavailableReason;
pub(crate) use crate::viewport_tear_off_placement::{
    suggested_tear_off_window_bounds, suggested_tear_off_window_bounds_from_native_frame,
};
#[cfg(test)]
use crate::viewport_window_lifecycle::DockViewportReusableWindow;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockDropDelivery,
    DockDropWorkspaceCommit, DockGraphMutationError, DockItemId, DockMergeBackTarget, DockNodeId,
    DockPreparedProvisionalWindowPromotion, DockSpaceId, DockViewportActivationBackendFocusApply,
    DockViewportActivationBackendFocusObservation, DockViewportActivationBackendFocusRecordEffect,
    DockViewportActivationPendingBackendFocusEffect, DockViewportActivationTransaction,
    DockViewportAdapter, DockViewportBackendFocusState, DockViewportCloseCoordinator,
    DockViewportCloseOutcome, DockViewportClosePlanState, DockViewportClosePolicy,
    DockViewportCloseStatus, DockViewportCommittedTearOffMove, DockViewportDropActionOutcome,
    DockViewportDropPayload, DockViewportDropRouteOutcome, DockViewportFocusCoordinator,
    DockViewportFocusRequest, DockViewportFrameCoordinator, DockViewportHostGeometry,
    DockViewportIdentity, DockViewportMergeBackClosePlan, DockViewportPayloadDragState,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportPlatformFocusRestoreGate, DockViewportPlatformFocusRestorePolicy,
    DockViewportPlatformSyncRecord, DockViewportProvisionalOpenAttemptCompletion,
    DockViewportRegisterOutcome, DockViewportRestoreReadiness, DockViewportRoutedDropPreviewState,
    DockViewportRuntimeAdmission, DockViewportRuntimeHandle,
    DockViewportRuntimeLineageActivationOutcome, DockViewportRuntimeLineageFreezeOutcome,
    DockViewportRuntimeStatus, DockViewportRuntimeUpdate, DockViewportRuntimeWorkContext,
    DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus,
    DockViewportSurfaceShutdownEffects, DockViewportSurfaceShutdownReservation,
    DockViewportTearOffBeginOutcome, DockViewportTearOffCancelReason, DockViewportTearOffCancelled,
    DockViewportTearOffCompleted, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffPlacement, DockViewportTearOffPlacementPolicy,
    DockViewportTearOffRequest, DockViewportTearOffSourceStatus, DockViewportWindowAuthority,
    DockViewportWindowCloseEffect, DockViewportWindowEffects, DockViewportWindowFacts,
    DockViewportWindowOpenAttemptKey, DockViewportWindowOwnership, DockViewportWindowRetirement,
    DockViewportWindowRetirementKey, DockViewportWindowRole, DockViewportWorkspaceRouteFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    extend_unique_windows,
    interaction::DockRuntimeDragSession,
    surface::DockSurfaceTransactionId,
    viewport_coordinates::{DockViewportFrameSample, DockViewportFrameSampleRequest},
    viewport_drop_scene::{
        DockViewportHostSceneDraft, DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneSnapshot,
    },
    viewport_registry::{
        DockViewportPlatformRequests, DockViewportPreparedVacantRegistration,
        DockViewportRegistrationKey,
    },
    viewport_window_lifecycle::{
        DockViewportCloseRecoveryActivation, DockViewportClosedWindowRefresh,
        DockViewportReplacementCleanup, DockViewportReusableWindowOutcome,
        DockViewportRuntimeWindowStateCleanup, DockViewportShouldCloseRefresh,
        DockViewportSpaceFocusCleanup, DockViewportUnregisteredSpace,
        DockViewportVacatedTearOffSource, DockViewportWindowLifecycleController,
    },
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
    workspace_drop_transaction::{
        DockWorkspaceLockedPayloadDropPlan, DockWorkspaceLockedPayloadDropRequest,
        DockWorkspacePayloadDropOutcome, DockWorkspacePayloadDropRequest,
        DockWorkspacePreparedLockedPayloadDrop,
    },
};
use open_gpui::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, PlatformFocusedWindow, Point, WindowId,
    WindowOptions,
};
use std::{cell::Cell, collections::HashMap, rc::Rc};

mod drop_resolution;
mod routed_preview;

/// Internal owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`model::DockController`](crate::model::DockController) together
/// with the low-level [`DockViewportAdapter`] so the handle does not have to pass the controller
/// into every open call or duplicate close-callback cleanup logic. The adapter remains the place
/// for window mappings, live window facts, and placement import/export.
#[derive(Debug)]
pub(crate) struct DockViewportRuntime {
    controller: Entity<DockController>,
    admission: Rc<Cell<DockViewportRuntimeAdmission>>,
    adapter: DockViewportAdapter,
    close_policy: DockViewportClosePolicy,
    visual_style_resolver: Option<crate::DockVisualStyleResolver>,
    frame_coordinator: DockViewportFrameCoordinator,
    tear_off: DockViewportTearOffMachine,
    tear_off_target_reservations: HashMap<DockSpaceId, DockViewportTearOffTargetReservation>,
    next_tear_off_space_index: u64,
    payload_drag: DockViewportPayloadDragState,
    window_ownership: DockViewportWindowOwnership,
    focus: DockViewportFocusCoordinator,
    backend_focus: DockViewportBackendFocusState,
    backend_focus_cancellations: Vec<DockViewportActivationTransaction>,
    close_coordinator: DockViewportCloseCoordinator,
    routed_drop_preview: DockViewportRoutedDropPreviewState,
    status: DockViewportRuntimeStatus,
    #[cfg(test)]
    reject_next_provisional_registration: bool,
    #[cfg(test)]
    reject_next_live_undock_promotion_commit: Cell<bool>,
}

#[derive(Debug)]
pub(crate) struct DockViewportRuntimeRegistration {
    pub(crate) outcome: DockViewportRegisterOutcome,
    window_effects: DockViewportWindowEffects,
    runtime_update: DockViewportRuntimeUpdate,
}

pub(crate) struct DockViewportPreparedLiveUndockPromotion {
    target_space: DockSpaceId,
    window: AnyWindowHandle,
    context: DockViewportRuntimeWorkContext,
    window_facts: crate::DockViewportWindowFacts,
    host_geometry: Option<crate::DockViewportHostGeometry>,
    ownership: DockPreparedProvisionalWindowPromotion,
    registration: DockViewportPreparedVacantRegistration,
}

impl DockViewportPreparedLiveUndockPromotion {
    pub(crate) fn registration(&self) -> &DockViewportRegistrationKey {
        self.registration.registration()
    }

    pub(crate) fn with_host_geometry(
        mut self,
        host_geometry: crate::DockViewportHostGeometry,
    ) -> Self {
        self.host_geometry = Some(host_geometry);
        self
    }
}

pub(crate) struct DockViewportCommittedLiveUndockPromotion {
    pub(crate) registration: DockViewportRegistrationKey,
    pub(crate) runtime_update: DockViewportRuntimeUpdate,
}

#[derive(Debug, Clone, PartialEq)]
struct DockViewportTearOffTargetReservation {
    pending: DockViewportTearOffPending,
    opening_window: Option<AnyWindowHandle>,
}

pub(crate) struct DockViewportPreparedReusableWindow {
    state: DockViewportReusableWindowProbe,
}

enum DockViewportReusableWindowProbe {
    Missing,
    Stale,
    Candidate {
        key: DockViewportRegistrationKey,
        window: AnyWindowHandle,
        known_live: bool,
    },
}

pub(crate) struct DockViewportAppliedReusableWindow {
    state: DockViewportReusableWindowObservation,
}

enum DockViewportReusableWindowObservation {
    Missing,
    Stale,
    Candidate {
        key: DockViewportRegistrationKey,
        window: AnyWindowHandle,
        live: bool,
    },
}

impl DockViewportPreparedReusableWindow {
    pub(crate) fn sample(self, cx: &mut App) -> DockViewportAppliedReusableWindow {
        let state = match self.state {
            DockViewportReusableWindowProbe::Missing => {
                DockViewportReusableWindowObservation::Missing
            }
            DockViewportReusableWindowProbe::Stale => DockViewportReusableWindowObservation::Stale,
            DockViewportReusableWindowProbe::Candidate {
                key,
                window,
                known_live,
            } => DockViewportReusableWindowObservation::Candidate {
                key,
                window,
                live: known_live || window.update(cx, |_, _, _| ()).is_ok(),
            },
        };
        DockViewportAppliedReusableWindow { state }
    }
}

pub(crate) struct DockViewportPreparedPayloadDrop {
    admission: Rc<Cell<DockViewportRuntimeAdmission>>,
    work_context: DockViewportRuntimeWorkContext,
    controller: Entity<DockController>,
    source_space: DockSpaceId,
    target: DockViewportPreparedPayloadDropTarget,
    target_space: DockSpaceId,
    drag_session: DockRuntimeDragSession,
    target_window: DockViewportPreparedReusableWindow,
    source_registration: Option<DockViewportRegistrationKey>,
}

enum DockViewportPreparedPayloadDropTarget {
    Resolved {
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        target: DockWorkspaceResolvedDropTarget,
    },
    Locked(DockWorkspaceLockedPayloadDropPlan),
}

pub(crate) struct DockViewportLockedWorkspaceDrop {
    plan: DockWorkspaceLockedPayloadDropPlan,
    drag_session: DockRuntimeDragSession,
}

#[must_use = "an atomic locked payload drop must be sampled before preflight"]
pub(crate) struct DockViewportPreparedAtomicLockedPayloadDrop {
    admission: Rc<Cell<DockViewportRuntimeAdmission>>,
    work_context: DockViewportRuntimeWorkContext,
    controller: Entity<DockController>,
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    drag_session: DockRuntimeDragSession,
    plan: DockWorkspaceLockedPayloadDropPlan,
    target_window: DockViewportPreparedReusableWindow,
    expected_target_window: AnyWindowHandle,
    source_registration: Option<DockViewportRegistrationKey>,
}

#[must_use = "a sampled locked payload drop must complete runtime preflight"]
pub(crate) struct DockViewportSampledAtomicLockedPayloadDrop {
    admission: Rc<Cell<DockViewportRuntimeAdmission>>,
    work_context: DockViewportRuntimeWorkContext,
    controller: Entity<DockController>,
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    drag_session: DockRuntimeDragSession,
    plan: DockWorkspaceLockedPayloadDropPlan,
    target_window: DockViewportAppliedReusableWindow,
    expected_target_window: AnyWindowHandle,
    source_registration: Option<DockViewportRegistrationKey>,
}

#[must_use = "a preflighted locked payload drop must commit without further validation"]
pub(crate) struct DockViewportPreflightedLockedPayloadDrop {
    work_context: DockViewportRuntimeWorkContext,
    controller: Entity<DockController>,
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    drag_session: DockRuntimeDragSession,
    workspace: DockWorkspacePreparedLockedPayloadDrop,
    target_registration: DockViewportRegistrationKey,
    target_window: AnyWindowHandle,
    source_registration: Option<DockViewportRegistrationKey>,
    source_is_empty: bool,
}

#[must_use = "a committed locked payload drop must publish its update and settle window effects"]
pub(crate) struct DockViewportCommittedLockedPayloadDrop {
    outcome: DockViewportDropRouteOutcome,
    runtime_update: DockViewportRuntimeUpdate,
    window_effects: DockViewportWindowEffects,
}

impl DockViewportCommittedLockedPayloadDrop {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DockViewportDropRouteOutcome,
        DockViewportRuntimeUpdate,
        DockViewportWindowEffects,
    ) {
        (self.outcome, self.runtime_update, self.window_effects)
    }
}

pub(crate) struct DockViewportAppliedPayloadDrop {
    work_context: DockViewportRuntimeWorkContext,
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    drag_session: DockRuntimeDragSession,
    drop_outcome: DockWorkspacePayloadDropOutcome,
    focus_request: DockViewportFocusRequest,
    target_window: DockViewportAppliedReusableWindow,
    source_registration: Option<DockViewportRegistrationKey>,
    source_is_empty: bool,
}

pub(crate) struct DockViewportPreparedDragFocusItem {
    controller: Entity<DockController>,
    payload: DockDragPayload,
    focused_item: DockItemId,
}

impl DockViewportPreparedDragFocusItem {
    pub(crate) fn sample(self, cx: &App) -> Option<DockItemId> {
        self.controller
            .read(cx)
            .workspace()
            .drag_focus_item_for_payload(&self.payload, Some(&self.focused_item))
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedSourceVacate {
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    source_registration: Option<DockViewportRegistrationKey>,
}

#[derive(Debug)]
pub(crate) struct DockViewportAppliedSourceVacate {
    prepared: DockViewportPreparedSourceVacate,
    source_is_empty: bool,
}

impl DockViewportPreparedSourceVacate {
    pub(crate) fn apply(self, source_is_empty: bool) -> DockViewportAppliedSourceVacate {
        DockViewportAppliedSourceVacate {
            prepared: self,
            source_is_empty,
        }
    }
}

pub(crate) struct DockViewportPreparedTearOffMoveApply {
    controller: Entity<DockController>,
    pending: DockViewportTearOffPending,
}

pub(crate) struct DockViewportAppliedTearOffMove {
    pending: DockViewportTearOffPending,
    result: Result<(DockActionOutcome, bool), DockActionApplyError>,
}

pub(crate) struct DockViewportPreparedTearOffTargetClaim {
    controller: Entity<DockController>,
    pending: DockViewportTearOffPending,
    window: AnyWindowHandle,
    open_attempt: DockViewportWindowOpenAttemptKey,
    target_registration_generation: Option<u64>,
}

pub(crate) struct DockViewportAppliedTearOffTargetClaim {
    pending: DockViewportTearOffPending,
    window: AnyWindowHandle,
    open_attempt: DockViewportWindowOpenAttemptKey,
    target_registration_generation: Option<u64>,
    target_graph_is_vacant: bool,
}

#[derive(Debug)]
pub(crate) struct DockViewportClaimedTearOffTarget {
    pending: DockViewportTearOffPending,
    window: AnyWindowHandle,
    open_attempt: DockViewportWindowOpenAttemptKey,
    registration: DockViewportRuntimeRegistration,
}

#[derive(Debug)]
pub(crate) struct DockViewportRolledBackTearOffTarget {
    window: AnyWindowHandle,
    open_attempt: DockViewportWindowOpenAttemptKey,
    window_effects: DockViewportWindowEffects,
}

pub(crate) struct DockViewportPreparedTearOffSourceCheck {
    controller: Entity<DockController>,
    pending: DockViewportTearOffPending,
}

pub(crate) struct DockViewportAppliedTearOffSourceCheck {
    pending: DockViewportTearOffPending,
    source_status: DockViewportTearOffSourceStatus,
}

impl DockViewportPreparedTearOffMoveApply {
    pub(crate) fn apply(self, cx: &mut App) -> DockViewportAppliedTearOffMove {
        let Self {
            controller,
            pending,
        } = self;
        let result = controller.update(cx, |controller, cx| {
            let outcome = crate::commit_tear_off_move(controller.workspace_mut(), &pending);
            if outcome.as_ref().is_ok_and(|outcome| outcome.changed()) {
                cx.notify();
            }
            outcome.map(|outcome| {
                let source_is_empty = outcome.changed()
                    && controller
                        .graph()
                        .collect_items_in_space(pending.request().source_space())
                        .is_empty();
                (outcome, source_is_empty)
            })
        });
        DockViewportAppliedTearOffMove { pending, result }
    }
}

impl DockViewportPreparedTearOffTargetClaim {
    pub(crate) fn sample(self, cx: &App) -> DockViewportAppliedTearOffTargetClaim {
        let target_graph_is_vacant = !self
            .controller
            .read(cx)
            .graph()
            .spaces()
            .iter()
            .any(|space| space == self.pending.target_space());
        DockViewportAppliedTearOffTargetClaim {
            pending: self.pending,
            window: self.window,
            open_attempt: self.open_attempt,
            target_registration_generation: self.target_registration_generation,
            target_graph_is_vacant,
        }
    }
}

impl DockViewportRolledBackTearOffTarget {
    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn open_attempt(&self) -> DockViewportWindowOpenAttemptKey {
        self.open_attempt
    }

    pub(crate) fn into_window_effects(self) -> DockViewportWindowEffects {
        self.window_effects
    }
}

impl DockViewportPreparedTearOffSourceCheck {
    pub(crate) fn sample(self, cx: &App) -> DockViewportAppliedTearOffSourceCheck {
        let source_status =
            crate::tear_off_source_status(self.controller.read(cx).graph(), &self.pending);
        DockViewportAppliedTearOffSourceCheck {
            pending: self.pending,
            source_status,
        }
    }
}

impl DockViewportPreparedPayloadDrop {
    pub(crate) fn apply(
        self,
        cx: &mut App,
    ) -> Result<DockViewportAppliedPayloadDrop, DockActionApplyError> {
        let Self {
            admission,
            work_context,
            controller,
            source_space,
            target,
            target_space,
            drag_session,
            target_window,
            source_registration,
        } = self;
        let frozen_focus_item = drag_session.focus_item().cloned();
        let drag_session_id = drag_session.id();
        let drop_outcome = controller.update(cx, |controller, cx| {
            if !admission.get().admits(work_context.lineage()) {
                return Err(DockActionApplyError::DropDragSessionStale {
                    session: drag_session_id,
                });
            }
            let outcome = match target {
                DockViewportPreparedPayloadDropTarget::Resolved {
                    source_node,
                    payload,
                    target,
                } => {
                    let payload = payload.as_workspace_payload(source_node);
                    controller.workspace_mut().commit_resolved_payload_drop(
                        DockWorkspacePayloadDropRequest {
                            source_space: &source_space,
                            payload,
                            target,
                            frozen_focus_item: frozen_focus_item.as_ref(),
                        },
                    )
                }
                DockViewportPreparedPayloadDropTarget::Locked(target) => controller
                    .workspace_mut()
                    .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                        plan: target,
                        frozen_focus_item: frozen_focus_item.as_ref(),
                    }),
            };
            if outcome.as_ref().is_ok_and(|outcome| outcome.changed()) {
                cx.notify();
            }
            outcome
        })?;
        let focus_item = drag_session
            .focus_item()
            .and_then(|focus_item| {
                controller
                    .read(cx)
                    .graph()
                    .find_item_in_space(&target_space, focus_item)?;
                Some(focus_item.clone())
            })
            .or_else(|| drop_outcome.focus_item().cloned());
        let focus_request = focus_item.map_or_else(
            DockViewportFocusRequest::no_panel_focus,
            DockViewportFocusRequest::panel,
        );
        let source_is_empty = source_space != target_space
            && controller
                .read(cx)
                .graph()
                .collect_items_in_space(&source_space)
                .is_empty();
        let target_window = target_window.sample(cx);
        Ok(DockViewportAppliedPayloadDrop {
            work_context,
            source_space,
            target_space,
            drag_session,
            drop_outcome,
            focus_request,
            target_window,
            source_registration,
            source_is_empty,
        })
    }
}

impl DockViewportPreparedAtomicLockedPayloadDrop {
    pub(crate) fn sample_atomic_locked_payload_drop(
        self,
        cx: &mut App,
    ) -> DockViewportSampledAtomicLockedPayloadDrop {
        let Self {
            admission,
            work_context,
            controller,
            source_space,
            target_space,
            drag_session,
            plan,
            target_window,
            expected_target_window,
            source_registration,
        } = self;
        DockViewportSampledAtomicLockedPayloadDrop {
            admission,
            work_context,
            controller,
            source_space,
            target_space,
            drag_session,
            plan,
            target_window: target_window.sample(cx),
            expected_target_window,
            source_registration,
        }
    }
}

impl DockViewportLockedWorkspaceDrop {
    pub(crate) fn new(
        plan: DockWorkspaceLockedPayloadDropPlan,
        drag_session: DockRuntimeDragSession,
    ) -> Self {
        Self { plan, drag_session }
    }
}

pub(crate) struct DockViewportPreparedWindowClose {
    controller: Entity<DockController>,
    close: DockViewportClosedWindowRefresh,
    pending_state: Option<DockViewportClosePlanState>,
    finalize_key: Option<DockViewportCloseFinalizeKey>,
}

pub(crate) struct DockViewportAppliedWindowClose {
    prepared: DockViewportPreparedWindowClose,
    merge_back_status: Option<DockViewportCloseStatus>,
}

pub(crate) struct DockViewportPreparedShouldClose {
    controller: Entity<DockController>,
    expected_registration: Option<DockViewportRegistrationKey>,
    finalize_key: DockViewportShouldCloseFinalizeKey,
    request_update_generation: Option<u64>,
    outcome: DockViewportShouldCloseOutcome,
    close_policy: DockViewportClosePolicy,
    focused_item: Option<DockItemId>,
    close_already_requested: bool,
}

pub(crate) struct DockViewportAppliedShouldClose {
    expected_registration: Option<DockViewportRegistrationKey>,
    finalize_key: DockViewportShouldCloseFinalizeKey,
    request_update_generation: Option<u64>,
    outcome: DockViewportShouldCloseOutcome,
    plan_mutation: DockViewportShouldClosePlanMutation,
    invalidate_route: bool,
}

enum DockViewportShouldClosePlanMutation {
    Preserve,
    Replace(Option<DockViewportMergeBackClosePlan>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportCloseFinalizeDisposition {
    Current,
    Stale,
}

pub(crate) struct DockViewportFinalizedWindowClose {
    close: DockViewportClosedWindowRefresh,
    disposition: DockViewportCloseFinalizeDisposition,
}

pub(crate) struct DockViewportFinalizedShouldClose {
    should_close: DockViewportShouldCloseRefresh,
    disposition: DockViewportCloseFinalizeDisposition,
}

impl DockViewportPreparedWindowClose {
    pub(crate) fn apply_merge_back(self, cx: &mut App) -> DockViewportAppliedWindowClose {
        let merge_back_status = match (
            self.pending_state.as_ref(),
            self.close.outcome.space().is_some(),
        ) {
            (Some(DockViewportClosePlanState::Pending(plan)), true) => Some(
                crate::commit_prevalidated_merge_back_plan(&self.controller, plan, cx),
            ),
            _ => None,
        };
        DockViewportAppliedWindowClose {
            prepared: self,
            merge_back_status,
        }
    }
}

impl DockViewportPreparedShouldClose {
    pub(crate) fn apply(self, cx: &App) -> DockViewportAppliedShouldClose {
        self.apply_with_controller_observer(cx, || {})
    }

    #[cfg(test)]
    pub(crate) fn apply_with_controller_observer_for_test(
        self,
        cx: &App,
        observer: impl FnOnce(),
    ) -> DockViewportAppliedShouldClose {
        self.apply_with_controller_observer(cx, observer)
    }

    fn apply_with_controller_observer(
        self,
        cx: &App,
        observer: impl FnOnce(),
    ) -> DockViewportAppliedShouldClose {
        let Self {
            controller,
            expected_registration,
            finalize_key,
            request_update_generation,
            mut outcome,
            close_policy,
            focused_item,
            close_already_requested,
        } = self;
        let mut controller_observer = Some(observer);
        let plan_mutation = if close_already_requested
            || outcome.status != DockViewportShouldCloseStatus::Allowed
        {
            DockViewportShouldClosePlanMutation::Preserve
        } else if let Some(source_space) = outcome.space.as_ref() {
            let controller = controller.read(cx);
            controller_observer
                .take()
                .expect("should-close controller observer must run at most once")();
            let workspace = controller.workspace();
            let focus_item = focused_item.filter(|item| {
                controller
                    .graph()
                    .find_item_in_space(source_space, item)
                    .is_some()
            });
            let merge_target = match &close_policy {
                DockViewportClosePolicy::RetainLayout => workspace
                    .validate_close_space(source_space)
                    .is_ok()
                    .then_some(DockMergeBackTarget::SpaceOnly),
                DockViewportClosePolicy::MergeBack { target_space } => workspace
                    .resolve_merge_target(source_space, target_space)
                    .ok(),
                DockViewportClosePolicy::Prevent => None,
            };
            match merge_target {
                Some(merge_target) => {
                    let plan = match close_policy {
                        DockViewportClosePolicy::MergeBack { target_space } => Some(
                            DockViewportMergeBackClosePlan::new(
                                source_space.clone(),
                                target_space,
                                focus_item,
                            )
                            .with_target(merge_target),
                        ),
                        DockViewportClosePolicy::RetainLayout => None,
                        DockViewportClosePolicy::Prevent => {
                            unreachable!("prevent policy cannot produce an allowed close outcome")
                        }
                    };
                    DockViewportShouldClosePlanMutation::Replace(plan)
                }
                None => {
                    outcome.status = DockViewportShouldCloseStatus::Vetoed;
                    DockViewportShouldClosePlanMutation::Replace(None)
                }
            }
        } else {
            DockViewportShouldClosePlanMutation::Preserve
        };
        DockViewportAppliedShouldClose {
            expected_registration,
            finalize_key,
            request_update_generation,
            invalidate_route: !close_already_requested
                && outcome.status == DockViewportShouldCloseStatus::Allowed,
            outcome,
            plan_mutation,
        }
    }
}

impl DockViewportFinalizedWindowClose {
    pub(crate) fn is_current(&self) -> bool {
        self.disposition == DockViewportCloseFinalizeDisposition::Current
    }

    pub(crate) fn into_refresh(self) -> DockViewportClosedWindowRefresh {
        self.close
    }
}

impl DockViewportFinalizedShouldClose {
    pub(crate) fn is_current(&self) -> bool {
        self.disposition == DockViewportCloseFinalizeDisposition::Current
    }

    pub(crate) fn into_refresh(self) -> DockViewportShouldCloseRefresh {
        self.should_close
    }
}

impl DockViewportRuntimeRegistration {
    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }

    pub(crate) fn runtime_update(&self) -> &DockViewportRuntimeUpdate {
        &self.runtime_update
    }
}

#[derive(Debug, Default)]
struct DockViewportVacatedPayloadDropSource {
    changed: bool,
    windows: Vec<DockViewportWindowCloseEffect>,
    affected_windows: Vec<AnyWindowHandle>,
}

impl DockViewportVacatedPayloadDropSource {
    fn changed(&self) -> bool {
        self.changed
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedTearOffDrop {
    request: DockViewportTearOffRequest,
    target_space: DockSpaceId,
    focus_item: Option<DockItemId>,
    options: WindowOptions,
    move_plan: crate::viewport_tear_off_move::DockViewportTearOffMovePlan,
}

pub(crate) struct DockViewportPreparedTearOffDropProbe {
    controller: Entity<DockController>,
    request: DockViewportTearOffRequest,
    target_space: DockSpaceId,
    options: WindowOptions,
}

impl DockViewportPreparedTearOffDropProbe {
    pub(crate) fn sample(
        self,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        let (focus_item, move_plan) = {
            let controller = self.controller.read(cx);
            let move_plan = crate::lock_tear_off_move(
                controller.workspace(),
                &self.request,
                &self.target_space,
            )?;
            let focus_item = controller
                .workspace()
                .activation_focus_item_for_viewport_payload(
                    self.request.payload(),
                    self.request.source_node(),
                    self.request
                        .drag_session()
                        .and_then(DockRuntimeDragSession::focus_item),
                );
            (focus_item, move_plan)
        };
        Ok(DockViewportPreparedTearOffDrop::new(
            self.request,
            self.target_space,
            focus_item,
            self.options,
            move_plan,
        ))
    }
}

impl DockViewportPreparedTearOffDrop {
    fn new(
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        focus_item: Option<DockItemId>,
        options: WindowOptions,
        move_plan: crate::viewport_tear_off_move::DockViewportTearOffMovePlan,
    ) -> Self {
        Self {
            request,
            target_space,
            focus_item,
            options,
            move_plan,
        }
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    #[cfg(test)]
    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

pub(crate) struct DockViewportPreparedTearOffWindow {
    pub(crate) pending: DockViewportTearOffPending,
    pub(crate) options: WindowOptions,
}

pub(crate) enum DockViewportPreparedTearOffBegin {
    Pending(DockViewportPreparedTearOffWindow),
    Duplicate(DockViewportTearOffPending),
    Unavailable(DockViewportTearOffPending),
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub(crate) fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy_and_visual_style_resolver(
            controller,
            DockViewportClosePolicy::default(),
            None,
        )
    }

    /// Creates a runtime with an explicit close policy.
    pub(crate) fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self::with_close_policy_and_visual_style_resolver(controller, close_policy, None)
    }

    pub(crate) fn with_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<crate::DockVisualStyleResolver>,
    ) -> Self {
        Self::with_admission_close_policy_and_visual_style_resolver(
            controller,
            DockViewportRuntimeAdmission::unmanaged(),
            close_policy,
            visual_style_resolver,
        )
    }

    pub(crate) fn with_surface_authority_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        authority: open_gpui::EntityId,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<crate::DockVisualStyleResolver>,
    ) -> Self {
        Self::with_admission_close_policy_and_visual_style_resolver(
            controller,
            DockViewportRuntimeAdmission::surface(authority),
            close_policy,
            visual_style_resolver,
        )
    }

    fn with_admission_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        admission: DockViewportRuntimeAdmission,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: Option<crate::DockVisualStyleResolver>,
    ) -> Self {
        Self {
            controller,
            admission: Rc::new(Cell::new(admission)),
            adapter: DockViewportAdapter::new(),
            close_policy,
            visual_style_resolver,
            frame_coordinator: DockViewportFrameCoordinator::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_target_reservations: HashMap::new(),
            next_tear_off_space_index: 0,
            payload_drag: DockViewportPayloadDragState::default(),
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            backend_focus_cancellations: Vec::new(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
            status: DockViewportRuntimeStatus::default(),
            #[cfg(test)]
            reject_next_provisional_registration: false,
            #[cfg(test)]
            reject_next_live_undock_promotion_commit: Cell::new(false),
        }
    }

    /// Creates a runtime from an existing adapter.
    #[cfg(test)]
    pub(crate) fn from_adapter(
        controller: Entity<DockController>,
        adapter: DockViewportAdapter,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            admission: Rc::new(Cell::new(DockViewportRuntimeAdmission::unmanaged())),
            adapter,
            close_policy,
            visual_style_resolver: None,
            frame_coordinator: DockViewportFrameCoordinator::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_target_reservations: HashMap::new(),
            next_tear_off_space_index: 0,
            payload_drag: DockViewportPayloadDragState::default(),
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            backend_focus_cancellations: Vec::new(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
            status: DockViewportRuntimeStatus::default(),
            reject_next_provisional_registration: false,
            reject_next_live_undock_promotion_commit: Cell::new(false),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub(crate) fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    pub(crate) fn admission(&self) -> DockViewportRuntimeAdmission {
        self.admission.get()
    }

    pub(crate) fn current_work_context(
        &self,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Option<DockViewportRuntimeWorkContext> {
        self.admission
            .get()
            .default_lineage()
            .map(|lineage| DockViewportRuntimeWorkContext::new(lineage, surface_transaction))
    }

    pub(crate) fn activate_surface_lineage(
        &mut self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> DockViewportRuntimeLineageActivationOutcome {
        let mut admission = self.admission.get();
        let prior_generation_empty = admission
            .frozen_surface_lease()
            .is_none_or(|frozen| self.surface_generation_empty(frozen));
        let outcome = admission.activate_surface(lease, prior_generation_empty);
        self.admission.set(admission);
        outcome
    }

    pub(crate) fn freeze_surface_shutdown(
        &mut self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> Option<DockViewportSurfaceShutdownReservation> {
        let mut admission = self.admission.get();
        let outcome = admission.freeze_surface(lease);
        self.admission.set(admission);
        match outcome {
            DockViewportRuntimeLineageFreezeOutcome::Frozen => {}
            DockViewportRuntimeLineageFreezeOutcome::AlreadyFrozen => return None,
            DockViewportRuntimeLineageFreezeOutcome::StaleLease
            | DockViewportRuntimeLineageFreezeOutcome::UnmanagedRuntime => return None,
        }

        let windows = self.window_ownership.freeze_surface(lease);
        Some(DockViewportSurfaceShutdownReservation::new(lease, windows))
    }

    pub(crate) fn commit_surface_shutdown(
        &mut self,
        reservation: DockViewportSurfaceShutdownReservation,
    ) -> DockViewportSurfaceShutdownEffects {
        self.finalize_frozen_surface_retirement(reservation)
    }

    pub(crate) fn retire_frozen_surface_after_capture_failure(
        &mut self,
        reservation: DockViewportSurfaceShutdownReservation,
    ) -> DockViewportSurfaceShutdownEffects {
        self.finalize_frozen_surface_retirement(reservation)
    }

    fn finalize_frozen_surface_retirement(
        &mut self,
        reservation: DockViewportSurfaceShutdownReservation,
    ) -> DockViewportSurfaceShutdownEffects {
        let (lease, windows) = reservation.into_parts();
        if !self.admits_frozen_surface_shutdown(lease) {
            return DockViewportSurfaceShutdownEffects::new(
                lease,
                windows,
                DockViewportRuntimeUpdate::default(),
            );
        }
        let lineage = crate::DockViewportRuntimeLineage::Surface(lease);
        let work_context = DockViewportRuntimeWorkContext::new(lineage, None);
        let spaces = self
            .adapter
            .snapshots()
            .filter_map(|(space, snapshot)| {
                (snapshot.lineage() == lineage).then_some(space.clone())
            })
            .collect::<Vec<_>>();
        let topology_changed = !spaces.is_empty();
        for space in spaces {
            let _ = self.unregister_space_runtime_state(&space);
        }
        let mut cleanup_update = DockViewportRuntimeUpdate::default();
        cleanup_update.mark_viewport_topology(topology_changed, work_context);

        self.frame_coordinator = DockViewportFrameCoordinator::default();
        self.tear_off = DockViewportTearOffMachine::default();
        self.tear_off_target_reservations.clear();
        self.payload_drag = DockViewportPayloadDragState::default();
        self.focus = DockViewportFocusCoordinator::default();
        self.backend_focus = DockViewportBackendFocusState::default();
        self.backend_focus_cancellations.clear();
        self.close_coordinator = DockViewportCloseCoordinator::default();
        self.routed_drop_preview = DockViewportRoutedDropPreviewState::default();
        self.status = DockViewportRuntimeStatus::default();
        DockViewportSurfaceShutdownEffects::new(lease, windows, cleanup_update)
    }

    pub(crate) fn admits_frozen_surface_shutdown(
        &self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        self.admission.get().frozen_surface_lease() == Some(lease)
    }

    pub(crate) fn abort_surface_opening(
        &mut self,
        opening: crate::surface::window_session::DockSurfaceWindowSessionOpeningToken,
    ) -> Vec<AnyWindowHandle> {
        self.window_ownership.abort_surface_opening(opening)
    }

    pub(crate) fn settle_surface_window_terminal(
        &mut self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let terminal_space = self
            .adapter
            .space_for_window_id(window_id)
            .and_then(|space| {
                self.adapter
                    .registration_key(space)
                    .filter(|registration| {
                        registration.lineage() == crate::DockViewportRuntimeLineage::Surface(lease)
                    })
                    .map(|_| space.clone())
            });
        let ownership_settled = self
            .window_ownership
            .settle_surface_window_terminal(lease, window_id);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(ownership_settled);
        if let Some(space) = terminal_space
            && let Some(unregistered) = self.unregister_space_runtime_state(&space)
        {
            update.mark_changed(true);
            update.extend_windows(unregistered.affected_windows);
        }
        update
    }

    pub(crate) fn settle_live_undock_committed_destination_logical_close(
        &mut self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<DockViewportClosedWindowRefresh> {
        if !self.adapter.is_current_registration(registration) {
            return None;
        }

        let space = registration.space().clone();
        let window_id = registration.window_id();
        self.retire_window(window_id);
        let unregistered = self.unregister_space_runtime_state(&space)?;
        let outcome =
            DockViewportCloseOutcome::new(Some(space), window_id, DockViewportCloseStatus::Closed);
        self.status.record_close(&outcome);
        Some(DockViewportClosedWindowRefresh::new(
            outcome,
            DockViewportWindowEffects::refresh_only(unregistered.affected_windows),
        ))
    }

    pub(crate) fn settle_native_window_terminal(&mut self, window_id: WindowId) -> bool {
        self.window_ownership
            .settle_native_window_terminal(window_id)
    }

    pub(crate) fn admits_work_context(&self, context: DockViewportRuntimeWorkContext) -> bool {
        self.admission.get().admits(context.lineage())
    }

    pub(crate) fn visual_style_resolver(&self) -> Option<crate::DockVisualStyleResolver> {
        self.visual_style_resolver.clone()
    }

    /// Returns the low-level viewport adapter.
    pub(crate) fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    pub(crate) fn registration_key_for_space_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRegistrationKey> {
        self.adapter
            .registration_key(space)
            .filter(|key| key.window_id() == window_id)
    }

    pub(crate) fn admits_registration(&self, key: &DockViewportRegistrationKey) -> bool {
        self.admission.get().admits(key.lineage()) && self.adapter.is_current_registration(key)
    }

    pub(crate) fn admits_registration_in_context(
        &self,
        context: DockViewportRuntimeWorkContext,
        key: &DockViewportRegistrationKey,
    ) -> bool {
        context.lineage() == key.lineage()
            && self.admits_work_context(context)
            && self.adapter.is_current_registration(key)
    }

    #[cfg(test)]
    pub(crate) fn unregister_adapter_window_for_test(&mut self, window_id: WindowId) {
        let _ = self.adapter.unregister_window_id_snapshot(window_id);
    }

    #[cfg(test)]
    pub(crate) fn replace_adapter_registration_for_test(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRegistrationKey {
        let _ = self.adapter.unregister_space(&space);
        let lineage = self
            .admission
            .get()
            .default_lineage()
            .expect("test runtime registration requires an admitted lineage");
        self.adapter
            .register_viewport_with_outcome(space, window, lineage)
            .expect("test replacement cannot cross runtime lineage")
            .registration_key()
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_ready(&self, space: &DockSpaceId) -> bool {
        self.adapter.route_ready(space)
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_unavailable_reason(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRouteUnavailableReason> {
        self.adapter.route_unavailable_reason(space)
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub(crate) fn runtime_status(&self) -> DockViewportRuntimeStatus {
        let status = self.status.clone();
        status
            .with_window_ownership(self.window_ownership.status())
            .with_viewport_lifecycle(self.adapter.viewport_lifecycle_records())
    }

    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.backend_focus.pending_activation()
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag(
        &mut self,
        payload: &DockDragPayload,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_with_focus(payload, None)
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag_with_focus(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> DockRuntimeDragSession {
        self.begin_payload_drag_with_focus_and_drag_visual_style(
            payload,
            focus_item,
            crate::DockVisualStyle::built_in().drag,
        )
    }

    pub(crate) fn begin_payload_drag_with_focus_and_drag_visual_style(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
        drag_visual_style: crate::DockDragVisualStyle,
    ) -> DockRuntimeDragSession {
        let lineage = self
            .admission
            .get()
            .default_lineage()
            .expect("payload drag requires an admitted runtime lineage");
        let source_window = self
            .adapter
            .window_for_space(payload.identity().source_space());
        self.clear_routed_drop_preview();
        self.payload_drag.begin(
            lineage,
            payload,
            focus_item,
            source_window,
            drag_visual_style,
        )
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &mut self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        self.payload_drag
            .update_tear_off_geometry(session, geometry)
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        self.payload_drag.tear_off_geometry(session)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.payload_drag.active_session_for_payload(payload)
    }

    pub(crate) fn active_payload_drag_visual_style(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockDragVisualStyle> {
        self.payload_drag.drag_visual_style(session).cloned()
    }

    #[cfg(test)]
    pub(crate) fn active_payload_drag_source_window_id(
        &self,
        payload: &DockDragPayload,
    ) -> Option<WindowId> {
        self.payload_drag
            .active_source_window_id_for_payload(payload)
    }

    fn window_for_viewport_identity(
        &self,
        identity: &DockViewportIdentity,
    ) -> Option<AnyWindowHandle> {
        let window = self.adapter.window_for_space(identity.space())?;
        (window.window_id() == identity.window_id()).then_some(window)
    }

    pub(crate) fn finish_payload_drag(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> DockViewportRuntimeUpdate {
        if self
            .admission
            .get()
            .frozen_surface_lease()
            .is_some_and(|lease| {
                session.lineage() == crate::DockViewportRuntimeLineage::Surface(lease)
            })
        {
            return DockViewportRuntimeUpdate::default();
        }
        let Some(finish) = self.payload_drag.finish(session) else {
            return DockViewportRuntimeUpdate::default();
        };
        let last_routed_window = finish
            .last_routed_viewport_identity()
            .and_then(|identity| self.window_for_viewport_identity(identity));
        let mut update = self.clear_routed_drop_preview_for_drag_session(Some(session));
        update.extend_windows(last_routed_window);
        update.mark_changed(true);
        update
    }

    pub(crate) fn validate_payload_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        let Some(session) = session else {
            return Err(DockActionApplyError::DropDragSessionMissing);
        };
        if !self.admission.get().admits(session.lineage()) {
            return Err(DockActionApplyError::DropDragSessionStale {
                session: session.id(),
            });
        }
        self.payload_drag.validate_session(Some(session))
    }

    fn record_confirmed_backend_focused_window(&mut self, window_id: WindowId) -> Option<bool> {
        let adapter = &self.adapter;
        self.backend_focus
            .record_confirmed_backend_focused_window(window_id, |candidate| {
                adapter.space_for_window_id(candidate).is_some()
                    && !adapter.window_close_requested(candidate)
            })
            .map(|focus_record| {
                if let Some(cancellation) = focus_record.cleared_pending_activation() {
                    if cancellation.surface_activation_binding().is_some() {
                        self.backend_focus_cancellations.push(cancellation.clone());
                    }
                }
                focus_record.changed()
            })
    }

    pub(crate) fn take_backend_focus_cancellations(
        &mut self,
    ) -> Vec<DockViewportActivationTransaction> {
        std::mem::take(&mut self.backend_focus_cancellations)
    }

    pub(crate) fn record_confirmed_backend_focus_for_window(
        &mut self,
        window_id: WindowId,
    ) -> bool {
        self.record_confirmed_backend_focused_window(window_id)
            .unwrap_or(false)
    }

    pub(crate) fn record_confirmed_backend_focus_signal(
        &mut self,
        focus: PlatformFocusedWindow,
    ) -> bool {
        match focus {
            PlatformFocusedWindow::Window(window) => {
                self.record_confirmed_backend_focus_for_window(window.window_id())
            }
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn reconcile_backend_window_focus(&mut self, cx: &App) -> bool {
        self.record_confirmed_backend_focus_signal(cx.focused_window())
    }

    pub(crate) fn apply_activation_backend_focus(
        &mut self,
        activation: &DockViewportActivationTransaction,
        backend_focus: DockViewportActivationBackendFocusObservation,
        request_backend_activation: bool,
    ) -> DockViewportActivationBackendFocusApply {
        let backend_focus_recorded_changed = if backend_focus.target_focused() {
            self.record_confirmed_backend_focus_for_window(activation.window_id())
        } else {
            false
        };
        let pending_backend_focus = request_backend_activation
            && !backend_focus.target_focused()
            && self.record_pending_activation(activation.clone());
        let pending_backend_focus_cleared = if backend_focus.target_focused() {
            let cleared =
                self.take_pending_activation_for_registration(activation.registration_key());
            let cleared_present = cleared.is_some();
            self.queue_displaced_activation(cleared, Some(activation));
            cleared_present
        } else {
            false
        };
        DockViewportActivationBackendFocusApply::new(
            DockViewportActivationBackendFocusRecordEffect::from_changed(
                backend_focus_recorded_changed,
            ),
            if backend_focus.target_focused() {
                DockViewportActivationPendingBackendFocusEffect::from_cleared(
                    pending_backend_focus_cleared,
                )
            } else {
                DockViewportActivationPendingBackendFocusEffect::from_recorded(
                    pending_backend_focus,
                )
            },
        )
    }

    pub(crate) fn record_pending_activation(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> bool {
        let update = self
            .backend_focus
            .record_pending_activation_with_displaced(activation.clone());
        let changed = update.changed();
        self.queue_displaced_activation(update.displaced(), Some(&activation));
        changed
    }

    pub(crate) fn clear_pending_activation_for(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        let cleared = self
            .backend_focus
            .take_pending_activation_for(space, window_id);
        let changed = cleared.is_some();
        self.queue_displaced_activation(cleared, None);
        changed
    }

    fn take_pending_activation_for_registration(
        &mut self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<DockViewportActivationTransaction> {
        self.backend_focus
            .take_pending_activation_for_registration(registration)
    }

    fn queue_displaced_activation(
        &mut self,
        displaced: Option<DockViewportActivationTransaction>,
        replacement: Option<&DockViewportActivationTransaction>,
    ) {
        let Some(displaced) = displaced else {
            return;
        };
        let displaced_binding = displaced.surface_activation_binding();
        let replacement_binding =
            replacement.and_then(DockViewportActivationTransaction::surface_activation_binding);
        if displaced_binding != replacement_binding && displaced_binding.is_some() {
            self.backend_focus_cancellations.push(displaced);
        }
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        backend_focus: PlatformFocusedWindow,
        platform_focus_restore_policy: DockViewportPlatformFocusRestorePolicy,
    ) -> crate::DockViewportConfirmedBackendFocusOutcome {
        let backend_focused = match backend_focus {
            PlatformFocusedWindow::Window(window) => window.window_id() == window_id,
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => {
                return crate::DockViewportConfirmedBackendFocusOutcome::default();
            }
        };
        let Some(registration) = self.adapter.registration_key(space).filter(|registration| {
            registration.window_id() == window_id && !self.adapter.window_close_requested(window_id)
        }) else {
            return crate::DockViewportConfirmedBackendFocusOutcome::default();
        };
        if !backend_focused || !self.admits_registration(&registration) {
            return crate::DockViewportConfirmedBackendFocusOutcome::default();
        }

        let focus_record_changed = self
            .record_confirmed_backend_focused_window(window_id)
            .expect("backend focus was already validated as a live docking window");
        let focus_outcome = self.backend_focus.confirmed_backend_window_focus_outcome(
            &self.focus,
            &registration,
            platform_focus_restore_gate,
            platform_focus_restore_policy,
        );
        focus_outcome.with_additional_changed(focus_record_changed)
    }

    pub(crate) fn record_panel_focus(&mut self, space: DockSpaceId, item: DockItemId) {
        self.focus.record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&mut self, space: &DockSpaceId) {
        self.focus.record_no_panel_focus(space);
    }

    pub(crate) fn recorded_panel_focus_matches(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> bool {
        self.focus.focused_panel(space) == Some(item)
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus_for_test(&self, space: &DockSpaceId) -> Option<bool> {
        self.focus.had_panel_focus(space)
    }

    fn retire_window(&mut self, window_id: WindowId) -> DockViewportWindowRetirement {
        self.window_ownership.retire_window(window_id)
    }

    fn retire_runtime_window_for_close(
        &mut self,
        window: AnyWindowHandle,
    ) -> DockViewportWindowRetirement {
        self.retire_window(window.window_id())
    }

    pub(crate) fn begin_window_open_attempt(
        &mut self,
        window: AnyWindowHandle,
        lineage: crate::DockViewportRuntimeLineage,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        if !self.admission.get().admits(lineage) {
            return None;
        }
        let authority = match lineage {
            crate::DockViewportRuntimeLineage::Unmanaged => DockViewportWindowAuthority::Unmanaged,
            crate::DockViewportRuntimeLineage::Surface(lease) => {
                DockViewportWindowAuthority::Surface(lease)
            }
        };
        self.window_ownership.begin_open_attempt_with_authority(
            window,
            authority,
            DockViewportWindowRole::ManagedViewport,
        )
    }

    pub(crate) fn begin_primary_anchor_open_attempt(
        &mut self,
        window: AnyWindowHandle,
        opening: crate::surface::window_session::DockSurfaceWindowSessionOpeningToken,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        self.window_ownership.begin_open_attempt_with_authority(
            window,
            DockViewportWindowAuthority::SurfaceOpening(opening),
            DockViewportWindowRole::PrimaryAnchor,
        )
    }

    pub(crate) fn begin_live_undock_provisional_open_attempt(
        &mut self,
        window: AnyWindowHandle,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        let lineage = crate::DockViewportRuntimeLineage::Surface(opening.lease());
        if !self.admission.get().admits(lineage) {
            return None;
        }
        self.window_ownership
            .begin_provisional_open_attempt(window, opening)
    }

    pub(crate) fn complete_live_undock_provisional_open_attempt(
        &mut self,
        attempt: DockViewportWindowOpenAttemptKey,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
        admit: bool,
    ) -> DockViewportProvisionalOpenAttemptCompletion {
        let lineage = crate::DockViewportRuntimeLineage::Surface(opening.lease());
        let mut admit = admit && self.admission.get().admits(lineage);
        #[cfg(test)]
        if std::mem::take(&mut self.reject_next_provisional_registration) {
            admit = false;
        }
        self.window_ownership
            .complete_provisional_open_attempt(attempt, opening, admit)
    }

    pub(crate) fn prepare_live_undock_provisional_promotion(
        &mut self,
        target_space: &DockSpaceId,
        window: AnyWindowHandle,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
        context: DockViewportRuntimeWorkContext,
        window_facts: crate::DockViewportWindowFacts,
    ) -> Option<DockViewportPreparedLiveUndockPromotion> {
        if context.lineage() != crate::DockViewportRuntimeLineage::Surface(opening.lease())
            || !self.admits_work_context(context)
            || self.target_space_is_reserved(target_space)
            || self.adapter.window_for_space(target_space).is_some()
            || self
                .adapter
                .space_for_window_id(window.window_id())
                .is_some()
        {
            return None;
        }
        let ownership = self
            .window_ownership
            .prepare_provisional_window_promotion(window.window_id(), opening)?;
        let registration = self.adapter.prepare_vacant_registration(
            target_space.clone(),
            window,
            context.lineage(),
        )?;
        Some(DockViewportPreparedLiveUndockPromotion {
            target_space: target_space.clone(),
            window,
            context,
            window_facts,
            host_geometry: None,
            ownership,
            registration,
        })
    }

    pub(crate) fn can_commit_live_undock_provisional_promotion(
        &self,
        prepared: &DockViewportPreparedLiveUndockPromotion,
    ) -> bool {
        #[cfg(test)]
        if self.reject_next_live_undock_promotion_commit.replace(false) {
            return false;
        }
        prepared.host_geometry.is_some()
            && self.admits_work_context(prepared.context)
            && !self.target_space_is_reserved(&prepared.target_space)
            && self
                .window_ownership
                .can_commit_provisional_window_promotion(&prepared.ownership)
            && self
                .adapter
                .can_commit_vacant_registration(&prepared.registration)
    }

    pub(crate) fn commit_live_undock_provisional_promotion(
        &mut self,
        prepared: DockViewportPreparedLiveUndockPromotion,
    ) -> DockViewportCommittedLiveUndockPromotion {
        assert!(
            self.can_commit_live_undock_provisional_promotion(&prepared),
            "prepared live-undock viewport promotion must remain exact until commit"
        );
        self.window_ownership
            .commit_provisional_window_promotion(prepared.ownership);
        let registration = self
            .adapter
            .commit_vacant_registration(prepared.registration);
        let host_geometry = prepared
            .host_geometry
            .expect("prepared live-undock viewport promotion must retain accepted host geometry");
        let facts_change = self.adapter.update_snapshot_with_change(
            &prepared.target_space,
            prepared.window_facts,
            host_geometry,
        );
        self.close_coordinator
            .invalidate_finalize_for_space(&prepared.target_space);
        self.backend_focus
            .record_viewport_created(prepared.window.window_id());
        let mut runtime_update = DockViewportRuntimeUpdate::default();
        runtime_update.bind_work_context(prepared.context);
        runtime_update.mark_viewport_topology(true, prepared.context);
        runtime_update
            .mark_observed_viewport_placement(facts_change.placement_changed, prepared.context);
        runtime_update.extend_windows([prepared.window]);
        DockViewportCommittedLiveUndockPromotion {
            registration,
            runtime_update,
        }
    }

    #[cfg(test)]
    pub(crate) fn reject_next_provisional_registration_for_test(&mut self) {
        self.reject_next_provisional_registration = true;
    }

    #[cfg(test)]
    pub(crate) fn reject_next_live_undock_promotion_commit_for_test(&self) {
        self.reject_next_live_undock_promotion_commit.set(true);
    }

    pub(crate) fn adopt_provisional_window_during_shutdown(
        &mut self,
        window: AnyWindowHandle,
        opening: crate::surface::live_undock::DockLiveUndockOpeningKey,
    ) -> bool {
        if self.admission.get().frozen_surface_lease() != Some(opening.lease()) {
            return false;
        }
        let Some(ownership) = self
            .window_ownership
            .adopt_frozen_provisional_window(window, opening)
        else {
            return false;
        };
        debug_assert_eq!(ownership.window_id(), window.window_id());
        true
    }

    pub(crate) fn promote_primary_anchor_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        self.window_ownership
            .promote_primary_open_attempt(key, lease)
    }

    pub(crate) fn windows_for_surface(
        &self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> Vec<(DockViewportWindowRole, AnyWindowHandle)> {
        self.window_ownership.windows_for_surface(lease)
    }

    pub(crate) fn surface_generation_empty(
        &self,
        lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        !self.adapter.snapshots().any(|(_, snapshot)| {
            snapshot.lineage() == crate::DockViewportRuntimeLineage::Surface(lease)
        }) && self.window_ownership.windows_for_surface(lease).is_empty()
    }

    pub(crate) fn abort_window_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
    ) -> bool {
        self.window_ownership.abort_open_attempt(key)
    }

    pub(crate) fn retire_window_open_attempt_for_close(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        window: AnyWindowHandle,
    ) -> Option<DockViewportWindowCloseEffect> {
        let retirement = self.window_ownership.retire_open_attempt(key)?;
        DockViewportWindowCloseEffect::from_retirement(
            window,
            DockViewportWindowRetirement::RetiredNow(retirement),
        )
    }

    pub(crate) fn retire_claimed_window_open_attempt_for_close(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        window: AnyWindowHandle,
    ) -> Option<DockViewportWindowCloseEffect> {
        let retirement = self.window_ownership.retire_claimed_open_attempt(key)?;
        DockViewportWindowCloseEffect::from_retirement(
            window,
            DockViewportWindowRetirement::RetiredNow(retirement),
        )
    }

    pub(crate) fn settle_window_retirement(
        &mut self,
        key: DockViewportWindowRetirementKey,
    ) -> bool {
        self.window_ownership.settle_retirement(key)
    }

    pub(crate) fn record_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.window_ownership
            .record_render_passthrough_pointer_input(window_id)
    }

    pub(crate) fn take_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.window_ownership
            .take_render_passthrough_pointer_input(window_id)
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.clone()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_policy = close_policy;
    }

    #[cfg(test)]
    pub(crate) fn pending_tear_off_len(&self) -> usize {
        self.tear_off.len()
    }

    /// Updates display, window, and host bounds for a registered viewport.
    ///
    /// Separates any route-facts refresh from changes to serialized placement fields.
    pub(crate) fn update_viewport_snapshot(
        &mut self,
        space: &DockSpaceId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
    ) -> crate::viewport_registry::DockViewportWindowFactsChange {
        self.adapter
            .update_snapshot_with_change(space, window_facts, host_geometry)
    }

    pub(crate) fn platform_requests_for_space(
        &self,
        space: &DockSpaceId,
    ) -> DockViewportPlatformRequests {
        self.adapter.platform_requests_for_space(space)
    }

    pub(crate) fn mark_viewport_window_snapshot_stale(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(self.adapter.mark_window_snapshot_stale(window_id));
        update.merge(self.clear_preview_for_unready_window_route(window_id));
        update
    }

    pub(crate) fn release_host_binding(
        &mut self,
        registration: &DockViewportRegistrationKey,
    ) -> DockViewportRuntimeUpdate {
        if !self.admits_registration(registration) {
            return DockViewportRuntimeUpdate::default();
        }

        let space = registration.space();
        let window_id = registration.window_id();
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(self.adapter.mark_window_snapshot_stale(window_id));
        update.mark_changed(
            self.frame_coordinator
                .discard_frame_for_viewport(space, window_id),
        );
        update.mark_changed(self.clear_pending_activation_for(space, window_id));
        self.status.clear_window_references(space, window_id);
        update.merge(self.clear_routed_drop_preview_if_window_matches(window_id));
        update.merge(self.finish_payload_drag_for_source_space(space));
        update
    }

    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
    ) -> DockViewportRuntimeUpdate {
        let work_context = self.current_work_context(None);
        let facts_change = self
            .adapter
            .apply_platform_window_facts_with_change(window_id, window_facts);
        let mut update = DockViewportRuntimeUpdate::default();
        if let Some(work_context) = work_context {
            update.mark_observed_viewport_placement(facts_change.placement_changed, work_context);
        }
        update.mark_changed(facts_change.changed);
        update.merge(self.clear_preview_for_unready_window_route(window_id));
        update
    }

    fn mark_viewport_window_close_requested(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(self.adapter.mark_window_close_requested(window_id));
        if let Some(space) = self.adapter.space_for_window_id(window_id).cloned() {
            self.status.clear_window_references(&space, window_id);
            update.merge(self.finish_payload_drag_for_source_space(&space));
        }
        self.frame_coordinator.unregister_window_scene(window_id);
        update.merge(self.clear_routed_drop_preview_if_window_matches(window_id));
        update
    }

    pub(crate) fn cancel_window_close_request(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        let close_plan_effect = self.close_coordinator.cancel_window(window_id);
        let changed = self.adapter.cancel_window_close_requested(window_id);
        if !changed {
            let mut update = DockViewportRuntimeUpdate::default();
            update.mark_changed(close_plan_effect.changed());
            return update;
        }
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(true);
        let windows: Vec<AnyWindowHandle> = self
            .adapter
            .space_for_window_id(window_id)
            .and_then(|space| self.adapter.window_for_space(space))
            .into_iter()
            .collect();
        update.extend_windows(windows);
        update
    }

    pub(crate) fn prepare_viewport_frame_reconciliation(
        &self,
        skip_window_id: Option<WindowId>,
    ) -> Vec<DockViewportFrameSampleRequest> {
        self.adapter
            .prepare_registered_window_fact_samples(skip_window_id)
    }

    pub(crate) fn finalize_viewport_frame_reconciliation(
        &mut self,
        samples: Vec<DockViewportFrameSample>,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        for sample in samples {
            let Some(window) = self.adapter.finalize_registered_window_fact_sample(sample) else {
                continue;
            };
            update.mark_changed(true);
            update.extend_windows([window]);
            update.merge(self.clear_preview_for_unready_window_route(window.window_id()));
        }
        update
    }

    pub(crate) fn clear_preview_for_unready_window_route(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        if self.adapter.window_route_ready(window_id) == Some(false) {
            self.clear_routed_drop_preview_if_window_matches(window_id)
        } else {
            DockViewportRuntimeUpdate::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
            crate::DockDropGuideMetrics::default(),
        )
        .is_some_and(|registration| registration.changed)
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene_frame(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.begin_viewport_host_scene_frame_with_facts(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
            drop_guide_metrics,
            Vec::new(),
        )
    }

    pub(crate) fn begin_viewport_host_scene_frame_with_facts(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
        initial_facts: Vec<DockHostDropSceneFact>,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let registration_key = self.adapter.registration_key(&space)?;
        let snapshot = DockViewportHostSceneDraft::new_with_facts(
            space.clone(),
            window_id,
            window_facts.current_bounds,
            host_geometry,
            host_position,
            drop_guide_metrics,
            initial_facts,
        )
        .bind(registration_key)?;
        self.commit_viewport_host_scene_snapshot(snapshot, window_facts)
    }

    pub(crate) fn commit_viewport_host_scene_snapshot(
        &mut self,
        snapshot: DockViewportHostSceneSnapshot,
        window_facts: DockViewportWindowFacts,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.commit_viewport_host_scene_snapshot_at_update(snapshot, window_facts, None)
    }

    pub(crate) fn commit_viewport_host_scene_snapshot_at_update(
        &mut self,
        snapshot: DockViewportHostSceneSnapshot,
        window_facts: DockViewportWindowFacts,
        scene_update_generation: Option<u64>,
    ) -> Option<DockViewportHostSceneRegistration> {
        if !self.admits_registration(snapshot.registration_key()) {
            return None;
        }
        let space = snapshot.space.clone();
        let window_id = snapshot.window_id;
        let window = self.adapter.window_for_space(&space)?;
        let current_identity = DockViewportIdentity::new(space.clone(), window.window_id());
        if !current_identity.matches(&space, window_id) {
            return None;
        }
        let close_cancelled = if self.adapter.window_close_requested(window_id)
            && self
                .close_coordinator
                .scene_commit_can_cancel_window_close(window_id, scene_update_generation)
        {
            self.cancel_window_close_request(window_id).changed()
        } else {
            false
        };
        let host_geometry = snapshot.host_geometry.clone();
        let snapshot_change = self.update_viewport_snapshot(&space, window_facts, host_geometry);
        let mut registration = self
            .frame_coordinator
            .register_host_scene_snapshot(snapshot);
        registration.changed |= snapshot_change.changed || close_cancelled;
        registration.placement_changed = snapshot_change.placement_changed;
        Some(registration)
    }

    #[cfg(test)]
    pub(crate) fn discard_viewport_host_scene_frame(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        expected_registration: Option<&DockViewportRegistrationKey>,
    ) -> DockViewportRuntimeUpdate {
        let current_registration = self.registration_key_for_space_window(space, window_id);
        if current_registration.as_ref() != expected_registration {
            return DockViewportRuntimeUpdate::default();
        }
        let Some(registration) = current_registration else {
            return DockViewportRuntimeUpdate::default();
        };
        if !self
            .frame_coordinator
            .discard_frame_for_viewport(space, window_id)
        {
            return DockViewportRuntimeUpdate::default();
        }
        self.finalize_discarded_viewport_host_scene(registration)
    }

    pub(crate) fn discard_viewport_host_scene_frame_exact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
    ) -> DockViewportRuntimeUpdate {
        let registration = frame.registration_key();
        if !self.admits_registration(registration)
            || !self.frame_coordinator.discard_exact_frame(frame)
        {
            return DockViewportRuntimeUpdate::default();
        }
        self.finalize_discarded_viewport_host_scene(registration.clone())
    }

    pub(crate) fn is_current_viewport_host_scene_frame(
        &self,
        frame: &DockViewportHostSceneFrame,
    ) -> bool {
        self.admits_registration(frame.registration_key())
            && self.frame_coordinator.host_scenes().is_current_frame(frame)
    }

    fn finalize_discarded_viewport_host_scene(
        &mut self,
        registration: DockViewportRegistrationKey,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(true);
        update.mark_changed(
            self.adapter
                .mark_window_snapshot_stale(registration.window_id()),
        );
        self.status
            .clear_window_references(registration.space(), registration.window_id());
        update.merge(self.clear_preview_for_unready_window_route(registration.window_id()));
        update
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.frame_coordinator.push_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.frame_coordinator.push_frame_fact(frame, fact)
    }

    pub(crate) fn rendered_leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .leaf_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_leaf_displayed_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .leaf_displayed_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .tab_bar_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.frame_coordinator
            .tab_label_bounds_for_tabs(space, window_id, tabs, target_index)
    }

    #[cfg(test)]
    pub(crate) fn rendered_host_drop_scene_for_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockHostDropScene> {
        self.frame_coordinator.scene_for_window(space, window_id)
    }

    fn clear_runtime_window_state(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        cleanup: DockViewportRuntimeWindowStateCleanup,
    ) -> DockViewportRuntimeUpdate {
        let mut update = DockViewportRuntimeUpdate::default();
        update.merge(self.clear_routed_drop_preview_if_window_matches(window_id));
        if cleanup.discard_close_plan() {
            update.mark_changed(self.close_coordinator.discard_window(window_id).changed());
        }
        self.window_ownership.clear_window_state(window_id);
        self.backend_focus.discard_window(window_id);
        self.frame_coordinator.unregister_space(space);
        self.clear_pending_activation_for(space, window_id);
        self.status.clear_window_references(space, window_id);
        if cleanup.focus_cleanup() == DockViewportSpaceFocusCleanup::Remove {
            self.focus.remove_space(space);
        }
        update.merge(self.finish_payload_drag_for_source_space(space));
        update
    }

    pub(crate) fn finish_payload_drag_for_source_space(
        &mut self,
        space: &DockSpaceId,
    ) -> DockViewportRuntimeUpdate {
        let Some(session) = self
            .payload_drag
            .active_session()
            .filter(|session| session.source_space() == space)
            .cloned()
        else {
            return DockViewportRuntimeUpdate::default();
        };
        self.finish_payload_drag(&session)
    }

    fn unregister_space_runtime_state(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<DockViewportUnregisteredSpace> {
        let snapshot = self.adapter.unregister_space(space)?;
        let window = snapshot.window;
        let affected_windows = self
            .clear_runtime_window_state(
                space,
                window.window_id(),
                DockViewportRuntimeWindowStateCleanup::SpaceUnregistered,
            )
            .into_windows();
        Some(DockViewportUnregisteredSpace {
            window,
            affected_windows,
        })
    }

    #[cfg(test)]
    pub(crate) fn unregister_host_for_space(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.unregister_host_for_space_with_cleanup(space, window_id)
            .changed()
    }

    #[cfg(test)]
    pub(crate) fn unregister_host_for_space_with_cleanup(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        if self
            .adapter
            .window_for_space(space)
            .is_none_or(|window| window.window_id() != window_id)
        {
            return DockViewportRuntimeUpdate::default();
        }
        let mut update = self.finish_payload_drag_for_source_space(space);
        if let Some(unregistered) = self.unregister_space_runtime_state(space) {
            let context = self
                .current_work_context(None)
                .expect("test viewport unregister requires an admitted lineage");
            update.mark_viewport_topology(true, context);
            update.extend_windows(unregistered.affected_windows);
            self.retire_window(unregistered.window.window_id());
        }
        update
    }

    #[cfg(test)]
    pub(crate) fn reusable_window_for_space(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindow {
        self.reusable_window_for_space_with_cleanup(space, cx)
            .into_parts()
            .0
    }

    #[cfg(test)]
    pub(crate) fn reusable_window_for_space_with_cleanup(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindowOutcome {
        let applied = self
            .prepare_reusable_window_for_space(space, None)
            .sample(cx);
        self.finalize_reusable_window(applied)
    }

    pub(crate) fn prepare_reusable_window_for_space(
        &self,
        space: &DockSpaceId,
        live_window: Option<AnyWindowHandle>,
    ) -> DockViewportPreparedReusableWindow {
        let Some(window) = self.adapter.window_for_space(space) else {
            return DockViewportPreparedReusableWindow {
                state: DockViewportReusableWindowProbe::Missing,
            };
        };
        if self.adapter.window_close_requested(window.window_id()) {
            return DockViewportPreparedReusableWindow {
                state: DockViewportReusableWindowProbe::Stale,
            };
        }
        let key = self
            .adapter
            .registration_key(space)
            .expect("registered viewport window must have a registration key");
        DockViewportPreparedReusableWindow {
            state: DockViewportReusableWindowProbe::Candidate {
                key,
                window,
                known_live: live_window == Some(window),
            },
        }
    }

    pub(crate) fn finalize_reusable_window(
        &mut self,
        applied: DockViewportAppliedReusableWindow,
    ) -> DockViewportReusableWindowOutcome {
        let (key, window, live) = match applied.state {
            DockViewportReusableWindowObservation::Missing => {
                return DockViewportReusableWindowOutcome::missing();
            }
            DockViewportReusableWindowObservation::Stale => {
                return DockViewportReusableWindowOutcome::stale();
            }
            DockViewportReusableWindowObservation::Candidate { key, window, live } => {
                (key, window, live)
            }
        };
        if !self.admits_registration(&key) || self.adapter.window_close_requested(key.window_id()) {
            return DockViewportReusableWindowOutcome::stale();
        }
        if live {
            return DockViewportReusableWindowOutcome::reused(key, window);
        }

        let Some(unregistered) = self.unregister_space_runtime_state(key.space()) else {
            return DockViewportReusableWindowOutcome::stale();
        };
        let affected_windows = unregistered.affected_windows;
        self.retire_window(unregistered.window.window_id());
        DockViewportReusableWindowOutcome::stale_with_affected_windows(affected_windows)
    }

    #[cfg(test)]
    pub(crate) fn register_opened_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        self.register_opened_viewport_with_cleanup(space, window)
            .expect("test viewport registration must not conflict with a tear-off reservation")
            .window_effects
            .close_now()
            .iter()
            .copied()
            .map(DockViewportWindowCloseEffect::window)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn register_opened_viewport_with_cleanup(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Result<DockViewportRuntimeRegistration, DockActionApplyError> {
        self.register_runtime_viewport(space, window, None)
    }

    pub(crate) fn register_opened_viewport_from_attempt_with_cleanup(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        open_attempt: DockViewportWindowOpenAttemptKey,
    ) -> Result<Option<DockViewportRuntimeRegistration>, DockActionApplyError> {
        self.register_runtime_viewport_from_attempt(space, window, open_attempt, None)
    }

    pub(crate) fn register_opened_viewport_from_attempt_with_cleanup_in_transaction(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        open_attempt: DockViewportWindowOpenAttemptKey,
        surface_transaction: DockSurfaceTransactionId,
    ) -> Result<Option<DockViewportRuntimeRegistration>, DockActionApplyError> {
        self.register_runtime_viewport_from_attempt(
            space,
            window,
            open_attempt,
            Some(surface_transaction),
        )
    }

    #[cfg(test)]
    fn register_runtime_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Result<DockViewportRuntimeRegistration, DockActionApplyError> {
        if self.target_space_is_reserved(&space) {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        self.register_runtime_viewport_unchecked(space, window, surface_transaction)
    }

    fn register_runtime_viewport_from_attempt(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        open_attempt: DockViewportWindowOpenAttemptKey,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Result<Option<DockViewportRuntimeRegistration>, DockActionApplyError> {
        if self.target_space_is_reserved(&space) {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        let Some(lineage) = open_attempt.active_lineage() else {
            return Ok(None);
        };
        let context = DockViewportRuntimeWorkContext::new(lineage, surface_transaction);
        if !self.admits_work_context(context)
            || open_attempt.window_id() != window.window_id()
            || !self.window_ownership.claim_open_attempt(open_attempt)
        {
            return Ok(None);
        }
        self.register_runtime_viewport_after_ownership_claim(space, window, context)
            .map(Some)
    }

    fn register_runtime_viewport_reserved_after_ownership_claim(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        pending: &DockViewportTearOffPending,
        context: DockViewportRuntimeWorkContext,
    ) -> Result<DockViewportRuntimeRegistration, DockActionApplyError> {
        debug_assert_eq!(&space, pending.target_space());
        debug_assert!(self.tear_off_target_reservation_matches(pending, Some(window)));
        self.register_runtime_viewport_after_ownership_claim(space, window, context)
    }

    #[cfg(test)]
    fn register_runtime_viewport_unchecked(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Result<DockViewportRuntimeRegistration, DockActionApplyError> {
        let lineage = self
            .admission
            .get()
            .default_lineage()
            .expect("test runtime registration requires an admitted lineage");
        self.window_ownership.register_runtime_window_with_lineage(
            window,
            lineage,
            DockViewportWindowRole::ManagedViewport,
        );
        self.register_runtime_viewport_after_ownership_claim(
            space,
            window,
            DockViewportRuntimeWorkContext::new(lineage, surface_transaction),
        )
    }

    fn register_runtime_viewport_after_ownership_claim(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        context: DockViewportRuntimeWorkContext,
    ) -> Result<DockViewportRuntimeRegistration, DockActionApplyError> {
        assert!(
            self.admits_work_context(context),
            "an owned runtime viewport requires its exact admitted lineage"
        );
        let lineage = context.lineage();
        self.close_coordinator.invalidate_finalize_for_space(&space);
        let topology_changed = self.adapter.window_for_space(&space) != Some(window)
            || self.adapter.space_for_window_id(window.window_id()) != Some(&space);
        let outcome = self
            .adapter
            .register_viewport_with_outcome(space.clone(), window, lineage)
            .map_err(|_| DockActionApplyError::DropTargetUnavailable)?;
        let cleanup = self.clear_replaced_viewport_mappings(&outcome, &space, window);
        self.backend_focus
            .record_viewport_created(window.window_id());
        let mut runtime_update = DockViewportRuntimeUpdate::default();
        runtime_update.bind_work_context(context);
        runtime_update.mark_viewport_topology(topology_changed, context);
        Ok(DockViewportRuntimeRegistration {
            outcome,
            window_effects: DockViewportWindowEffects::new(
                cleanup.replaced_windows,
                cleanup.affected_windows,
                Vec::new(),
            ),
            runtime_update,
        })
    }

    fn clear_replaced_viewport_mappings(
        &mut self,
        outcome: &DockViewportRegisterOutcome,
        registered_space: &DockSpaceId,
        registered_window: AnyWindowHandle,
    ) -> DockViewportReplacementCleanup {
        let mut cleanup = DockViewportReplacementCleanup::default();
        for removed in outcome.replaced() {
            let affected_windows = self
                .clear_runtime_window_state(
                    &removed.space,
                    removed.window.window_id(),
                    if &removed.space == registered_space {
                        DockViewportRuntimeWindowStateCleanup::ReplacedSameSpaceMapping
                    } else {
                        DockViewportRuntimeWindowStateCleanup::ReplacedDifferentSpaceMapping
                    },
                )
                .into_windows();
            extend_unique_windows(&mut cleanup.affected_windows, affected_windows);
            if removed.window != registered_window {
                let retirement = self.retire_runtime_window_for_close(removed.window);
                if let Some(effect) =
                    DockViewportWindowCloseEffect::from_retirement(removed.window, retirement)
                    && !cleanup.replaced_windows.contains(&effect)
                {
                    cleanup.replaced_windows.push(effect);
                }
            }
        }
        cleanup
    }

    #[cfg(test)]
    pub(crate) fn register_rendered_host_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> bool {
        let context = DockViewportRuntimeWorkContext::new(
            self.admission
                .get()
                .default_lineage()
                .expect("test runtime registration requires an admitted lineage"),
            None,
        );
        self.register_rendered_host_viewport_with_cleanup(space, window, context)
            .changed()
    }

    pub(crate) fn register_rendered_host_viewport_with_cleanup(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
        context: DockViewportRuntimeWorkContext,
    ) -> DockViewportRuntimeUpdate {
        if !self.admits_work_context(context) {
            return DockViewportRuntimeUpdate::default();
        }
        let lineage = context.lineage();
        if self.window_ownership.is_opening(window.window_id()) {
            return DockViewportRuntimeUpdate::default();
        }
        if self.window_ownership.is_retired(window.window_id()) {
            return DockViewportRuntimeUpdate::default();
        }
        if matches!(lineage, crate::DockViewportRuntimeLineage::Surface(_))
            && !self
                .window_ownership
                .owns_window(window.window_id(), lineage)
        {
            return DockViewportRuntimeUpdate::default();
        }
        if self.target_space_is_reserved(&space) {
            return DockViewportRuntimeUpdate::default();
        }
        match self.adapter.window_for_space(&space) {
            Some(existing)
                if existing == window
                    && self
                        .adapter
                        .registration_key(&space)
                        .is_some_and(|key| key.lineage() == lineage) =>
            {
                DockViewportRuntimeUpdate::default()
            }
            Some(_) => DockViewportRuntimeUpdate::default(),
            None => {
                self.close_coordinator.invalidate_finalize_for_space(&space);
                if lineage == crate::DockViewportRuntimeLineage::Unmanaged {
                    self.window_ownership.register_runtime_window_with_lineage(
                        window,
                        lineage,
                        DockViewportWindowRole::ManagedViewport,
                    );
                }
                let Ok(outcome) =
                    self.adapter
                        .register_viewport_with_outcome(space.clone(), window, lineage)
                else {
                    return DockViewportRuntimeUpdate::default();
                };
                let cleanup = self.clear_replaced_viewport_mappings(&outcome, &space, window);
                self.backend_focus
                    .record_viewport_created(window.window_id());
                let mut update = DockViewportRuntimeUpdate::default();
                update.bind_work_context(context);
                update.mark_viewport_topology(true, context);
                update.extend_windows(cleanup.affected_windows);
                update.extend_windows(
                    cleanup
                        .replaced_windows
                        .into_iter()
                        .map(DockViewportWindowCloseEffect::window),
                );
                update
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn deliver_drop_commit_delivery_with_outcome(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let facts = {
            let controller = self.controller.read(cx);
            DockViewportWorkspaceRouteFacts::capture_for_payload(
                controller.workspace(),
                delivery.payload(),
                delivery.source_node(),
            )
        };
        let prepared = self.prepare_payload_drop(delivery, None, None, &facts)?;
        let applied = prepared.apply(cx)?;
        Ok(self.finalize_payload_drop(applied)?.0)
    }

    pub(crate) fn prepare_payload_drop(
        &self,
        delivery: DockDropDelivery,
        live_window: Option<AnyWindowHandle>,
        surface_transaction: Option<DockSurfaceTransactionId>,
        facts: &DockViewportWorkspaceRouteFacts,
    ) -> Result<DockViewportPreparedPayloadDrop, DockActionApplyError> {
        self.validate_payload_drag_session(delivery.drag_session())?;
        let drag_session = delivery
            .drag_session()
            .cloned()
            .ok_or(DockActionApplyError::DropDragSessionMissing)?;
        let work_context =
            DockViewportRuntimeWorkContext::new(drag_session.lineage(), surface_transaction);
        if !self.admits_work_context(work_context) {
            return Err(DockActionApplyError::DropDragSessionStale {
                session: drag_session.id(),
            });
        }
        let DockDropWorkspaceCommit {
            source_space,
            source_node,
            payload,
            target,
            drag_session: delivered_drag_session,
        } = delivery.into_workspace_commit(
            &self.adapter,
            self.frame_coordinator.host_scenes(),
            facts,
        )?;
        debug_assert_eq!(delivered_drag_session.as_ref(), Some(&drag_session));
        let target_space = target.target_space().clone();
        let target_window = self.prepare_reusable_window_for_space(&target_space, live_window);
        let source_registration = self.adapter.registration_key(&source_space);
        Ok(DockViewportPreparedPayloadDrop {
            admission: self.admission.clone(),
            work_context,
            controller: self.controller.clone(),
            source_space,
            target: DockViewportPreparedPayloadDropTarget::Resolved {
                source_node,
                payload,
                target,
            },
            target_space,
            drag_session,
            target_window,
            source_registration,
        })
    }

    pub(crate) fn resolve_locked_workspace_drop_delivery(
        &self,
        delivery: DockDropDelivery,
        facts: &DockViewportWorkspaceRouteFacts,
    ) -> Result<DockDropWorkspaceCommit, DockActionApplyError> {
        self.validate_payload_drag_session(delivery.drag_session())?;
        delivery.into_workspace_commit(&self.adapter, self.frame_coordinator.host_scenes(), facts)
    }

    pub(crate) fn prepare_locked_payload_drop(
        &self,
        locked: DockViewportLockedWorkspaceDrop,
        live_window: Option<AnyWindowHandle>,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Result<DockViewportPreparedPayloadDrop, DockActionApplyError> {
        let DockViewportLockedWorkspaceDrop { plan, drag_session } = locked;
        self.validate_payload_drag_session(Some(&drag_session))?;
        let work_context =
            DockViewportRuntimeWorkContext::new(drag_session.lineage(), surface_transaction);
        if !self.admits_work_context(work_context) {
            return Err(DockActionApplyError::DropDragSessionStale {
                session: drag_session.id(),
            });
        }
        let source_space = plan.source_space().clone();
        let target_space = plan.target_space().clone();
        let target_window = self.prepare_reusable_window_for_space(&target_space, live_window);
        let source_registration = self.adapter.registration_key(&source_space);
        Ok(DockViewportPreparedPayloadDrop {
            admission: self.admission.clone(),
            work_context,
            controller: self.controller.clone(),
            source_space,
            target: DockViewportPreparedPayloadDropTarget::Locked(plan),
            target_space,
            drag_session,
            target_window,
            source_registration,
        })
    }

    pub(crate) fn prepare_atomic_locked_payload_drop(
        &self,
        locked: DockViewportLockedWorkspaceDrop,
        target_window: AnyWindowHandle,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Result<DockViewportPreparedAtomicLockedPayloadDrop, DockActionApplyError> {
        let DockViewportLockedWorkspaceDrop { plan, drag_session } = locked;
        self.validate_payload_drag_session(Some(&drag_session))?;
        let work_context =
            DockViewportRuntimeWorkContext::new(drag_session.lineage(), surface_transaction);
        if !self.admits_work_context(work_context) {
            return Err(DockActionApplyError::DropDragSessionStale {
                session: drag_session.id(),
            });
        }
        let source_space = plan.source_space().clone();
        let target_space = plan.target_space().clone();
        let prepared_target_window =
            self.prepare_reusable_window_for_space(&target_space, Some(target_window));
        let source_registration = self.adapter.registration_key(&source_space);
        Ok(DockViewportPreparedAtomicLockedPayloadDrop {
            admission: self.admission.clone(),
            work_context,
            controller: self.controller.clone(),
            source_space,
            target_space,
            drag_session,
            plan,
            target_window: prepared_target_window,
            expected_target_window: target_window,
            source_registration,
        })
    }

    pub(crate) fn preflight_atomic_locked_payload_drop(
        &self,
        sampled: DockViewportSampledAtomicLockedPayloadDrop,
        cx: &App,
    ) -> Result<DockViewportPreflightedLockedPayloadDrop, DockActionApplyError> {
        let DockViewportSampledAtomicLockedPayloadDrop {
            admission,
            work_context,
            controller,
            source_space,
            target_space,
            drag_session,
            plan,
            target_window,
            expected_target_window,
            source_registration,
        } = sampled;
        let stale = || DockActionApplyError::DropDragSessionStale {
            session: drag_session.id(),
        };
        if !admission.get().admits(work_context.lineage())
            || !self.admits_work_context(work_context)
        {
            return Err(stale());
        }

        let target_registration = match target_window.state {
            DockViewportReusableWindowObservation::Candidate {
                key,
                window,
                live: true,
            } if window == expected_target_window
                && key.window_id() == expected_target_window.window_id()
                && key.space() == &target_space
                && self.admits_registration_in_context(work_context, &key) =>
            {
                key
            }
            DockViewportReusableWindowObservation::Missing
            | DockViewportReusableWindowObservation::Stale
            | DockViewportReusableWindowObservation::Candidate { .. } => {
                return Err(DockActionApplyError::DropTargetUnavailable);
            }
        };

        let frozen_focus_item = drag_session.focus_item().cloned();
        let workspace = controller
            .read(cx)
            .workspace()
            .prepare_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: frozen_focus_item.as_ref(),
            })?;
        let source_is_empty =
            source_space != target_space && workspace.space_is_empty(&source_space);
        if source_is_empty
            && !source_registration.as_ref().is_some_and(|registration| {
                registration.space() == &source_space
                    && registration.window_id() != target_registration.window_id()
                    && self.admits_registration_in_context(work_context, registration)
            })
        {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        if !admission.get().admits(work_context.lineage())
            || !self.admits_work_context(work_context)
            || !self.admits_registration_in_context(work_context, &target_registration)
        {
            return Err(stale());
        }

        Ok(DockViewportPreflightedLockedPayloadDrop {
            work_context,
            controller,
            source_space,
            target_space,
            drag_session,
            workspace,
            target_registration,
            target_window: expected_target_window,
            source_registration,
            source_is_empty,
        })
    }

    pub(crate) fn commit_preflighted_locked_payload_drop(
        &mut self,
        prepared: DockViewportPreflightedLockedPayloadDrop,
        cx: &mut App,
    ) -> DockViewportCommittedLockedPayloadDrop {
        debug_assert!(
            self.can_commit_preflighted_locked_payload_drop(&prepared),
            "preflighted locked payload drop must retain exact runtime authority until commit"
        );
        let DockViewportPreflightedLockedPayloadDrop {
            work_context,
            controller,
            source_space,
            target_space,
            drag_session,
            workspace,
            target_registration,
            target_window,
            source_registration,
            source_is_empty,
        } = prepared;
        let vacated_source = self.commit_preflighted_vacated_payload_drop_source(
            &source_space,
            &target_space,
            source_registration,
            source_is_empty,
        );
        let reusable =
            DockViewportReusableWindowOutcome::reused(target_registration, target_window);
        let drop_outcome = controller.update(cx, |controller, cx| {
            let outcome = controller
                .workspace_mut()
                .commit_prepared_locked_payload_drop(workspace);
            if outcome.changed() {
                cx.notify();
            }
            outcome
        });
        let focus_request = drop_outcome.focus_item().cloned().map_or_else(
            DockViewportFocusRequest::no_panel_focus,
            DockViewportFocusRequest::panel,
        );
        let (activation, reusable_effects) =
            DockViewportWindowLifecycleController::drop_activation(reusable, focus_request);
        let mut runtime_update = DockViewportRuntimeUpdate::default();
        runtime_update.bind_work_context(work_context);
        runtime_update.mark_graph_commit(drop_outcome.changed(), work_context);
        runtime_update.mark_viewport_topology(vacated_source.changed(), work_context);
        let window_effects = reusable_effects.merge(DockViewportWindowEffects::new(
            Vec::new(),
            vacated_source.affected_windows,
            vacated_source.windows,
        ));
        let outcome = DockViewportDropRouteOutcome::Action(DockViewportDropActionOutcome::new(
            drop_outcome.action(),
            activation,
        ));
        self.status.record_drop_result(&Ok(outcome.clone()));
        runtime_update.merge(self.clear_routed_drop_preview_for_drag_session(Some(&drag_session)));
        DockViewportCommittedLockedPayloadDrop {
            outcome,
            runtime_update,
            window_effects,
        }
    }

    fn can_commit_preflighted_locked_payload_drop(
        &self,
        prepared: &DockViewportPreflightedLockedPayloadDrop,
    ) -> bool {
        self.admits_work_context(prepared.work_context)
            && self.admits_registration_in_context(
                prepared.work_context,
                &prepared.target_registration,
            )
            && prepared.target_registration.window_id() == prepared.target_window.window_id()
            && (!prepared.source_is_empty
                || prepared
                    .source_registration
                    .as_ref()
                    .is_some_and(|registration| {
                        registration.window_id() != prepared.target_registration.window_id()
                            && self
                                .admits_registration_in_context(prepared.work_context, registration)
                    }))
    }

    fn commit_preflighted_vacated_payload_drop_source(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        source_registration: Option<DockViewportRegistrationKey>,
        source_is_empty: bool,
    ) -> DockViewportVacatedPayloadDropSource {
        if source_space == target_space || !source_is_empty {
            return DockViewportVacatedPayloadDropSource::default();
        }
        let source_registration = source_registration
            .expect("preflighted empty source must retain an exact viewport registration");
        debug_assert_eq!(source_registration.space(), source_space);
        let unregistered = self
            .unregister_space_runtime_state(source_space)
            .expect("preflighted empty source registration must remain current until commit");
        let windows = DockViewportWindowCloseEffect::from_retirement(
            unregistered.window,
            self.retire_runtime_window_for_close(unregistered.window),
        )
        .into_iter()
        .collect();
        DockViewportVacatedPayloadDropSource {
            changed: true,
            windows,
            affected_windows: unregistered.affected_windows,
        }
    }

    pub(crate) fn finalize_payload_drop(
        &mut self,
        applied: DockViewportAppliedPayloadDrop,
    ) -> Result<(DockViewportDropRouteOutcome, DockViewportRuntimeUpdate), DockActionApplyError>
    {
        let DockViewportAppliedPayloadDrop {
            work_context,
            source_space,
            target_space,
            drag_session,
            drop_outcome,
            focus_request,
            target_window,
            source_registration,
            source_is_empty,
        } = applied;
        if !self.admits_work_context(work_context) {
            return Err(DockActionApplyError::DropDragSessionStale {
                session: drag_session.id(),
            });
        }
        let reusable = self.finalize_reusable_window(target_window);
        let reusable_topology_changed = reusable.topology_changed();
        let (activation, reusable_effects) =
            DockViewportWindowLifecycleController::drop_activation(reusable, focus_request);
        let vacated_source = self.finalize_vacated_payload_drop_source(
            &source_space,
            &target_space,
            source_registration.as_ref(),
            source_is_empty,
        );
        let mut runtime_update = DockViewportRuntimeUpdate::default();
        runtime_update.bind_work_context(work_context);
        runtime_update.mark_graph_commit(drop_outcome.changed(), work_context);
        runtime_update.mark_viewport_topology(
            reusable_topology_changed || vacated_source.changed(),
            work_context,
        );
        let window_effects = reusable_effects.merge(DockViewportWindowEffects::new(
            Vec::new(),
            vacated_source.affected_windows,
            vacated_source.windows,
        ));
        let outcome = DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(drop_outcome.action(), activation)
                .with_window_effects(window_effects),
        );
        self.status.record_drop_result(&Ok(outcome.clone()));
        runtime_update.merge(self.clear_routed_drop_preview_for_drag_session(Some(&drag_session)));
        Ok((outcome, runtime_update))
    }
    #[cfg(test)]
    pub(crate) fn validate_payload_drop_delivery(
        &self,
        delivery: &DockDropDelivery,
        cx: &App,
    ) -> Result<(), DockActionApplyError> {
        self.validate_payload_drag_session(delivery.drag_session())?;
        let controller = self.controller.read(cx);
        delivery.validate_current_workspace_target(
            &self.adapter,
            self.frame_coordinator.host_scenes(),
            controller.workspace(),
        )
    }

    pub(crate) fn record_drop_route_result(
        &mut self,
        result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) {
        self.status.record_drop_result(result);
    }

    pub(crate) fn record_tear_off_outcome(&mut self, outcome: &DockViewportTearOffOpenOutcome) {
        self.status.record_tear_off(outcome);
    }

    pub(crate) fn record_platform_dispatch(&mut self, record: DockViewportPlatformSyncRecord) {
        self.status.record_platform_dispatch(record);
    }

    pub(crate) fn record_platform_observation(
        &mut self,
        window_id: WindowId,
        observation: crate::DockViewportPlatformSyncObservation,
    ) {
        self.status
            .record_platform_observation(window_id, observation);
    }

    pub(crate) fn record_visual_affordance_status(
        &mut self,
        space: DockSpaceId,
        window_id: WindowId,
        summary: crate::DockVisualAffordanceDebugSummary,
    ) {
        self.status
            .record_visual_affordance(space, window_id, summary);
    }

    pub(crate) fn clear_visual_affordance_status(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) {
        self.status.clear_visual_affordance(space, window_id);
    }

    pub(crate) fn prepare_empty_payload_drop_source_vacate(
        &self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> DockViewportPreparedSourceVacate {
        let source_registration = (source_space != target_space)
            .then(|| self.adapter.registration_key(source_space))
            .flatten();
        DockViewportPreparedSourceVacate {
            source_space: source_space.clone(),
            target_space: target_space.clone(),
            source_registration,
        }
    }

    pub(crate) fn finalize_empty_payload_drop_source_vacate(
        &mut self,
        applied: DockViewportAppliedSourceVacate,
    ) -> (DockViewportWindowEffects, bool) {
        let DockViewportAppliedSourceVacate {
            prepared:
                DockViewportPreparedSourceVacate {
                    source_space,
                    target_space,
                    source_registration,
                },
            source_is_empty,
        } = applied;
        let vacated_source = self.finalize_vacated_payload_drop_source(
            &source_space,
            &target_space,
            source_registration.as_ref(),
            source_is_empty,
        );
        let changed = vacated_source.changed();
        (
            DockViewportWindowEffects::new(
                Vec::new(),
                vacated_source.affected_windows,
                vacated_source.windows,
            ),
            changed,
        )
    }

    fn finalize_vacated_payload_drop_source(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
        source_registration: Option<&DockViewportRegistrationKey>,
        source_is_empty: bool,
    ) -> DockViewportVacatedPayloadDropSource {
        if source_space == target_space || !source_is_empty {
            return DockViewportVacatedPayloadDropSource::default();
        }
        let Some(source_registration) = source_registration else {
            return DockViewportVacatedPayloadDropSource::default();
        };
        if !self.admits_registration(source_registration) {
            return DockViewportVacatedPayloadDropSource::default();
        }
        let Some(unregistered) = self.unregister_space_runtime_state(source_space) else {
            return DockViewportVacatedPayloadDropSource::default();
        };
        let windows = DockViewportWindowCloseEffect::from_retirement(
            unregistered.window,
            self.retire_runtime_window_for_close(unregistered.window),
        )
        .into_iter()
        .collect();
        DockViewportVacatedPayloadDropSource {
            changed: true,
            windows,
            affected_windows: unregistered.affected_windows,
        }
    }

    pub(crate) fn prepare_tear_off_drop_delivery(
        &mut self,
        request: DockViewportTearOffRequest,
        graph_spaces: &[DockSpaceId],
    ) -> Result<DockViewportPreparedTearOffDropProbe, DockActionApplyError> {
        self.validate_payload_drag_session(request.drag_session())?;
        let options = self.tear_off_window_options(&request)?;
        let target_space = self.next_tear_off_space(&request, graph_spaces);
        Ok(DockViewportPreparedTearOffDropProbe {
            controller: self.controller.clone(),
            request,
            target_space,
            options,
        })
    }

    #[cfg(test)]
    pub(crate) fn prepare_tear_off_drop_route_for_test(
        &self,
        request: DockViewportTearOffRequest,
        target_space: DockSpaceId,
        options: WindowOptions,
    ) -> DockViewportPreparedTearOffDropProbe {
        DockViewportPreparedTearOffDropProbe {
            controller: self.controller.clone(),
            request,
            target_space,
            options,
        }
    }

    fn next_tear_off_space(
        &mut self,
        request: &DockViewportTearOffRequest,
        graph_spaces: &[DockSpaceId],
    ) -> DockSpaceId {
        loop {
            let space_index = self.next_tear_off_space_index();
            let space = DockSpaceId::new(format!(
                "{}:tear-off:{}:{}",
                request.source_space(),
                request.payload().label(),
                space_index
            ));
            let graph_has_space = graph_spaces.iter().any(|known| known == &space);
            if !graph_has_space && self.adapter.window_for_space(&space).is_none() {
                return space;
            }
        }
    }

    pub(crate) fn tear_off_window_options(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Result<WindowOptions, DockActionApplyError> {
        let window_bounds = self
            .tear_off_window_placement(request)
            .ok_or(DockActionApplyError::TearOffViewportPlacementUnavailable)?
            .window_bounds();

        Ok(WindowOptions {
            window_bounds: Some(window_bounds),
            // Tear-off viewports are activated after graph commit and runtime registration, so
            // panel focus restoration flows through the explicit activation transaction.
            focus_on_appearing: false,
            ..Default::default()
        })
    }

    pub(crate) fn tear_off_window_placement(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Option<DockViewportTearOffPlacement> {
        DockViewportTearOffPlacementPolicy::default().resolve(request)
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.frame_coordinator.screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        policy: &crate::DockPolicy,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let window = self.adapter.window_for_space(space)?;
        if self
            .adapter
            .snapshot_facts_generation(space, window.window_id())
            .is_none()
        {
            return None;
        }
        self.frame_coordinator.host_scenes().resolve_for_window(
            space,
            Some(window.window_id()),
            host_position,
            policy,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockViewportIdentity> {
        let session = session?;
        self.payload_drag
            .last_routed_viewport_identity(Some(session))
            .cloned()
    }

    pub(crate) fn begin_tear_off_request_with_focus(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
    ) -> DockViewportTearOffBeginOutcome {
        self.begin_tear_off_request_with_focus_and_plan(request, target_space, focus_item, None)
    }

    fn begin_tear_off_request_with_focus_and_plan(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
        move_plan: Option<crate::viewport_tear_off_move::DockViewportTearOffMovePlan>,
    ) -> DockViewportTearOffBeginOutcome {
        let target_space = target_space.into();
        let source_window = self.adapter.window_for_space(request.source_space());
        let source_registration = self.adapter.registration_key(request.source_space());
        let outcome = self.tear_off.begin_with_move_plan(
            request,
            target_space,
            source_window,
            source_registration,
            focus_item,
            move_plan,
        );
        if let DockViewportTearOffBeginOutcome::Pending(pending) = &outcome
            && !self.reserve_tear_off_target(pending)
        {
            let _ = self
                .tear_off
                .cancel_matching(pending, DockViewportTearOffCancelReason::Cancelled);
        }
        outcome
    }

    pub(crate) fn begin_prepared_tear_off_drop(
        &mut self,
        prepared: DockViewportPreparedTearOffDrop,
    ) -> DockViewportPreparedTearOffBegin {
        let DockViewportPreparedTearOffDrop {
            request,
            target_space,
            focus_item,
            options,
            move_plan,
        } = prepared;
        match self.begin_tear_off_request_with_focus_and_plan(
            request,
            target_space,
            focus_item,
            Some(move_plan),
        ) {
            DockViewportTearOffBeginOutcome::Pending(pending) => {
                if self.tear_off_target_reservation_matches(&pending, None) {
                    DockViewportPreparedTearOffBegin::Pending(DockViewportPreparedTearOffWindow {
                        pending,
                        options,
                    })
                } else {
                    DockViewportPreparedTearOffBegin::Unavailable(pending)
                }
            }
            DockViewportTearOffBeginOutcome::Duplicate(pending) => {
                DockViewportPreparedTearOffBegin::Duplicate(pending)
            }
        }
    }

    pub(crate) fn cancel_tear_off_pending(
        &mut self,
        pending: &DockViewportTearOffPending,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        let cancelled = self.tear_off.cancel_matching(pending, reason);
        if let Some(cancelled) = &cancelled {
            self.release_tear_off_target_reservation(cancelled.pending());
        }
        cancelled
    }

    fn reserve_tear_off_target(&mut self, pending: &DockViewportTearOffPending) -> bool {
        let target_space = pending.target_space();
        if self.adapter.window_for_space(target_space).is_some() {
            return false;
        }
        match self.tear_off_target_reservations.get(target_space) {
            Some(reservation) => reservation.pending == *pending,
            None => {
                self.tear_off_target_reservations.insert(
                    target_space.clone(),
                    DockViewportTearOffTargetReservation {
                        pending: pending.clone(),
                        opening_window: None,
                    },
                );
                true
            }
        }
    }

    pub(crate) fn bind_tear_off_target_window(
        &mut self,
        pending: &DockViewportTearOffPending,
        window: AnyWindowHandle,
    ) -> bool {
        let Some(reservation) = self
            .tear_off_target_reservations
            .get_mut(pending.target_space())
        else {
            return false;
        };
        if reservation.pending != *pending {
            return false;
        }
        match reservation.opening_window {
            Some(existing) => existing == window,
            None => {
                reservation.opening_window = Some(window);
                true
            }
        }
    }

    fn tear_off_target_reservation_matches(
        &self,
        pending: &DockViewportTearOffPending,
        window: Option<AnyWindowHandle>,
    ) -> bool {
        self.tear_off_target_reservations
            .get(pending.target_space())
            .is_some_and(|reservation| {
                reservation.pending == *pending
                    && window.is_none_or(|window| reservation.opening_window == Some(window))
            })
    }

    fn release_tear_off_target_reservation(&mut self, pending: &DockViewportTearOffPending) {
        if self
            .tear_off_target_reservations
            .get(pending.target_space())
            .is_some_and(|reservation| reservation.pending == *pending)
        {
            self.tear_off_target_reservations
                .remove(pending.target_space());
        }
    }

    fn target_space_is_reserved(&self, space: &DockSpaceId) -> bool {
        self.tear_off_target_reservations.contains_key(space)
    }

    pub(crate) fn prepare_tear_off_move_apply(
        &mut self,
        pending: &DockViewportTearOffPending,
    ) -> Result<DockViewportPreparedTearOffMoveApply, DockActionApplyError> {
        if !self.tear_off.begin_apply(pending) {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        Ok(DockViewportPreparedTearOffMoveApply {
            controller: self.controller.clone(),
            pending: pending.clone(),
        })
    }

    pub(crate) fn finalize_tear_off_move_apply(
        &mut self,
        applied: DockViewportAppliedTearOffMove,
    ) -> Result<(DockViewportCommittedTearOffMove, bool), DockActionApplyError> {
        let DockViewportAppliedTearOffMove { pending, result } = applied;
        match result {
            Ok((action, source_is_empty)) => self
                .tear_off
                .finish_apply(&pending, action)
                .map(|committed| (committed, source_is_empty))
                .ok_or(DockActionApplyError::DropTargetUnavailable),
            Err(error) => {
                let _ = self
                    .cancel_tear_off_pending(&pending, DockViewportTearOffCancelReason::Cancelled);
                Err(error)
            }
        }
    }

    pub(crate) fn prepare_tear_off_target_claim(
        &self,
        pending: &DockViewportTearOffPending,
        window: AnyWindowHandle,
        open_attempt: DockViewportWindowOpenAttemptKey,
    ) -> Result<DockViewportPreparedTearOffTargetClaim, DockActionApplyError> {
        let target_space = pending.target_space();
        if open_attempt.window_id() != window.window_id()
            || !self.tear_off_target_reservation_matches(pending, Some(window))
            || self.adapter.window_for_space(target_space).is_some()
        {
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        Ok(DockViewportPreparedTearOffTargetClaim {
            controller: self.controller.clone(),
            target_registration_generation: self.adapter.last_registration_generation(target_space),
            pending: pending.clone(),
            window,
            open_attempt,
        })
    }

    pub(crate) fn finalize_tear_off_target_claim(
        &mut self,
        applied: DockViewportAppliedTearOffTargetClaim,
    ) -> Result<DockViewportClaimedTearOffTarget, DockActionApplyError> {
        let target_space = applied.pending.target_space();
        let context = applied
            .open_attempt
            .active_lineage()
            .map(|lineage| DockViewportRuntimeWorkContext::new(lineage, None));
        if !applied.target_graph_is_vacant {
            let target_space = target_space.clone();
            self.release_tear_off_target_reservation(&applied.pending);
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space,
            }
            .into());
        }
        if context.is_none_or(|context| !self.admits_work_context(context))
            || !self.tear_off_target_reservation_matches(&applied.pending, Some(applied.window))
            || self.adapter.window_for_space(target_space).is_some()
            || self.adapter.last_registration_generation(target_space)
                != applied.target_registration_generation
            || applied.open_attempt.window_id() != applied.window.window_id()
            || !self
                .window_ownership
                .claim_open_attempt(applied.open_attempt)
        {
            self.release_tear_off_target_reservation(&applied.pending);
            return Err(DockActionApplyError::DropTargetUnavailable);
        }
        let registration = self.register_runtime_viewport_reserved_after_ownership_claim(
            target_space.clone(),
            applied.window,
            &applied.pending,
            context.expect("admitted tear-off claim requires an exact work context"),
        )?;
        Ok(DockViewportClaimedTearOffTarget {
            pending: applied.pending,
            window: applied.window,
            open_attempt: applied.open_attempt,
            registration,
        })
    }

    pub(crate) fn rollback_tear_off_target_claim(
        &mut self,
        claimed: DockViewportClaimedTearOffTarget,
    ) -> DockViewportRolledBackTearOffTarget {
        let registration_key = claimed.registration.outcome.registration_key();
        let affected_windows = if self.admits_registration(registration_key) {
            self.unregister_space_runtime_state(registration_key.space())
                .map(|unregistered| unregistered.affected_windows)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let _ = self
            .cancel_tear_off_pending(&claimed.pending, DockViewportTearOffCancelReason::Cancelled);
        self.release_tear_off_target_reservation(&claimed.pending);
        DockViewportRolledBackTearOffTarget {
            window: claimed.window,
            open_attempt: claimed.open_attempt,
            window_effects: claimed
                .registration
                .window_effects
                .merge(DockViewportWindowEffects::refresh_only(affected_windows)),
        }
    }

    pub(crate) fn prepare_tear_off_source_check(
        &self,
        pending: &DockViewportTearOffPending,
    ) -> DockViewportPreparedTearOffSourceCheck {
        DockViewportPreparedTearOffSourceCheck {
            controller: self.controller.clone(),
            pending: pending.clone(),
        }
    }

    pub(crate) fn finalize_tear_off_source_check(
        &mut self,
        applied: DockViewportAppliedTearOffSourceCheck,
    ) -> Option<DockViewportTearOffCancelled> {
        let source_registration_is_current = self
            .adapter
            .registration_key(applied.pending.request().source_space())
            .as_ref()
            == applied.pending.source_registration();
        match (source_registration_is_current, applied.source_status) {
            (true, DockViewportTearOffSourceStatus::Ready) => None,
            (false, _) | (_, DockViewportTearOffSourceStatus::Unavailable) => self
                .cancel_tear_off_pending(
                    &applied.pending,
                    DockViewportTearOffCancelReason::SourceUnavailable,
                ),
        }
    }

    pub(crate) fn complete_tear_off_registration(
        &mut self,
        committed: DockViewportCommittedTearOffMove,
        claimed: DockViewportClaimedTearOffTarget,
        source_is_empty: bool,
    ) -> Result<
        (DockViewportTearOffCompleted, DockViewportRuntimeUpdate),
        (DockActionApplyError, DockViewportClaimedTearOffTarget),
    > {
        let registration_key = claimed.registration.outcome.registration_key();
        if committed.pending() != &claimed.pending || !self.admits_registration(registration_key) {
            self.release_tear_off_target_reservation(&claimed.pending);
            return Err((DockActionApplyError::DropTargetUnavailable, claimed));
        }
        let commit = committed.into_commit();
        let vacated_source =
            self.vacate_empty_tear_off_source_viewport(&commit.pending, source_is_empty);
        self.release_tear_off_target_reservation(&commit.pending);
        let DockViewportRuntimeRegistration {
            outcome,
            window_effects,
            mut runtime_update,
        } = claimed.registration;
        let work_context = runtime_update
            .work_context()
            .expect("tear-off registration updates require an exact work context");
        runtime_update.mark_viewport_topology(vacated_source.changed, work_context);
        Ok((
            DockViewportTearOffCompleted::new(
                commit.pending,
                outcome,
                window_effects.close_now().to_vec(),
                window_effects.refresh().to_vec(),
                vacated_source.windows,
                vacated_source.affected_windows,
                commit.action,
            ),
            runtime_update,
        ))
    }

    fn vacate_empty_tear_off_source_viewport(
        &mut self,
        pending: &DockViewportTearOffPending,
        source_is_empty: bool,
    ) -> DockViewportVacatedTearOffSource {
        let source_space = pending.request().source_space();
        if source_space == pending.target_space() {
            return DockViewportVacatedTearOffSource::default();
        }
        if !source_is_empty {
            return DockViewportVacatedTearOffSource::default();
        }
        let Some(source_registration) = pending.source_registration() else {
            return DockViewportVacatedTearOffSource::default();
        };
        if !self.admits_registration(source_registration) {
            return DockViewportVacatedTearOffSource::default();
        }
        let (window, affected_windows, changed) =
            if let Some(unregistered) = self.unregister_space_runtime_state(source_space) {
                (
                    Some(unregistered.window),
                    unregistered.affected_windows,
                    true,
                )
            } else {
                (pending.source_window(), Vec::new(), false)
            };
        let Some(window) = window else {
            return DockViewportVacatedTearOffSource {
                changed,
                windows: Vec::new(),
                affected_windows,
            };
        };
        let windows = DockViewportWindowCloseEffect::from_retirement(
            window,
            self.retire_runtime_window_for_close(window),
        )
        .into_iter()
        .collect();
        DockViewportVacatedTearOffSource {
            changed,
            windows,
            affected_windows,
        }
    }

    pub(crate) fn prepare_drag_focus_item(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockViewportPreparedDragFocusItem> {
        let focused_item = self.focus.focused_panel(&payload.source_space)?;
        Some(DockViewportPreparedDragFocusItem {
            controller: self.controller.clone(),
            payload: payload.clone(),
            focused_item: focused_item.clone(),
        })
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    #[cfg(test)]
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let close = self.cleanup_closed_window(window_id);
        self.status.record_close(&close.outcome);
        close.outcome
    }

    fn cleanup_closed_window(&mut self, window_id: WindowId) -> DockViewportClosedWindowRefresh {
        self.retire_window(window_id);
        let outcome = self.adapter.handle_window_closed(window_id);
        let affected_windows = if let Some(space) = outcome.space().cloned() {
            self.clear_runtime_window_state(
                &space,
                window_id,
                DockViewportRuntimeWindowStateCleanup::ClosedWindow,
            )
            .into_windows()
        } else {
            self.frame_coordinator.unregister_window_scene(window_id);
            self.clear_routed_drop_preview_if_window_matches(window_id)
                .into_windows()
        };
        DockViewportClosedWindowRefresh::new(
            outcome,
            DockViewportWindowEffects::refresh_only(affected_windows),
        )
    }

    #[cfg(test)]
    pub(crate) fn handle_window_closed_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        self.handle_window_closed_with_app_and_refresh(window_id, cx)
            .outcome
    }

    pub(crate) fn prepare_window_closed(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportPreparedWindowClose {
        let pending_state = self.close_coordinator.take_window_close_state(window_id);
        let close = self.cleanup_closed_window(window_id);
        let finalize_key = close
            .outcome
            .space()
            .cloned()
            .map(|space| self.close_coordinator.begin_finalize(&space));
        DockViewportPreparedWindowClose {
            controller: self.controller.clone(),
            close,
            pending_state,
            finalize_key,
        }
    }

    pub(crate) fn finalize_window_closed(
        &mut self,
        applied: DockViewportAppliedWindowClose,
    ) -> DockViewportFinalizedWindowClose {
        let DockViewportAppliedWindowClose {
            prepared:
                DockViewportPreparedWindowClose {
                    close,
                    pending_state,
                    finalize_key,
                    ..
                },
            merge_back_status,
        } = applied;
        let close = DockViewportWindowLifecycleController::complete_pending_close_plan(
            close,
            pending_state,
            |_| {
                merge_back_status
                    .expect("a pending merge-back close plan must be applied before finalize")
            },
        );
        let disposition = if finalize_key
            .map(|key| self.close_coordinator.finish_finalize(key))
            .unwrap_or(true)
        {
            self.status.record_close(&close.outcome);
            DockViewportCloseFinalizeDisposition::Current
        } else {
            DockViewportCloseFinalizeDisposition::Stale
        };
        DockViewportFinalizedWindowClose { close, disposition }
    }

    #[cfg(test)]
    pub(crate) fn handle_window_closed_with_app_and_refresh(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportClosedWindowRefresh {
        let applied = self.prepare_window_closed(window_id).apply_merge_back(cx);
        self.finalize_window_closed(applied).into_refresh()
    }

    pub(crate) fn prepare_close_recovery_window(
        &self,
        outcome: &DockViewportCloseOutcome,
    ) -> Option<DockViewportPreparedReusableWindow> {
        let Some(target_space) = outcome.merge_target_space().cloned() else {
            return None;
        };
        Some(self.prepare_reusable_window_for_space(&target_space, None))
    }

    pub(crate) fn finalize_close_recovery_activation(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        applied: Option<DockViewportAppliedReusableWindow>,
    ) -> DockViewportCloseRecoveryActivation {
        let Some(applied) = applied else {
            return DockViewportCloseRecoveryActivation::none();
        };
        DockViewportWindowLifecycleController::close_recovery_activation(
            outcome,
            self.finalize_reusable_window(applied),
        )
    }

    #[cfg(test)]
    pub(crate) fn prepare_window_should_close(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportPreparedShouldClose {
        self.prepare_window_should_close_at_update(window_id, None)
    }

    pub(crate) fn prepare_window_should_close_at_update(
        &mut self,
        window_id: WindowId,
        request_update_generation: Option<u64>,
    ) -> DockViewportPreparedShouldClose {
        let close_already_requested = self.adapter.window_close_requested(window_id);
        let close_policy = self.close_policy();
        let outcome = if close_already_requested {
            self.allowed_should_close_outcome(window_id)
        } else {
            self.adapter
                .should_close_viewport(window_id, close_policy.clone())
        };
        let expected_registration = outcome.space.as_ref().and_then(|space| {
            self.adapter
                .registration_key(space)
                .filter(|key| key.window_id() == window_id)
        });
        let focused_item = outcome
            .space
            .as_ref()
            .and_then(|space| self.focus.focused_panel(space).cloned());
        let finalize_key = self.close_coordinator.begin_should_close(window_id);
        DockViewportPreparedShouldClose {
            controller: self.controller.clone(),
            expected_registration,
            finalize_key,
            request_update_generation,
            outcome,
            close_policy,
            focused_item,
            close_already_requested,
        }
    }

    pub(crate) fn finalize_window_should_close(
        &mut self,
        applied: DockViewportAppliedShouldClose,
    ) -> DockViewportFinalizedShouldClose {
        let DockViewportAppliedShouldClose {
            expected_registration,
            finalize_key,
            request_update_generation,
            outcome,
            plan_mutation,
            invalidate_route,
        } = applied;
        let current_registration = self
            .adapter
            .space_for_window_id(outcome.window_id)
            .and_then(|space| {
                self.adapter
                    .registration_key(space)
                    .filter(|key| key.window_id() == outcome.window_id)
            });
        let is_current = self.close_coordinator.finish_should_close(finalize_key)
            && current_registration == expected_registration;
        if !is_current {
            return DockViewportFinalizedShouldClose {
                should_close: DockViewportShouldCloseRefresh::new(
                    outcome,
                    DockViewportWindowEffects::default(),
                ),
                disposition: DockViewportCloseFinalizeDisposition::Stale,
            };
        }
        if let DockViewportShouldClosePlanMutation::Replace(plan) = plan_mutation {
            let request_update_generation = request_update_generation.filter(|_| {
                invalidate_route && outcome.status == DockViewportShouldCloseStatus::Allowed
            });
            self.close_coordinator.replace_window_close_plan(
                outcome.window_id,
                plan,
                request_update_generation,
            );
        }
        let affected_windows = if invalidate_route {
            self.apply_allowed_should_close_route_invalidation(&outcome)
                .into_windows()
        } else {
            Vec::new()
        };
        self.status.record_should_close(&outcome);
        DockViewportFinalizedShouldClose {
            should_close: DockViewportShouldCloseRefresh::new(
                outcome,
                DockViewportWindowEffects::refresh_only(affected_windows),
            ),
            disposition: DockViewportCloseFinalizeDisposition::Current,
        }
    }

    #[cfg(test)]
    pub(crate) fn handle_window_should_close_with_app_and_refresh(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseRefresh {
        let applied = self
            .prepare_window_should_close_at_update(window_id, Some(cx.current_update_generation()))
            .apply(cx);
        self.finalize_window_should_close(applied).into_refresh()
    }

    fn allowed_should_close_outcome(&self, window_id: WindowId) -> DockViewportShouldCloseOutcome {
        DockViewportShouldCloseOutcome {
            space: self.adapter.space_for_window_id(window_id).cloned(),
            window_id,
            status: DockViewportShouldCloseStatus::Allowed,
        }
    }

    fn apply_allowed_should_close_route_invalidation(
        &mut self,
        outcome: &DockViewportShouldCloseOutcome,
    ) -> DockViewportRuntimeUpdate {
        if outcome.status == crate::DockViewportShouldCloseStatus::Allowed {
            return self.mark_viewport_window_close_requested(outcome.window_id);
        }
        DockViewportRuntimeUpdate::default()
    }

    fn next_tear_off_space_index(&mut self) -> u64 {
        let index = self.next_tear_off_space_index;
        self.next_tear_off_space_index = self.next_tear_off_space_index.saturating_add(1);
        index
    }

    /// Exports serializable placement snapshots from the adapter.
    pub(crate) fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Checks saved placement snapshots against registered viewport windows.
    pub(crate) fn check_placement_restore(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        let readiness = self.adapter.check_placement_restore(placement)?;
        self.status.record_placement_restore(Some(readiness));
        Ok(readiness)
    }
}

#[cfg(test)]
mod lineage_drop_tests {
    use super::*;
    use crate::{
        DockViewportDropPayload,
        host_test_support::item,
        host_viewport_runtime_test_support::{
            DockViewportHostSceneSeed, DockViewportRuntimeFixture,
            hovered_window_route_request_for_test,
        },
        interaction::DockPayloadDropReleaseOrigin,
        surface::window_session::{
            DockSurfaceWindowSession, DockSurfaceWindowSessionBeginShutdownOutcome,
            DockSurfaceWindowSessionLease, DockSurfaceWindowSessionRuntimeEmptyOutcome,
            DockSurfaceWindowSessionShutdownConvergenceOutcome,
            DockSurfaceWindowSessionShutdownReason, DockSurfaceWindowSessionTerminalDisposition,
            DockSurfaceWindowSessionTerminalOutcome,
        },
        viewport_test_support::handle,
    };
    use open_gpui::{AppContext as _, EntityId, TestAppContext, WindowId};

    fn active_surface_lease(
        session: &mut DockSurfaceWindowSession,
        anchor: WindowId,
    ) -> DockSurfaceWindowSessionLease {
        let opening = session
            .reserve_opening()
            .expect("the surface session should reserve an opening generation");
        session
            .commit_opening(opening, anchor)
            .expect("the reserved surface generation should activate")
    }

    #[open_gpui::test]
    fn live_undock_logical_close_delegation_cannot_remove_replacement_registration(
        cx: &mut TestAppContext,
    ) {
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(target_space.clone())
            .space(target_space.clone(), ["a"])
            .build_controller(cx);
        let mut runtime = DockViewportRuntime::new(fixture.controller);
        let stale =
            runtime.replace_adapter_registration_for_test(target_space.clone(), handle(1001));
        let replacement_window = handle(1002);
        let replacement =
            runtime.replace_adapter_registration_for_test(target_space.clone(), replacement_window);

        assert!(
            runtime
                .settle_live_undock_committed_destination_logical_close(&stale)
                .is_none(),
            "a delayed logical close must not detach a replacement registration"
        );
        assert!(runtime.admits_registration(&replacement));
        assert_eq!(
            runtime.adapter().window_for_space(&target_space),
            Some(replacement_window)
        );

        let closed = runtime
            .settle_live_undock_committed_destination_logical_close(&replacement)
            .expect("the exact active registration should settle");
        assert_eq!(closed.outcome.status(), DockViewportCloseStatus::Closed);
        assert_eq!(closed.outcome.space(), Some(&target_space));
        assert_eq!(runtime.adapter().window_for_space(&target_space), None);
    }

    fn close_surface_generation(
        runtime: &mut DockViewportRuntime,
        session: &mut DockSurfaceWindowSession,
        lease: DockSurfaceWindowSessionLease,
        expected_window: AnyWindowHandle,
    ) {
        assert_eq!(
            session.begin_shutdown(
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                std::iter::empty(),
            ),
            DockSurfaceWindowSessionBeginShutdownOutcome::Started {
                terminal_ticket_count: 1,
            }
        );
        let shutdown_reservation = runtime
            .freeze_surface_shutdown(lease)
            .expect("the active surface generation should freeze");
        assert_eq!(
            shutdown_reservation
                .windows()
                .iter()
                .map(|(_, window)| window.window_id())
                .collect::<Vec<_>>(),
            vec![expected_window.window_id()]
        );
        let _ = runtime
            .commit_surface_shutdown(shutdown_reservation)
            .into_parts();
        assert!(
            runtime
                .settle_surface_window_terminal(lease, expected_window.window_id())
                .changed()
        );
        assert_eq!(
            session.mark_runtime_empty(lease),
            DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked
        );
        assert_eq!(
            session.settle_terminal(
                lease,
                lease.anchor(),
                DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
            ),
            DockSurfaceWindowSessionTerminalOutcome::Settled
        );
        assert_eq!(
            session.complete_shutdown(lease),
            DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
        );
    }

    #[open_gpui::test]
    fn stale_drag_and_prepared_delivery_cannot_commit_across_surface_generations(
        cx: &mut TestAppContext,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let fixture = DockViewportRuntimeFixture::builder(source_space.clone())
            .space(source_space.clone(), ["a"])
            .space(target_space.clone(), ["b"])
            .build_controller(cx);
        let source_tabs = fixture.tabs(&source_space);
        let target_tabs = fixture.tabs(&target_space);
        let controller = fixture.controller.clone();
        let authority = EntityId::from(901);
        let mut runtime =
            DockViewportRuntime::with_surface_authority_close_policy_and_visual_style_resolver(
                controller.clone(),
                authority,
                DockViewportClosePolicy::default(),
                None,
            );
        let mut surface_session = DockSurfaceWindowSession::new(authority);
        let g1 = active_surface_lease(&mut surface_session, WindowId::from(1001));
        assert_eq!(
            runtime.activate_surface_lineage(g1),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );

        let target_window = handle(301);
        assert!(
            runtime
                .register_opened_viewport(target_space.clone(), target_window)
                .is_empty(),
            "the first G1 viewport registration should not replace another window"
        );
        let target_scene =
            DockViewportHostSceneSeed::new(target_space.clone(), target_window, target_tabs);
        target_scene.publish_runtime(&mut runtime);
        let payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            item("a"),
            "Panel A".to_string(),
        );
        let g1_drag = runtime.begin_payload_drag(&payload);
        let request = hovered_window_route_request_for_test(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            target_scene.screen_position(),
            None,
            target_window,
            DockPayloadDropReleaseOrigin::HoveredHost,
        )
        .with_drag_session(Some(g1_drag.clone()));
        let resolution = cx.update(|app| runtime.resolve_payload_drop_delivery(&request, app));
        let delivery = DockDropDelivery::from_resolution(resolution)
            .expect("G1 route facts should mint a delivery");
        let facts = cx.read_entity(&controller, |controller, _| {
            DockViewportWorkspaceRouteFacts::capture_for_payload(
                controller.workspace(),
                delivery.payload(),
                delivery.source_node(),
            )
        });
        let atomic_commit = runtime
            .resolve_locked_workspace_drop_delivery(delivery.clone(), &facts)
            .expect("G1 should resolve an exact locked workspace delivery");
        let atomic_drag_session = atomic_commit
            .drag_session
            .clone()
            .expect("the locked delivery should retain its drag session");
        let atomic_target = atomic_commit.target.clone();
        let atomic_plan = cx
            .read_entity(&controller, |controller, _| {
                controller.workspace().lock_resolved_payload_drop(
                    &atomic_commit.source_space,
                    atomic_commit
                        .payload
                        .as_workspace_payload(atomic_commit.source_node),
                    atomic_target,
                )
            })
            .expect("the exact G1 workspace route should lock");
        let atomic_prepared = runtime
            .prepare_atomic_locked_payload_drop(
                DockViewportLockedWorkspaceDrop::new(atomic_plan, atomic_drag_session),
                target_window,
                None,
            )
            .expect("G1 should prepare its atomic locked drop while active");
        let atomic_sampled =
            cx.update(|app| atomic_prepared.sample_atomic_locked_payload_drop(app));
        let prepared_for_frozen_runtime = runtime
            .prepare_payload_drop(delivery.clone(), None, None, &facts)
            .expect("G1 should prepare while its exact drag lineage is active");
        let prepared_for_g2 = runtime
            .prepare_payload_drop(delivery.clone(), None, None, &facts)
            .expect("G1 should be able to prepare a second delivery before shutdown");

        close_surface_generation(&mut runtime, &mut surface_session, g1, target_window);
        let atomic_preflight_result =
            cx.update(|app| runtime.preflight_atomic_locked_payload_drop(atomic_sampled, app));
        assert!(matches!(
            atomic_preflight_result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == g1_drag.id()
        ));
        let frozen_delivery_result = cx
            .update(|app| runtime.deliver_drop_commit_delivery_with_outcome(delivery.clone(), app));
        assert!(matches!(
            frozen_delivery_result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == g1_drag.id()
        ));
        let frozen_prepared_result = cx.update(|app| prepared_for_frozen_runtime.apply(app));
        assert!(matches!(
            frozen_prepared_result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == g1_drag.id()
        ));
        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")],
                "frozen G1 work must not remove its payload from the source"
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")],
                "frozen G1 work must not write its payload into the graph"
            );
        });

        let g2 = active_surface_lease(&mut surface_session, WindowId::from(1002));
        assert_eq!(
            runtime.activate_surface_lineage(g2),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        let g2_drag = runtime.begin_payload_drag(&payload);
        assert_eq!(
            g1_drag.id(),
            g2_drag.id(),
            "surface freeze resets the per-generation drag counter in this regression"
        );
        assert_ne!(g1_drag.lineage(), g2_drag.lineage());
        assert_ne!(
            g1_drag, g2_drag,
            "equal session ids and payloads must remain distinct across generations"
        );

        let stale_delivery_result =
            cx.update(|app| runtime.deliver_drop_commit_delivery_with_outcome(delivery, app));
        assert!(matches!(
            stale_delivery_result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == g1_drag.id()
        ));
        let stale_prepared_result = cx.update(|app| prepared_for_g2.apply(app));
        assert!(matches!(
            stale_prepared_result,
            Err(DockActionApplyError::DropDragSessionStale { session })
                if session == g1_drag.id()
        ));

        cx.read_entity(&controller, |controller, _| {
            assert_eq!(
                controller.graph().collect_items_in_space(&source_space),
                vec![item("a")],
                "neither stale path may remove the G1 payload from its source"
            );
            assert_eq!(
                controller.graph().collect_items_in_space(&target_space),
                vec![item("b")],
                "neither stale path may write the G1 payload into the G2 graph"
            );
        });
    }
}
