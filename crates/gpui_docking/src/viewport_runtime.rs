#[cfg(test)]
use crate::viewport_registry::DockViewportRouteUnavailableReason;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockDropDelivery, DockItemId,
    DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportActivationTarget,
    DockViewportAdapter, DockViewportCloseCoordinator, DockViewportCloseOutcome,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropActionOutcome,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteOutcome,
    DockViewportDropRouteRequest, DockViewportIdentity, DockViewportPlacementLayout,
    DockViewportPlacementValidationError, DockViewportResolvedDropRoute,
    DockViewportRestoreReadiness, DockViewportRoutedDropPreview,
    DockViewportRoutedDropPreviewStore, DockViewportRuntimeHandle, DockViewportRuntimeStatus,
    DockViewportShouldCloseOutcome, DockViewportTearOffBeginOutcome,
    DockViewportTearOffCancelReason, DockViewportTearOffCancelled,
    DockViewportTearOffCommitFailure, DockViewportTearOffCompleted,
    DockViewportTearOffCompletionOutcome, DockViewportTearOffCompletionPending,
    DockViewportTearOffKey, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockViewportTearOffSourceStatus,
    DockViewportTearOffTick, DockViewportWindowFacts,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    drop_target::{DockDropResolution, DockResolvedDropTarget, validate_resolved_drop_target},
    interaction::DockRuntimeDragSession,
    viewport_close_gate::DockViewportCloseGate,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
    workspace_move_validation::dock_target_validator,
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
use open_gpui::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, Point, WindowBounds, WindowId, WindowOptions,
    point, px, size,
};
use std::collections::HashSet;

const DEFAULT_TEAR_OFF_WINDOW_SIZE: open_gpui::Size<Pixels> = size(px(360.0), px(240.0));
const DEFAULT_TEAR_OFF_CURSOR_OFFSET: Point<Pixels> = point(px(24.0), px(18.0));

/// Internal owner for controller-backed platform viewport lifecycle.
///
/// The runtime keeps the shared [`DockController`] together with the low-level
/// [`DockViewportAdapter`] so the handle does not have to pass the controller into every open call
/// or duplicate close-callback cleanup logic. The adapter remains the place for window mappings,
/// live window facts, and placement import/export.
#[derive(Debug)]
pub(crate) struct DockViewportRuntime {
    controller: Entity<DockController>,
    adapter: DockViewportAdapter,
    close_gate: DockViewportCloseGate,
    host_scenes: DockViewportHostSceneRegistry,
    tear_off: DockViewportTearOffMachine,
    tear_off_tick: DockViewportTearOffTick,
    drag_session: Option<DockRuntimeDragSession>,
    drag_tear_off_geometry: Option<DockRuntimeDragTearOffGeometry>,
    next_drag_session_id: u64,
    owned_windows: HashSet<WindowId>,
    close_coordinator: DockViewportCloseCoordinator,
    routed_drop_preview: DockViewportRoutedDropPreviewStore,
    status: DockViewportRuntimeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockRuntimeDragTearOffGeometry {
    drag_session_id: u64,
    geometry: DockDragTearOffGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffPlacementSource {
    Suggested,
    DragGeometry,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportTearOffPlacement {
    window_bounds: WindowBounds,
    source: DockViewportTearOffPlacementSource,
}

#[derive(Debug, Clone, Copy)]
struct DockViewportTearOffPlacementPolicy {
    minimum_size: open_gpui::Size<Pixels>,
    fallback_size: open_gpui::Size<Pixels>,
    fallback_cursor_offset: Point<Pixels>,
}

enum DockViewportWorkspaceRouteTarget {
    Valid(Option<crate::DockViewportResolvedDropTargetSnapshot>),
    Unavailable,
    Rejected(DockPolicyError),
}

impl DockRuntimeDragTearOffGeometry {
    fn new(drag_session_id: u64, geometry: DockDragTearOffGeometry) -> Self {
        Self {
            drag_session_id,
            geometry,
        }
    }

    fn matches_drag_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.drag_session_id == session.id()
    }
}

impl DockViewportTearOffPlacement {
    fn new(window_bounds: WindowBounds, source: DockViewportTearOffPlacementSource) -> Self {
        Self {
            window_bounds,
            source,
        }
    }

    pub(crate) fn window_bounds(&self) -> WindowBounds {
        self.window_bounds
    }

    #[cfg(test)]
    pub(crate) fn source(&self) -> DockViewportTearOffPlacementSource {
        self.source
    }
}

impl Default for DockViewportTearOffPlacementPolicy {
    fn default() -> Self {
        Self {
            minimum_size: DEFAULT_TEAR_OFF_WINDOW_SIZE,
            fallback_size: DEFAULT_TEAR_OFF_WINDOW_SIZE,
            fallback_cursor_offset: DEFAULT_TEAR_OFF_CURSOR_OFFSET,
        }
    }
}

impl DockViewportTearOffPlacementPolicy {
    fn resolve(&self, request: &DockViewportTearOffRequest) -> DockViewportTearOffPlacement {
        if let Some(window_bounds) = request.suggested_window_bounds() {
            return DockViewportTearOffPlacement::new(
                window_bounds,
                DockViewportTearOffPlacementSource::Suggested,
            );
        }

        if let Some(geometry) = request.tear_off_geometry() {
            return DockViewportTearOffPlacement::new(
                WindowBounds::Windowed(self.bounds_from_drag_geometry(request, geometry)),
                DockViewportTearOffPlacementSource::DragGeometry,
            );
        }

        DockViewportTearOffPlacement::new(
            WindowBounds::Windowed(Bounds::new(
                request.release_position() - self.fallback_cursor_offset,
                self.fallback_size,
            )),
            DockViewportTearOffPlacementSource::Fallback,
        )
    }

    fn bounds_from_drag_geometry(
        &self,
        request: &DockViewportTearOffRequest,
        geometry: DockDragTearOffGeometry,
    ) -> Bounds<Pixels> {
        let mut size = geometry
            .preferred_size()
            .unwrap_or_else(|| geometry.source_bounds().size)
            .max(&self.minimum_size);
        if let Some(work_area) = geometry.display_work_area() {
            let minimum_size = self.minimum_size.min(&work_area.size);
            size = size.max(&minimum_size).min(&work_area.size);
        }

        let cursor_offset = geometry
            .cursor_offset()
            .clamp(&point(px(0.0), px(0.0)), &point(size.width, size.height));
        let mut bounds = Bounds::new(request.release_position() - cursor_offset, size);
        if let Some(work_area) = geometry.display_work_area() {
            bounds = clamp_bounds_to_work_area(bounds, work_area);
        }
        bounds
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedTearOffDrop {
    pub(crate) request: DockViewportTearOffRequest,
    pub(crate) target_space: DockSpaceId,
    pub(crate) options: WindowOptions,
}

#[derive(Debug)]
pub(crate) enum DockViewportTearOffCommitPreparation {
    Prepared(Box<DockViewportPreparedTearOffDrop>),
}

fn resolved_target_snapshot(
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    resolution: DockDropResolution,
) -> Result<crate::DockViewportResolvedDropTargetSnapshot, DockPolicyError> {
    match resolution {
        DockDropResolution::Valid(target) => {
            Ok(crate::DockViewportResolvedDropTargetSnapshot::new(
                target_space,
                target_window_id,
                frame,
                facts_generation,
                target,
            ))
        }
        DockDropResolution::Rejected(rejection) => Err(rejection.reason),
    }
}

impl DockViewportRuntime {
    /// Creates a runtime with the default close policy.
    pub(crate) fn new(controller: Entity<DockController>) -> Self {
        Self::with_close_policy(controller, DockViewportClosePolicy::default())
    }

    /// Creates a runtime with an explicit close policy.
    pub(crate) fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        Self {
            controller,
            adapter: DockViewportAdapter::new(),
            close_gate: DockViewportCloseGate::new(close_policy),
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            drag_session: None,
            drag_tear_off_geometry: None,
            next_drag_session_id: 0,
            owned_windows: HashSet::new(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewStore::default(),
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Creates a runtime from an existing adapter.
    #[cfg(test)]
    pub(crate) fn from_adapter(
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
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            drag_session: None,
            drag_tear_off_geometry: None,
            next_drag_session_id: 0,
            owned_windows: HashSet::new(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewStore::default(),
            status: DockViewportRuntimeStatus::default(),
        }
    }

    /// Wraps this runtime in a cloneable handle for GPUI application callbacks.
    pub(crate) fn into_handle(self) -> DockViewportRuntimeHandle {
        DockViewportRuntimeHandle::from_runtime(self)
    }

    pub(crate) fn controller_entity(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the low-level viewport adapter.
    pub(crate) fn adapter(&self) -> &DockViewportAdapter {
        &self.adapter
    }

    #[cfg(test)]
    pub(crate) fn unregister_adapter_window_for_test(&mut self, window_id: WindowId) {
        let _ = self.adapter.unregister_window_id_snapshot(window_id);
        self.close_gate.sync_adapter(&self.adapter);
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
        self.status
            .clone()
            .with_viewport_lifecycle(self.adapter.viewport_lifecycle_records())
    }

    pub(crate) fn begin_payload_drag(
        &mut self,
        payload: &DockDragPayload,
    ) -> DockRuntimeDragSession {
        let id = self.next_drag_session_id.wrapping_add(1);
        self.next_drag_session_id = id;
        let session = DockRuntimeDragSession::new(id, payload);
        self.drag_session = Some(session.clone());
        self.drag_tear_off_geometry = None;
        self.clear_routed_drop_preview_for_drag_session(Some(&session));
        session
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &mut self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        if self.drag_session.as_ref() != Some(session) {
            return false;
        }
        let next = Some(DockRuntimeDragTearOffGeometry::new(session.id(), geometry));
        if self.drag_tear_off_geometry == next {
            return false;
        }
        self.drag_tear_off_geometry = next;
        true
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        let session = session?;
        self.drag_tear_off_geometry
            .filter(|geometry| geometry.matches_drag_session(session))
            .map(|geometry| geometry.geometry)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.drag_session
            .as_ref()
            .filter(|session| session.accepts_payload(payload))
            .cloned()
    }

    pub(crate) fn finish_payload_drag(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> (bool, Vec<AnyWindowHandle>) {
        if self.drag_session.as_ref() != Some(session) {
            return (false, Vec::new());
        }
        self.drag_session = None;
        let mut changed = true;
        if self
            .drag_tear_off_geometry
            .is_some_and(|geometry| geometry.matches_drag_session(session))
        {
            self.drag_tear_off_geometry = None;
            changed = true;
        }
        let (preview_changed, windows) =
            self.clear_routed_drop_preview_for_drag_session(Some(session));
        (changed || preview_changed, windows)
    }

    pub(crate) fn validate_payload_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        let Some(session) = session else {
            return Ok(());
        };
        if self.drag_session.as_ref() == Some(session) {
            return Ok(());
        }
        Err(DockActionApplyError::DropDragSessionStale {
            session: session.id(),
        })
    }

    pub(crate) fn record_window_focus(&mut self, window_id: WindowId) {
        self.adapter.record_window_focus(window_id);
    }

    fn discard_owned_window(&mut self, window_id: WindowId) -> bool {
        self.owned_windows.remove(&window_id)
    }

    /// Returns the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_gate.close_policy()
    }

    /// Replaces the close policy used by [`handle_window_should_close`](Self::handle_window_should_close).
    pub(crate) fn set_close_policy(&mut self, close_policy: DockViewportClosePolicy) {
        self.close_gate.set_close_policy(close_policy);
    }

    #[cfg(test)]
    pub(crate) fn pending_tear_off_len(&self) -> usize {
        self.tear_off.len()
    }

    /// Updates display, window, and host bounds for a registered viewport.
    ///
    /// Returns true when the stored runtime snapshot changed.
    pub(crate) fn update_viewport_snapshot(
        &mut self,
        space: &DockSpaceId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        self.adapter
            .update_snapshot(space, window_facts, host_bounds)
    }

    pub(crate) fn mark_viewport_window_snapshot_stale(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let changed = self.adapter.mark_window_snapshot_stale(window_id);
        let (preview_changed, windows) =
            self.clear_routed_drop_preview_if_window_matches(window_id);
        (changed || preview_changed, windows)
    }

    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.begin_viewport_host_scene_frame(
            space,
            window_id,
            window_facts,
            host_bounds,
            host_position,
        )
        .is_some_and(|registration| registration.changed)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &mut self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
        host_position: Point<Pixels>,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let window = self.adapter.window_for_space(&space)?;
        let current_identity = DockViewportIdentity::new(space.clone(), window.window_id());
        if !current_identity.matches(&space, window_id) {
            return None;
        }
        let changed = self.update_viewport_snapshot(&space, window_facts, host_bounds);
        let mut registration = self
            .host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                space,
                window_id,
                window_facts.screen_bounds,
                host_bounds,
                host_position,
            ));
        registration.changed |= changed;
        Some(registration)
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.host_scenes.push_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &mut self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.host_scenes.push_frame_fact(frame, fact)
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.routed_drop_preview.preview_for(space, window_id)
    }

    #[cfg(test)]
    pub(crate) fn routed_drop_delivery_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDropDelivery> {
        self.routed_drop_preview.delivery_for_drag_session(session)
    }

    pub(crate) fn update_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload_title: impl Into<String>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.routed_drop_preview
            .update(resolution, payload_title, |space| {
                self.adapter.window_for_space(space)
            })
    }

    pub(crate) fn clear_routed_drop_preview(&mut self) -> (bool, Vec<AnyWindowHandle>) {
        self.routed_drop_preview
            .clear(|space| self.adapter.window_for_space(space))
    }

    fn clear_routed_drop_preview_if_window_matches(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.routed_drop_preview
            .clear_if_window_matches(window_id, |space| self.adapter.window_for_space(space))
    }

    fn clear_routed_drop_preview_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.routed_drop_preview
            .clear_for_drag_session(session, |space| self.adapter.window_for_space(space))
    }

    fn clear_runtime_window_state(&mut self, space: &DockSpaceId, window_id: WindowId) {
        let _ = self.clear_routed_drop_preview_if_window_matches(window_id);
        self.host_scenes.unregister_space(space);
    }

    pub(crate) fn reusable_window_for_space(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindow {
        let Some(window) = self.adapter.window_for_space(space) else {
            return DockViewportReusableWindow::Missing;
        };
        if window
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return DockViewportReusableWindow::Reused(window);
        }

        self.discard_owned_window(window.window_id());
        self.adapter.unregister_space(space);
        self.host_scenes.unregister_space(space);
        self.close_gate.sync_adapter(&self.adapter);
        DockViewportReusableWindow::Stale
    }

    pub(crate) fn register_opened_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        self.owned_windows.insert(window.window_id());
        let replaced = self.adapter.register_viewport_with_outcome(space, window);
        let mut replaced_windows = Vec::new();
        for removed in replaced.replaced() {
            self.clear_runtime_window_state(&removed.space, removed.window.window_id());
            if removed.window != window
                && self.discard_owned_window(removed.window.window_id())
                && !replaced_windows.contains(&removed.window)
            {
                replaced_windows.push(removed.window);
            }
        }
        self.close_gate.sync_adapter(&self.adapter);
        replaced_windows
    }

    pub(crate) fn deliver_payload_drop_with_outcome(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = self.deliver_payload_drop_inner(delivery, cx);
        self.status.record_drop_result(&result);
        result
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

    fn deliver_payload_drop_inner(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let (source_space, source_node, payload, target_space) = match delivery {
            DockDropDelivery::Workspace(delivery) => {
                self.validate_payload_drag_session(delivery.drag_session())?;
                let (source_space, source_node, payload, target) = delivery.into_parts();
                let target_space = match target {
                    crate::DockDropWorkspaceTarget::Resolved(target)
                        if target.frame().is_current_in(&self.host_scenes)
                            && self.target_facts_generation_is_current(
                                target.target_space(),
                                target.target_window_id(),
                                target.facts_generation(),
                            ) =>
                    {
                        let target_space = target.target_space().clone();
                        self.validate_resolved_target_snapshot(
                            &target_space,
                            target.into_target(),
                            &payload,
                            source_node,
                            cx,
                        )?
                    }
                    crate::DockDropWorkspaceTarget::Resolved(_) => {
                        return Err(DockActionApplyError::DropTargetUnavailable);
                    }
                    crate::DockDropWorkspaceTarget::ResolveLocalAtDelivery { host_position } => {
                        self.resolve_local_route_target(
                            &source_space,
                            host_position,
                            &payload,
                            source_node,
                            cx,
                        )?
                    }
                };
                (source_space, source_node, payload, target_space)
            }
            DockDropDelivery::TearOff(request) => {
                self.validate_payload_drag_session(request.drag_session())?;
                return Err(DockActionApplyError::TearOffViewportOpenFailed {
                    message:
                        "tear-off viewport commits must be opened through DockViewportRuntimeHandle"
                            .to_string(),
                });
            }
            DockDropDelivery::Unavailable(delivery) => {
                self.validate_payload_drag_session(delivery.drag_session())?;
                return Err(DockActionApplyError::DropTargetUnavailable);
            }
            DockDropDelivery::Rejected(delivery) => {
                self.validate_payload_drag_session(delivery.drag_session())?;
                return Err(delivery.into_error().into());
            }
        };

        let (target_space, target) = target_space;
        let focus_item = self.focus_item_for_payload(&payload, source_node, cx);
        let action = self.controller.update(cx, |controller, cx| {
            let outcome = controller.workspace_mut().commit_resolved_payload_drop(
                DockWorkspacePayloadDropRequest {
                    source_space: &source_space,
                    payload: payload.as_workspace_payload(source_node),
                    target_space: &target_space,
                    target,
                },
            );
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })?;
        let activation = self.activate_viewport_for_space(&target_space, focus_item, cx);
        Ok(DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(action, activation),
        ))
    }

    fn validate_resolved_target_snapshot(
        &self,
        target_space: &DockSpaceId,
        target: DockResolvedDropTarget,
        payload: &DockViewportDropPayload,
        source_node: DockNodeId,
        cx: &App,
    ) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().clone();
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
        let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
        match validate_resolved_drop_target(target, &policy, Some(&target_validator)) {
            DockDropResolution::Valid(target) => Ok((target_space.clone(), target)),
            DockDropResolution::Rejected(rejection) => {
                Err(DockActionApplyError::Policy(rejection.reason))
            }
        }
    }

    fn resolve_local_route_target(
        &self,
        target_space: &DockSpaceId,
        host_position: Point<Pixels>,
        payload: &DockViewportDropPayload,
        source_node: DockNodeId,
        cx: &App,
    ) -> Result<(DockSpaceId, DockResolvedDropTarget), DockActionApplyError> {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().clone();
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
        let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
        let Some((_, resolution)) = self.host_scenes.resolve_frame_for_window(
            target_space,
            None,
            host_position,
            &policy,
            Some(&target_validator),
        ) else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        match resolution {
            crate::drop_target::DockDropResolution::Valid(target) => {
                Ok((target_space.clone(), target))
            }
            crate::drop_target::DockDropResolution::Rejected(rejection) => {
                Err(DockActionApplyError::Policy(rejection.reason))
            }
        }
    }

    fn target_facts_generation_is_current(
        &self,
        target_space: &DockSpaceId,
        target_window_id: Option<WindowId>,
        target_facts_generation: Option<u64>,
    ) -> bool {
        let (Some(window_id), Some(facts_generation)) = (target_window_id, target_facts_generation)
        else {
            return true;
        };
        self.adapter
            .snapshot_facts_generation(target_space, window_id)
            == Some(facts_generation)
    }

    pub(crate) fn prepare_tear_off_drop_delivery(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportTearOffCommitPreparation, DockActionApplyError> {
        self.validate_payload_drag_session(request.drag_session())?;
        Ok(DockViewportTearOffCommitPreparation::Prepared(Box::new(
            self.prepare_tear_off_drop_route(request, cx)?,
        )))
    }

    pub(crate) fn prepare_tear_off_drop_route(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        crate::validate_tear_off_request(self.controller.read(cx).graph(), &request)?;

        let target_space = self.next_tear_off_space(&request, cx);
        let options = self.tear_off_window_options(&request);
        Ok(DockViewportPreparedTearOffDrop {
            request,
            target_space,
            options,
        })
    }

    pub(crate) fn next_tear_off_space(
        &mut self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> DockSpaceId {
        loop {
            let tick = self.next_tear_off_tick();
            let space = DockSpaceId::new(format!(
                "{}:tear-off:{}:{}",
                request.source_space(),
                request.payload().label(),
                tick.as_u64()
            ));
            let graph_has_space = self
                .controller
                .read(cx)
                .graph()
                .spaces()
                .iter()
                .any(|known| known == &space);
            if !graph_has_space && self.adapter.window_for_space(&space).is_none() {
                return space;
            }
        }
    }

    pub(crate) fn tear_off_window_options(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> WindowOptions {
        let window_bounds = self.tear_off_window_placement(request).window_bounds();

        WindowOptions {
            window_bounds: Some(window_bounds),
            ..Default::default()
        }
    }

    pub(crate) fn tear_off_window_placement(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> DockViewportTearOffPlacement {
        DockViewportTearOffPlacementPolicy::default().resolve(request)
    }

    #[cfg(test)]
    pub(crate) fn last_host_scene_screen_position(
        &self,
        space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        self.host_scenes.screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let policy = self.controller.read(cx).workspace().policy().clone();
        self.host_scenes.resolve(space, host_position, &policy)
    }

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportDropRoute {
        self.resolve_payload_drop_delivery(request, cx)
            .route()
            .clone()
    }

    /// Resolves a rendered payload release into route and delivery facts from one snapshot.
    pub(crate) fn resolve_payload_drop_delivery(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &App,
    ) -> DockViewportResolvedDropRoute {
        let controller = self.controller.read(cx);
        let workspace = controller.workspace();
        let policy = workspace.policy().to_owned();
        let mut route = self.adapter.resolve_payload_drop_route(request, &policy);
        let payload_classes = workspace
            .payload_dock_classes_for_viewport_payload(request.payload(), request.source_node());
        let resolved_target = match self.resolved_workspace_target_for_route(
            &route,
            request,
            &policy,
            &payload_classes,
        ) {
            DockViewportWorkspaceRouteTarget::Valid(target) => target,
            DockViewportWorkspaceRouteTarget::Unavailable => {
                route = DockViewportDropRoute::Unavailable;
                None
            }
            DockViewportWorkspaceRouteTarget::Rejected(error) => {
                if matches!(route, DockViewportDropRoute::KnownViewport { .. }) {
                    route = DockViewportDropRoute::Rejected(error);
                }
                None
            }
        };
        self.status.record_route(request, &route);
        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            request,
            route.clone(),
            resolved_target,
        );
        DockViewportResolvedDropRoute::new(route, delivery)
    }

    fn resolved_workspace_target_for_route(
        &self,
        route: &DockViewportDropRoute,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
        payload_classes: &crate::workspace_move_validation::DockPayloadDockClasses,
    ) -> DockViewportWorkspaceRouteTarget {
        match route {
            DockViewportDropRoute::Local { host_position } => {
                let target_validator =
                    dock_target_validator(request.source_space(), payload_classes, policy);
                let resolved = self
                    .host_scenes
                    .resolve_frame_for_window(
                        request.source_space(),
                        None,
                        *host_position,
                        policy,
                        Some(&target_validator),
                    )
                    .map(|(frame, resolution)| {
                        resolved_target_snapshot(
                            request.source_space().clone(),
                            None,
                            frame,
                            None,
                            resolution,
                        )
                    });
                DockViewportWorkspaceRouteTarget::Valid(resolved.and_then(Result::ok))
            }
            DockViewportDropRoute::KnownViewport { target } => {
                let target_validator =
                    dock_target_validator(target.space(), payload_classes, policy);
                let Some((frame, resolution)) = self.host_scenes.resolve_frame_for_window(
                    target.space(),
                    Some(target.window_id()),
                    target.host_position(),
                    policy,
                    Some(&target_validator),
                ) else {
                    return DockViewportWorkspaceRouteTarget::Unavailable;
                };
                match resolved_target_snapshot(
                    target.space().clone(),
                    Some(target.window_id()),
                    frame,
                    Some(target.facts_generation()),
                    resolution,
                ) {
                    Ok(target) => DockViewportWorkspaceRouteTarget::Valid(Some(target)),
                    Err(error) => DockViewportWorkspaceRouteTarget::Rejected(error),
                }
            }
            DockViewportDropRoute::TearOff(_)
            | DockViewportDropRoute::Unavailable
            | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::Valid(None),
        }
    }

    pub(crate) fn begin_tear_off_request(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        cx: &App,
    ) -> DockViewportTearOffBeginOutcome {
        let focus_item = self.focus_item_for_request(&request, cx);
        let now = self.next_tear_off_tick();
        self.begin_tear_off_request_at(request, target_space, focus_item, now)
    }

    pub(crate) fn begin_tear_off_request_at(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
        now: DockViewportTearOffTick,
    ) -> DockViewportTearOffBeginOutcome {
        self.tear_off
            .begin(request, target_space.into(), focus_item, now)
    }

    pub(crate) fn cancel_tear_off_request(
        &mut self,
        key: &DockViewportTearOffKey,
        reason: DockViewportTearOffCancelReason,
    ) -> Option<DockViewportTearOffCancelled> {
        self.tear_off.cancel(key, reason)
    }

    #[cfg(test)]
    pub(crate) fn expire_tear_off_requests_at(
        &mut self,
        now: DockViewportTearOffTick,
    ) -> Vec<DockViewportTearOffCancelled> {
        self.tear_off.expire(now)
    }

    pub(crate) fn complete_tear_off_viewport(
        &mut self,
        key: &DockViewportTearOffKey,
        window: impl Into<AnyWindowHandle>,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let now = self.next_tear_off_tick();
        self.complete_tear_off_viewport_at(key, window, now, cx)
    }

    pub(crate) fn complete_tear_off_viewport_at(
        &mut self,
        key: &DockViewportTearOffKey,
        window: impl Into<AnyWindowHandle>,
        now: DockViewportTearOffTick,
        cx: &mut App,
    ) -> DockViewportTearOffCompletionOutcome {
        let window = window.into();
        let payload = key.payload();
        let readiness = self.prepare_tear_off_completion(key, now, cx);
        let pending = match readiness {
            DockViewportTearOffCompletionPending::Pending(pending) => pending,
            DockViewportTearOffCompletionPending::Cancelled(cancelled) => {
                return DockViewportTearOffCompletionOutcome::Cancelled(cancelled);
            }
            DockViewportTearOffCompletionPending::Missing => {
                return DockViewportTearOffCompletionOutcome::MissingPending { payload };
            }
        };

        let registration = self
            .adapter
            .register_viewport_with_outcome(pending.target_space().clone(), window);
        self.owned_windows.insert(window.window_id());
        let mut replaced_windows = Vec::new();
        for removed in registration.replaced() {
            self.clear_runtime_window_state(&removed.space, removed.window.window_id());
            if removed.window != window
                && self.discard_owned_window(removed.window.window_id())
                && !replaced_windows.contains(&removed.window)
            {
                replaced_windows.push(removed.window);
            }
        }
        self.close_gate.sync_adapter(&self.adapter);
        match self.commit_tear_off_move(&pending, cx) {
            Ok(action) => {
                let _ = registration
                    .window()
                    .update(cx, |_, window, _| window.activate_window());
                DockViewportTearOffCompletionOutcome::Completed(DockViewportTearOffCompleted::new(
                    pending,
                    registration,
                    replaced_windows,
                    action,
                ))
            }
            Err(error) => {
                self.discard_owned_window(window.window_id());
                self.adapter.unregister_space(pending.target_space());
                self.host_scenes.unregister_space(pending.target_space());
                self.close_gate.sync_adapter(&self.adapter);
                DockViewportTearOffCompletionOutcome::CommitFailed(
                    DockViewportTearOffCommitFailure::new(pending, registration, error),
                )
            }
        }
    }

    pub(crate) fn finish_tear_off_open(
        &mut self,
        pending: DockViewportTearOffPending,
        completion: DockViewportTearOffCompletionOutcome,
        cx: &App,
    ) -> DockViewportTearOffOpenOutcome {
        match completion {
            DockViewportTearOffCompletionOutcome::Completed(completed) => {
                DockViewportTearOffOpenOutcome::Completed(completed)
            }
            DockViewportTearOffCompletionOutcome::Cancelled(cancelled) => {
                self.discard_tear_off_target(pending.target_space());
                DockViewportTearOffOpenOutcome::Cancelled(cancelled)
            }
            DockViewportTearOffCompletionOutcome::MissingPending { .. } => {
                self.discard_tear_off_target(pending.target_space());
                let reason = match self.tear_off_source_status(&pending, cx) {
                    DockViewportTearOffSourceStatus::Ready => {
                        DockViewportTearOffCancelReason::Cancelled
                    }
                    DockViewportTearOffSourceStatus::Missing => {
                        DockViewportTearOffCancelReason::SourceMissing
                    }
                    DockViewportTearOffSourceStatus::Moved => {
                        DockViewportTearOffCancelReason::SourceMoved
                    }
                };
                DockViewportTearOffOpenOutcome::Cancelled(DockViewportTearOffCancelled::new(
                    pending, reason,
                ))
            }
            DockViewportTearOffCompletionOutcome::CommitFailed(failure) => {
                DockViewportTearOffOpenOutcome::CommitFailed(failure)
            }
        }
    }

    fn discard_tear_off_target(&mut self, target_space: &DockSpaceId) {
        if let Some(snapshot) = self.adapter.unregister_space(target_space) {
            self.clear_runtime_window_state(target_space, snapshot.window.window_id());
            self.discard_owned_window(snapshot.window.window_id());
        }
        self.close_gate.sync_adapter(&self.adapter);
    }

    fn activate_viewport_for_space(
        &mut self,
        target_space: &DockSpaceId,
        focus_item: Option<DockItemId>,
        cx: &mut App,
    ) -> Option<DockViewportActivationTarget> {
        match self.reusable_window_for_space(target_space, cx) {
            DockViewportReusableWindow::Reused(window) => Some(DockViewportActivationTarget::new(
                target_space.clone(),
                window,
                focus_item,
            )),
            DockViewportReusableWindow::Missing | DockViewportReusableWindow::Stale => None,
        }
    }

    fn focus_item_for_request(
        &self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> Option<DockItemId> {
        self.focus_item_for_payload(request.payload(), request.source_node(), cx)
    }

    fn focus_item_for_payload(
        &self,
        payload: &DockViewportDropPayload,
        source_node: crate::DockNodeId,
        cx: &App,
    ) -> Option<DockItemId> {
        match payload {
            DockViewportDropPayload::Item(item) => Some(item.clone()),
            DockViewportDropPayload::Tabs => self
                .controller
                .read(cx)
                .graph()
                .selected_item_in_tabs(source_node),
            DockViewportDropPayload::Floating(floating) => self
                .controller
                .read(cx)
                .graph()
                .selected_item_in_subtree(*floating),
        }
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let _ = self.clear_routed_drop_preview_if_window_matches(window_id);
        self.discard_owned_window(window_id);
        let merge_back_prepared = self
            .close_coordinator
            .was_merge_back_precommitted(window_id);
        let outcome = self.adapter.handle_window_closed(window_id);
        self.host_scenes.unregister_window(window_id);
        self.close_gate.sync_adapter(&self.adapter);
        let outcome = if merge_back_prepared && outcome.status() == DockViewportCloseStatus::Closed
        {
            outcome.with_status(DockViewportCloseStatus::MergedBack)
        } else {
            outcome
        };
        self.status.record_close(&outcome);
        outcome
    }

    /// Handles a GPUI window-closed notification with access to graph mutation context.
    pub(crate) fn handle_window_closed_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        let close_policy = self.close_policy();
        let outcome = self.handle_window_closed(window_id);
        let Some(source_space) = outcome.space().cloned() else {
            return outcome;
        };
        let DockViewportClosePolicy::MergeBack { target_space } = close_policy else {
            return outcome;
        };
        if outcome.status() == DockViewportCloseStatus::MergedBack {
            return outcome;
        }

        let outcome = outcome.with_status(crate::merge_space_back(
            &self.controller,
            &source_space,
            &target_space,
            cx,
        ));
        self.status.record_close(&outcome);
        outcome
    }

    pub(crate) fn activation_target_after_close(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> Option<DockViewportActivationTarget> {
        if outcome.status() != DockViewportCloseStatus::MergedBack {
            return None;
        }
        let DockViewportClosePolicy::MergeBack { target_space } = self.close_policy() else {
            return None;
        };
        let focus_item = {
            let controller = self.controller.read(cx);
            let graph = controller.graph();
            graph
                .first_tabs_in_space(&target_space)
                .and_then(|tabs| graph.selected_item_in_tabs(tabs))
        };
        self.activate_viewport_for_space(&target_space, focus_item, cx)
    }

    /// Handles a GPUI window should-close query by applying this runtime's close policy.
    pub(crate) fn handle_window_should_close(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportShouldCloseOutcome {
        let outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        self.status.record_should_close(&outcome);
        outcome
    }

    pub(crate) fn handle_window_should_close_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportShouldCloseOutcome {
        let outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        let outcome = self.close_coordinator.apply_should_close_plan(
            outcome,
            self.close_policy(),
            &self.controller,
            cx,
        );
        self.status.record_should_close(&outcome);
        outcome
    }

    fn next_tear_off_tick(&mut self) -> DockViewportTearOffTick {
        let tick = self.tear_off_tick;
        self.tear_off_tick = self.tear_off_tick.saturating_add(1);
        tick
    }

    fn prepare_tear_off_completion(
        &mut self,
        key: &DockViewportTearOffKey,
        now: DockViewportTearOffTick,
        cx: &App,
    ) -> DockViewportTearOffCompletionPending {
        let Some(pending) = self.tear_off.pending(key).cloned() else {
            return DockViewportTearOffCompletionPending::Missing;
        };
        if pending.is_expired_at(now) {
            return DockViewportTearOffCompletionPending::Cancelled(
                self.tear_off
                    .cancel(key, DockViewportTearOffCancelReason::Expired)
                    .expect("pending payload should still be present"),
            );
        }

        match self.tear_off_source_status(&pending, cx) {
            DockViewportTearOffSourceStatus::Ready => self.tear_off.take_for_completion(key, now),
            DockViewportTearOffSourceStatus::Missing => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(key, DockViewportTearOffCancelReason::SourceMissing)
                        .expect("pending payload should still be present"),
                )
            }
            DockViewportTearOffSourceStatus::Moved => {
                DockViewportTearOffCompletionPending::Cancelled(
                    self.tear_off
                        .cancel(key, DockViewportTearOffCancelReason::SourceMoved)
                        .expect("pending payload should still be present"),
                )
            }
        }
    }

    fn tear_off_source_status(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &App,
    ) -> DockViewportTearOffSourceStatus {
        crate::tear_off_source_status(self.controller.read(cx).graph(), pending)
    }

    fn commit_tear_off_move(
        &self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.controller.update(cx, |controller, cx| {
            let outcome = crate::commit_tear_off_move(controller.workspace_mut(), pending);
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
    pub(crate) fn export_placement(&self) -> DockViewportPlacementLayout {
        self.adapter.export_placement()
    }

    /// Checks saved placement snapshots against registered viewport windows.
    pub(crate) fn check_placement_restore(
        &mut self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        self.adapter.check_placement_restore(placement)
    }
}

fn clamp_bounds_to_work_area(bounds: Bounds<Pixels>, work_area: Bounds<Pixels>) -> Bounds<Pixels> {
    let max_origin = point(
        work_area.right() - bounds.size.width,
        work_area.bottom() - bounds.size.height,
    );
    let origin = bounds.origin.clamp(&work_area.origin, &max_origin);
    Bounds::new(origin, bounds.size)
}

pub(crate) enum DockViewportReusableWindow {
    Missing,
    Reused(AnyWindowHandle),
    Stale,
}
