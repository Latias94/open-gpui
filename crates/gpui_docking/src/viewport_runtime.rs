#[cfg(test)]
use crate::viewport_registry::DockViewportRouteUnavailableReason;
use crate::{
    DockActionApplyError, DockActionOutcome, DockController, DockDropDelivery, DockItemId,
    DockSpaceId, DockViewportActivationTransaction, DockViewportAdapter,
    DockViewportAuthorizedRouteAuthority, DockViewportBackendFocusState,
    DockViewportCloseCoordinator, DockViewportCloseOutcome, DockViewportClosePlanState,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportDropActionOutcome,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportDropRouteRequest,
    DockViewportDropRouteResolution, DockViewportFocusCoordinator, DockViewportFocusRequest,
    DockViewportIdentity, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportPlatformSyncRecord, DockViewportRegisterOutcome, DockViewportResolvedDropRoute,
    DockViewportRestoreReadiness, DockViewportRoutedDropPreview,
    DockViewportRoutedDropPreviewReplacement, DockViewportRoutedDropPreviewState,
    DockViewportRuntimeHandle, DockViewportRuntimeStatus, DockViewportShouldCloseOutcome,
    DockViewportShouldCloseStatus, DockViewportTargetHit, DockViewportTearOffBeginOutcome,
    DockViewportTearOffCancelReason, DockViewportTearOffCancelled, DockViewportTearOffCompleted,
    DockViewportTearOffKey, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockViewportTearOffSourceStatus,
    DockViewportTearOffTick, DockViewportWindowFacts, DockViewportWindowOwnership,
    DockViewportWorkspaceRouteTarget,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::DockRuntimeDragSession,
    viewport_drop_scene::{
        DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
        DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot,
    },
    viewport_registry::DockViewportPlatformRequests,
    workspace_transaction::DockWorkspacePayloadDropRequest,
};
#[cfg(test)]
use open_gpui::AppContext as _;
use open_gpui::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, PlatformFocusedWindow, Point, WindowBounds,
    WindowId, WindowOptions, point, px,
};

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
    close_policy: DockViewportClosePolicy,
    host_scenes: DockViewportHostSceneRegistry,
    tear_off: DockViewportTearOffMachine,
    tear_off_tick: DockViewportTearOffTick,
    active_drag: Option<DockViewportActivePayloadDrag>,
    drag_tear_off_geometry: Option<DockRuntimeDragTearOffGeometry>,
    next_drag_session_id: u64,
    window_ownership: DockViewportWindowOwnership,
    focus: DockViewportFocusCoordinator,
    backend_focus: DockViewportBackendFocusState,
    close_coordinator: DockViewportCloseCoordinator,
    routed_drop_preview: DockViewportRoutedDropPreviewState,
    status: DockViewportRuntimeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockRuntimeDragTearOffGeometry {
    drag_session_id: u64,
    geometry: DockDragTearOffGeometry,
}

#[derive(Debug)]
struct DockViewportRuntimeRegistration {
    outcome: DockViewportRegisterOutcome,
    replaced_windows: Vec<AnyWindowHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportPointerInputSyncRequest {
    window: AnyWindowHandle,
    /// Desired live platform state. Route facts only change after a later window-facts refresh
    /// observes whether the backend actually applied this request.
    accepts_pointer_input: bool,
}

impl DockViewportPointerInputSyncRequest {
    fn new(window: AnyWindowHandle, accepts_pointer_input: bool) -> Self {
        Self {
            window,
            accepts_pointer_input,
        }
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn requested_accepts_pointer_input(&self) -> bool {
        self.accepts_pointer_input
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockViewportActivePayloadDrag {
    session: DockRuntimeDragSession,
    source_window: Option<AnyWindowHandle>,
    source_window_accepts_pointer_input: Option<bool>,
    /// Most recent viewport route target observed during this drag.
    ///
    /// This is preview bookkeeping, not hover authority. Releases still re-resolve current backend
    /// facts unless they can replay an accepted routed preview.
    last_routed_viewport_identity: Option<DockViewportIdentity>,
}

impl DockViewportActivePayloadDrag {
    fn new(
        session: DockRuntimeDragSession,
        source_window: Option<AnyWindowHandle>,
        source_window_accepts_pointer_input: Option<bool>,
    ) -> Self {
        Self {
            session,
            source_window,
            source_window_accepts_pointer_input,
            last_routed_viewport_identity: None,
        }
    }

    fn session(&self) -> &DockRuntimeDragSession {
        &self.session
    }

    fn source_space(&self) -> &DockSpaceId {
        self.session.source_space()
    }

    fn source_window(&self) -> Option<AnyWindowHandle> {
        self.source_window
    }

    fn source_window_accepts_pointer_input(&self) -> Option<bool> {
        self.source_window_accepts_pointer_input
    }

    fn matches_session(&self, session: &DockRuntimeDragSession) -> bool {
        self.session == *session
    }

    fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.session.accepts_payload(payload)
    }

    fn record_last_routed_viewport_identity(&mut self, identity: Option<DockViewportIdentity>) {
        self.last_routed_viewport_identity = identity;
    }

    #[cfg(test)]
    fn last_routed_viewport_identity(&self) -> Option<&DockViewportIdentity> {
        self.last_routed_viewport_identity.as_ref()
    }

    fn clear_last_routed_viewport_identity_if_window_matches(&mut self, window_id: WindowId) {
        if self
            .last_routed_viewport_identity
            .as_ref()
            .is_some_and(|identity| identity.window_id() == window_id)
        {
            self.last_routed_viewport_identity = None;
        }
    }

    fn clear_last_routed_viewport_identity_for_session(
        &mut self,
        session: &DockRuntimeDragSession,
    ) {
        if self.matches_session(session) {
            self.last_routed_viewport_identity = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffPlacementSource {
    Suggested,
    DragGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportTearOffPlacement {
    window_bounds: WindowBounds,
    source: DockViewportTearOffPlacementSource,
}

#[derive(Debug, Clone, Copy)]
struct DockViewportTearOffPlacementPolicy {}

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
        Self {}
    }
}

impl DockViewportTearOffPlacementPolicy {
    fn resolve(
        &self,
        request: &DockViewportTearOffRequest,
    ) -> Option<DockViewportTearOffPlacement> {
        if let Some(window_bounds) = request.suggested_window_bounds() {
            return Some(DockViewportTearOffPlacement::new(
                window_bounds,
                DockViewportTearOffPlacementSource::Suggested,
            ));
        }

        if let Some(geometry) = request.tear_off_geometry() {
            if let Some(release_position) = request.release_position() {
                return Some(DockViewportTearOffPlacement::new(
                    WindowBounds::Windowed(
                        self.bounds_from_drag_geometry(release_position, geometry),
                    ),
                    DockViewportTearOffPlacementSource::DragGeometry,
                ));
            }
        }
        None
    }

    fn bounds_from_drag_geometry(
        &self,
        release_position: Point<Pixels>,
        geometry: DockDragTearOffGeometry,
    ) -> Bounds<Pixels> {
        let mut size = geometry
            .preferred_size()
            .unwrap_or_else(|| geometry.source_bounds().size);
        if let Some(work_area) = geometry.display_work_area() {
            size = size.min(&work_area.size);
        }

        let cursor_offset = geometry
            .cursor_offset()
            .clamp(&point(px(0.0), px(0.0)), &point(size.width, size.height));
        let mut bounds = Bounds::new(release_position - cursor_offset, size);
        if let Some(work_area) = geometry.display_work_area() {
            bounds = clamp_bounds_to_work_area(bounds, work_area);
        }
        bounds
    }
}

pub(crate) fn suggested_tear_off_window_bounds(
    source_window_bounds: WindowBounds,
    host_position: Point<Pixels>,
    geometry: DockDragTearOffGeometry,
) -> WindowBounds {
    let mut size = geometry
        .preferred_size()
        .unwrap_or_else(|| geometry.source_bounds().size);
    if let Some(work_area) = geometry.display_work_area() {
        size = size.min(&work_area.size);
    }

    let cursor_offset = geometry
        .cursor_offset()
        .clamp(&point(px(0.0), px(0.0)), &point(size.width, size.height));
    let source_window_origin = source_window_bounds.get_bounds().origin;
    let mut bounds = Bounds::new(source_window_origin + host_position - cursor_offset, size);
    if let Some(work_area) = geometry.display_work_area() {
        bounds = clamp_bounds_to_work_area(bounds, work_area);
    }
    WindowBounds::Windowed(bounds)
}

#[derive(Debug)]
pub(crate) struct DockViewportPreparedTearOffDrop {
    pub(crate) request: DockViewportTearOffRequest,
    pub(crate) target_space: DockSpaceId,
    pub(crate) focus_item: Option<DockItemId>,
    pub(crate) options: WindowOptions,
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
            close_policy,
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            active_drag: None,
            drag_tear_off_geometry: None,
            next_drag_session_id: 0,
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
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
        Self {
            controller,
            adapter,
            close_policy,
            host_scenes: DockViewportHostSceneRegistry::default(),
            tear_off: DockViewportTearOffMachine::default(),
            tear_off_tick: DockViewportTearOffTick::default(),
            active_drag: None,
            drag_tear_off_geometry: None,
            next_drag_session_id: 0,
            window_ownership: DockViewportWindowOwnership::default(),
            focus: DockViewportFocusCoordinator::default(),
            backend_focus: DockViewportBackendFocusState::default(),
            close_coordinator: DockViewportCloseCoordinator::default(),
            routed_drop_preview: DockViewportRoutedDropPreviewState::default(),
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
        status.with_viewport_lifecycle(self.adapter.viewport_lifecycle_records())
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
        self.begin_payload_drag_with_pointer_sync_and_focus(payload, None)
            .0
    }

    pub(crate) fn begin_payload_drag_with_pointer_sync_and_focus(
        &mut self,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> (
        DockRuntimeDragSession,
        Option<DockViewportPointerInputSyncRequest>,
    ) {
        let id = self.next_drag_session_id.wrapping_add(1);
        self.next_drag_session_id = id;
        let session = DockRuntimeDragSession::with_focus_item(id, payload, focus_item);
        let source_window = self
            .adapter
            .window_for_space(payload.identity().source_space());
        let source_window_accepts_pointer_input =
            source_window.and_then(|_| self.source_window_accepts_pointer_input(payload));
        self.active_drag = Some(DockViewportActivePayloadDrag::new(
            session.clone(),
            source_window,
            source_window_accepts_pointer_input,
        ));
        self.drag_tear_off_geometry = None;
        self.clear_routed_drop_preview();
        let pointer_sync = match (source_window, source_window_accepts_pointer_input) {
            (Some(window), Some(true)) => {
                Some(DockViewportPointerInputSyncRequest::new(window, false))
            }
            _ => None,
        };
        (session, pointer_sync)
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &mut self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        if !self
            .active_drag
            .as_ref()
            .is_some_and(|drag| drag.matches_session(session))
        {
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
        self.active_drag
            .as_ref()
            .filter(|drag| drag.accepts_payload(payload))
            .map(|drag| drag.session().clone())
    }

    fn source_window_accepts_pointer_input(&self, payload: &DockDragPayload) -> Option<bool> {
        let Some(snapshot) = self.adapter.snapshot(payload.identity().source_space()) else {
            return Some(true);
        };
        match snapshot.pointer_routing {
            crate::viewport_registry::DockViewportPointerRouting::Routable => Some(true),
            crate::viewport_registry::DockViewportPointerRouting::NoInputPassThrough => Some(false),
            crate::viewport_registry::DockViewportPointerRouting::Minimized => Some(true),
        }
    }

    #[cfg(test)]
    pub(crate) fn finish_payload_drag(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let (changed, windows, _) = self.finish_payload_drag_with_pointer_sync(session);
        (changed, windows)
    }

    pub(crate) fn finish_payload_drag_with_pointer_sync(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> (
        bool,
        Vec<AnyWindowHandle>,
        Option<DockViewportPointerInputSyncRequest>,
    ) {
        if !self
            .active_drag
            .as_ref()
            .is_some_and(|drag| drag.matches_session(session))
        {
            return (false, Vec::new(), None);
        }
        let active_drag = self
            .active_drag
            .take()
            .expect("active drag should match the requested session");
        let pointer_sync = match (
            active_drag.source_window(),
            active_drag.source_window_accepts_pointer_input(),
        ) {
            (Some(window), Some(accepts)) => {
                Some(DockViewportPointerInputSyncRequest::new(window, accepts))
            }
            _ => None,
        };
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
        (changed || preview_changed, windows, pointer_sync)
    }

    pub(crate) fn validate_payload_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Result<(), DockActionApplyError> {
        let Some(session) = session else {
            return Err(DockActionApplyError::DropDragSessionMissing);
        };
        if self
            .active_drag
            .as_ref()
            .is_some_and(|drag| drag.matches_session(session))
        {
            return Ok(());
        }
        Err(DockActionApplyError::DropDragSessionStale {
            session: session.id(),
        })
    }

    fn record_confirmed_backend_focused_window(&mut self, window_id: WindowId) -> Option<bool> {
        let adapter = &self.adapter;
        self.backend_focus
            .record_confirmed_backend_focused_window(window_id, |candidate| {
                adapter.space_for_window_id(candidate).is_some()
                    && !adapter.window_close_requested(candidate)
            })
            .map(|focus_record| focus_record.changed())
    }

    pub(crate) fn reconcile_backend_window_focus(&mut self, cx: &mut App) -> bool {
        match cx.focused_window() {
            PlatformFocusedWindow::Window(window) => self
                .record_confirmed_backend_focused_window(window.window_id())
                .unwrap_or(false),
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => false,
        }
    }

    pub(crate) fn record_pending_activation(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> bool {
        self.backend_focus.record_pending_activation(activation)
    }

    pub(crate) fn clear_pending_activation_for(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.backend_focus
            .clear_pending_activation_for(space, window_id)
    }

    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        mouse_down: bool,
        cx: &mut App,
    ) -> Option<crate::DockViewportFocusCommand> {
        let backend_focused = match cx.focused_window() {
            PlatformFocusedWindow::Window(window) => window.window_id() == window_id,
            PlatformFocusedWindow::NoWindow => false,
            PlatformFocusedWindow::Unavailable => return None,
        };
        if !backend_focused || !self.adapter.is_live_window_for_space(space, window_id) {
            return None;
        }

        self.record_confirmed_backend_focused_window(window_id)
            .expect("backend focus was already validated as a live docking window");
        self.backend_focus
            .focus_command_for_confirmed_backend_window_focus(
                &self.focus,
                space,
                window_id,
                mouse_down,
            )
    }

    pub(crate) fn record_panel_focus(&mut self, space: DockSpaceId, item: DockItemId) {
        self.focus.record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&mut self, space: &DockSpaceId) {
        self.focus.record_no_panel_focus(space);
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus_for_test(&self, space: &DockSpaceId) -> Option<bool> {
        self.focus.had_panel_focus(space)
    }

    fn discard_owned_window(&mut self, window_id: WindowId) -> bool {
        self.window_ownership.discard_owned_window(window_id)
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

    pub(crate) fn platform_requests_for_space(
        &self,
        space: &DockSpaceId,
    ) -> DockViewportPlatformRequests {
        self.adapter.platform_requests_for_space(space)
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

    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let changed = self
            .adapter
            .apply_platform_window_facts(window_id, window_facts);
        let clear_preview = self.adapter.window_route_ready(window_id) == Some(false);
        let (preview_changed, windows) = if clear_preview {
            self.clear_routed_drop_preview_if_window_matches(window_id)
        } else {
            (false, Vec::new())
        };
        (changed || preview_changed, windows)
    }

    fn mark_viewport_window_close_requested(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let changed = self.adapter.mark_window_close_requested(window_id);
        let mut changed = changed;
        let mut windows = Vec::new();
        if let Some(space) = self.adapter.space_for_window_id(window_id).cloned() {
            self.status.clear_window_references(&space, window_id);
            let (drag_changed, drag_windows) = self.finish_payload_drag_for_source_space(&space);
            changed |= drag_changed;
            extend_unique_windows(&mut windows, drag_windows);
        }
        self.host_scenes.unregister_window(window_id);
        let (preview_changed, preview_windows) =
            self.clear_routed_drop_preview_if_window_matches(window_id);
        extend_unique_windows(&mut windows, preview_windows);
        (changed || preview_changed, windows)
    }

    pub(crate) fn cancel_window_close_request(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.close_coordinator.cancel_window(window_id);
        let changed = self.adapter.cancel_window_close_requested(window_id);
        if !changed {
            return (false, Vec::new());
        }
        let windows = self
            .adapter
            .space_for_window_id(window_id)
            .and_then(|space| self.adapter.window_for_space(space))
            .into_iter()
            .collect();
        (true, windows)
    }

    pub(crate) fn reconcile_viewport_frame<C: open_gpui::AppContext>(
        &mut self,
        cx: &mut C,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.reconcile_viewport_frame_except_window(None, cx)
    }

    pub(crate) fn reconcile_viewport_frame_except_window<C: open_gpui::AppContext>(
        &mut self,
        skip_window_id: Option<WindowId>,
        cx: &mut C,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let changed_windows = self
            .adapter
            .refresh_registered_window_facts_except_window(cx, skip_window_id);
        let mut changed = !changed_windows.is_empty();
        let mut windows = Vec::new();
        for window in changed_windows {
            extend_unique_windows(&mut windows, [window]);
            if self.adapter.window_route_ready(window.window_id()) == Some(false) {
                let (preview_changed, preview_windows) =
                    self.clear_routed_drop_preview_if_window_matches(window.window_id());
                changed |= preview_changed;
                extend_unique_windows(&mut windows, preview_windows);
            }
        }
        (changed, windows)
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
            crate::DockDropGuideStyle::default(),
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
        drop_guide_style: crate::DockDropGuideStyle,
    ) -> Option<DockViewportHostSceneRegistration> {
        let space = space.into();
        let window = self.adapter.window_for_space(&space)?;
        let current_identity = DockViewportIdentity::new(space.clone(), window.window_id());
        if !current_identity.matches(&space, window_id) {
            return None;
        }
        let close_cancelled = if self.adapter.window_close_requested(window_id) {
            self.cancel_window_close_request(window_id).0
        } else {
            false
        };
        let changed = self.update_viewport_snapshot(&space, window_facts, host_bounds);
        let mut registration = self
            .host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                space,
                window_id,
                window_facts.current_bounds,
                host_bounds,
                host_position,
                drop_guide_style,
            ));
        registration.changed |= changed || close_cancelled;
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
    pub(crate) fn has_routed_drop_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        self.routed_drop_preview
            .has_preview_for_drag_session(session)
    }

    #[cfg(test)]
    pub(crate) fn routed_drop_preview_is_accepted(&self) -> bool {
        self.routed_drop_preview.is_currently_accepted()
    }

    pub(crate) fn update_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload_title: impl Into<String>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let payload_title = payload_title.into();
        let active_drag_session_id = self.active_drag.as_ref().map(|drag| drag.session().id());
        if let Some(active_drag) = self.active_drag.as_mut()
            && let Some(identity) = crate::last_routed_viewport_identity_from_resolution(
                resolution,
                Some(active_drag.session()),
            )
        {
            active_drag.record_last_routed_viewport_identity(Some(identity));
        }
        let next = match resolution.route() {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                resolution
                    .routed_preview_target_snapshot()
                    .and_then(|target| {
                        crate::routed_drop_preview_from_target(
                            target,
                            active_drag_session_id,
                            payload_title,
                        )
                    })
            }
            DockViewportDropRoute::Rejected(_) => resolution
                .routed_preview_target_snapshot()
                .and_then(|target| {
                    crate::routed_rejected_drop_preview_from_target(
                        target,
                        active_drag_session_id,
                        payload_title,
                    )
                }),
            DockViewportDropRoute::TearOff => None,
            DockViewportDropRoute::Unavailable => None,
        };
        let next_resolution = match resolution.route() {
            DockViewportDropRoute::Unavailable => None,
            _ => Some(resolution.clone()),
        };
        let starts_acceptance_pass = matches!(
            resolution.route(),
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. }
        ) && next.is_some();
        if starts_acceptance_pass {
            self.routed_drop_preview.start_acceptance_pass();
        }
        let mut target_window = None;
        if starts_acceptance_pass && let Some(preview) = next.as_ref() {
            target_window = self.adapter.window_for_space(preview.space());
        }
        let (changed, mut windows) = self.replace_routed_drop_preview(next, next_resolution);
        if starts_acceptance_pass {
            crate::push_unique_window(&mut windows, target_window);
        }
        (changed, windows)
    }

    pub(crate) fn finish_routed_drop_acceptance_pass(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.routed_drop_preview
            .finish_acceptance_pass(space, window_id)
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        let resolution = self.resolve_payload_drop_delivery(request, cx);
        if crate::delivery_authority_for_route(resolution.route()).is_some() {
            return resolution;
        }
        resolution.without_delivery()
    }

    fn replace_routed_drop_preview(
        &mut self,
        next: Option<DockViewportRoutedDropPreview>,
        next_resolution: Option<DockViewportResolvedDropRoute>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let replacement = self.routed_drop_preview.replace(next, next_resolution);
        let windows = self.windows_for_routed_preview_replacement(&replacement);
        (replacement.has_changed(), windows)
    }

    fn windows_for_routed_preview_replacement(
        &self,
        replacement: &DockViewportRoutedDropPreviewReplacement,
    ) -> Vec<AnyWindowHandle> {
        let mut windows = Vec::new();
        for space in replacement.affected_spaces() {
            crate::push_unique_window(&mut windows, self.adapter.window_for_space(space));
        }
        windows
    }

    pub(crate) fn clear_routed_drop_preview(&mut self) -> (bool, Vec<AnyWindowHandle>) {
        self.replace_routed_drop_preview(None, None)
    }

    fn clear_routed_drop_preview_if_window_matches(
        &mut self,
        window_id: WindowId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        if let Some(active_drag) = self.active_drag.as_mut() {
            active_drag.clear_last_routed_viewport_identity_if_window_matches(window_id);
        }
        if self.routed_drop_preview.targets_window(window_id) {
            self.replace_routed_drop_preview(None, None)
        } else {
            (false, Vec::new())
        }
    }

    fn clear_routed_drop_preview_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let Some(session) = session else {
            return (false, Vec::new());
        };
        if let Some(active_drag) = self.active_drag.as_mut() {
            active_drag.clear_last_routed_viewport_identity_for_session(session);
        }
        let replacement = self
            .routed_drop_preview
            .clear_for_drag_session(Some(session));
        let windows = self.windows_for_routed_preview_replacement(&replacement);
        (replacement.has_changed(), windows)
    }

    fn clear_runtime_window_state(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
        discard_close_plan: bool,
    ) -> Vec<AnyWindowHandle> {
        let (_, mut windows) = self.clear_routed_drop_preview_if_window_matches(window_id);
        if discard_close_plan {
            self.close_coordinator.discard_window(window_id);
        }
        self.window_ownership.clear_window_state(window_id);
        self.host_scenes.unregister_space(space);
        self.clear_pending_activation_for(space, window_id);
        self.status.clear_window_references(space, window_id);
        self.focus.remove_space(space);
        let (_, drag_windows) = self.finish_payload_drag_for_source_space(space);
        extend_unique_windows(&mut windows, drag_windows);
        windows
    }

    fn finish_payload_drag_for_source_space(
        &mut self,
        space: &DockSpaceId,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let (changed, windows, _) =
            self.finish_payload_drag_for_source_space_with_pointer_sync(space);
        (changed, windows)
    }

    fn finish_payload_drag_for_source_space_with_pointer_sync(
        &mut self,
        space: &DockSpaceId,
    ) -> (
        bool,
        Vec<AnyWindowHandle>,
        Option<DockViewportPointerInputSyncRequest>,
    ) {
        let Some(session) = self
            .active_drag
            .as_ref()
            .filter(|drag| drag.source_space() == space)
            .map(|drag| drag.session().clone())
        else {
            return (false, Vec::new(), None);
        };
        self.finish_payload_drag_with_pointer_sync(&session)
    }

    fn unregister_space_runtime_state(&mut self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        let snapshot = self.adapter.unregister_space(space)?;
        let window = snapshot.window;
        let _ = self.clear_runtime_window_state(space, window.window_id(), true);
        Some(window)
    }

    #[cfg(test)]
    pub(crate) fn unregister_host_for_space(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.unregister_host_for_space_with_pointer_sync(space, window_id)
            .0
    }

    pub(crate) fn unregister_host_for_space_with_pointer_sync(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> (bool, Option<DockViewportPointerInputSyncRequest>) {
        if self
            .adapter
            .window_for_space(space)
            .is_none_or(|window| window.window_id() != window_id)
        {
            return (false, None);
        }
        let (_, _, pointer_sync) =
            self.finish_payload_drag_for_source_space_with_pointer_sync(space);
        if let Some(window) = self.unregister_space_runtime_state(space) {
            self.discard_owned_window(window.window_id());
            (true, pointer_sync)
        } else {
            (false, pointer_sync)
        }
    }

    pub(crate) fn reusable_window_for_space(
        &mut self,
        space: &DockSpaceId,
        cx: &mut App,
    ) -> DockViewportReusableWindow {
        let Some(window) = self.adapter.window_for_space(space) else {
            return DockViewportReusableWindow::Missing;
        };
        if self.adapter.window_close_requested(window.window_id()) {
            return DockViewportReusableWindow::Stale;
        }
        if window.update(cx, |_, _, _| ()).is_ok() {
            return DockViewportReusableWindow::Reused(window);
        }

        if let Some(window) = self.unregister_space_runtime_state(space) {
            self.discard_owned_window(window.window_id());
        }
        DockViewportReusableWindow::Stale
    }

    pub(crate) fn register_opened_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        self.register_runtime_viewport(space, window)
            .replaced_windows
    }

    fn register_runtime_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> DockViewportRuntimeRegistration {
        self.window_ownership
            .register_runtime_window(window.window_id());
        let outcome = self.adapter.register_viewport_with_outcome(space, window);
        let replaced_windows = self.clear_replaced_viewport_mappings(&outcome, window);
        DockViewportRuntimeRegistration {
            outcome,
            replaced_windows,
        }
    }

    fn clear_replaced_viewport_mappings(
        &mut self,
        outcome: &DockViewportRegisterOutcome,
        registered_window: AnyWindowHandle,
    ) -> Vec<AnyWindowHandle> {
        let mut replaced_windows = Vec::new();
        for removed in outcome.replaced() {
            self.clear_runtime_window_state(&removed.space, removed.window.window_id(), true);
            if removed.window != registered_window
                && self.discard_owned_window(removed.window.window_id())
                && !replaced_windows.contains(&removed.window)
            {
                replaced_windows.push(removed.window);
            }
        }
        replaced_windows
    }

    pub(crate) fn register_rendered_host_viewport(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> bool {
        if self.window_ownership.is_retired(window.window_id()) {
            return false;
        }
        match self.adapter.window_for_space(&space) {
            Some(existing) if existing == window => false,
            Some(_) => false,
            None => {
                let outcome = self.adapter.register_viewport_with_outcome(space, window);
                let _ = self.clear_replaced_viewport_mappings(&outcome, window);
                true
            }
        }
    }

    pub(crate) fn deliver_drop_commit_delivery_with_outcome(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let result = self.deliver_payload_drop_inner(delivery, cx);
        self.status.record_drop_result(&result);
        result
    }

    #[cfg(test)]
    pub(crate) fn validate_payload_drop_delivery(
        &self,
        delivery: &DockDropDelivery,
        cx: &App,
    ) -> Result<(), DockActionApplyError> {
        let (source, kind) = delivery.parts();
        self.validate_payload_drag_session(source.drag_session())?;
        match kind {
            crate::DockDropDeliveryKind::Workspace(delivery) => {
                let controller = self.controller.read(cx);
                crate::validate_delivery_workspace_target(
                    &self.adapter,
                    &self.host_scenes,
                    controller.workspace(),
                    source.source_node(),
                    source.payload(),
                    delivery,
                )
            }
            crate::DockDropDeliveryKind::TearOff(_) => Ok(()),
        }
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

    pub(crate) fn record_platform_sync(&mut self, record: DockViewportPlatformSyncRecord) {
        self.status.record_platform_sync(record);
    }

    fn deliver_payload_drop_inner(
        &mut self,
        delivery: DockDropDelivery,
        cx: &mut App,
    ) -> Result<DockViewportDropRouteOutcome, DockActionApplyError> {
        let (source, kind) = delivery.into_parts();
        self.validate_payload_drag_session(source.drag_session())?;
        let (source_space, source_node, payload, target, drag_session) = match kind {
            crate::DockDropDeliveryKind::Workspace(delivery) => {
                let (source_space, source_node, payload, drag_session) = source.into_parts();
                let target_space = {
                    let controller = self.controller.read(cx);
                    crate::resolve_delivery_workspace_target(
                        &self.adapter,
                        &self.host_scenes,
                        controller.workspace(),
                        source_node,
                        &payload,
                        delivery,
                    )?
                };
                (
                    source_space,
                    source_node,
                    payload,
                    target_space,
                    drag_session,
                )
            }
            crate::DockDropDeliveryKind::TearOff(_) => {
                return Err(DockActionApplyError::TearOffViewportOpenFailed {
                    message:
                        "tear-off viewport commits must be opened through DockViewportRuntimeHandle"
                            .to_string(),
                });
            }
        };

        let target_space = target.target_space().clone();
        let frozen_focus_item = drag_session
            .as_ref()
            .and_then(|session| session.focus_item())
            .cloned();
        let drop_outcome = self.controller.update(cx, |controller, cx| {
            let outcome = controller.workspace_mut().commit_resolved_payload_drop(
                DockWorkspacePayloadDropRequest {
                    source_space: &source_space,
                    payload: payload.as_workspace_payload(source_node),
                    target,
                    frozen_focus_item: frozen_focus_item.as_ref(),
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
        let focus_item = drag_session
            .as_ref()
            .and_then(|session| session.focus_item())
            .and_then(|focus_item| {
                self.controller
                    .read(cx)
                    .graph()
                    .find_item_in_space(&target_space, focus_item)?;
                Some(focus_item.clone())
            })
            .or_else(|| drop_outcome.focus_item().cloned());
        let focus_request = focus_item
            .map(DockViewportFocusRequest::panel)
            .unwrap_or_else(DockViewportFocusRequest::no_panel_focus);
        let activation = match self.reusable_window_for_space(&target_space, cx) {
            DockViewportReusableWindow::Reused(window) => Some(
                DockViewportActivationTransaction::new(target_space.clone(), window, focus_request),
            ),
            DockViewportReusableWindow::Missing | DockViewportReusableWindow::Stale => None,
        };
        Ok(DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(drop_outcome.action(), activation),
        ))
    }

    pub(crate) fn prepare_tear_off_drop_delivery(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &mut App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        self.validate_payload_drag_session(request.drag_session())?;
        self.prepare_tear_off_drop_route(request, cx)
    }

    pub(crate) fn prepare_tear_off_drop_route(
        &mut self,
        request: DockViewportTearOffRequest,
        cx: &App,
    ) -> Result<DockViewportPreparedTearOffDrop, DockActionApplyError> {
        crate::validate_tear_off_request(self.controller.read(cx).graph(), &request)?;

        let options = self.tear_off_window_options(&request)?;
        let target_space = self.next_tear_off_space(&request, cx);
        {
            let controller = self.controller.read(cx);
            crate::preflight_tear_off_move(controller.workspace(), &request, &target_space)?;
        }
        let focus_item = self.focus_item_for_request(&request, cx);
        Ok(DockViewportPreparedTearOffDrop {
            request,
            target_space,
            focus_item,
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
    ) -> Result<WindowOptions, DockActionApplyError> {
        let window_bounds = self
            .tear_off_window_placement(request)
            .ok_or(DockActionApplyError::TearOffViewportPlacementUnavailable)?
            .window_bounds();

        Ok(WindowOptions {
            window_bounds: Some(window_bounds),
            // Tear-off viewports are activated after graph commit and runtime registration, so
            // panel focus restoration flows through the explicit activation transaction.
            focus: false,
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
        self.host_scenes.screen_position(space)
    }

    #[cfg(test)]
    pub(crate) fn resolve_host_scene_target(
        &self,
        space: &DockSpaceId,
        host_position: Point<Pixels>,
        cx: &App,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let window = self.adapter.window_for_space(space)?;
        if self
            .adapter
            .snapshot_facts_generation(space, window.window_id())
            .is_none()
        {
            return None;
        }
        let policy = self.controller.read(cx).workspace().policy().clone();
        self.host_scenes.resolve_for_window(
            space,
            Some(window.window_id()),
            host_position,
            &policy,
            None,
        )
    }

    /// Resolves a rendered payload release into route and delivery facts from one snapshot.
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        let mut route_resolution = self
            .adapter
            .resolve_payload_drop_route_resolution(request, &policy);
        if let Some(resolution) =
            self.resolve_accepted_routed_preview_resolution(request, &route_resolution, cx)
        {
            self.status.record_route(request, resolution.route());
            return resolution;
        }

        let resolver_only_hover_request = request.release_origin()
            == crate::interaction::DockPayloadDropReleaseOrigin::HoveredHost
            && request.event_receiver_window().is_none();
        if resolver_only_hover_request
            && self.route_resolution_targets_unrefreshable_window(&route_resolution, cx)
        {
            let route = route_resolution.into_route();
            let resolution = self.resolve_payload_drop_delivery_resolution(request, route, cx);
            self.status.record_route(request, resolution.route());
            return resolution;
        }

        let source_only_preview_waiting_for_render = request.release_origin()
            == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly
            && self.routed_preview_targets_unowned_unrefreshable_window(cx);
        if source_only_preview_waiting_for_render {
            let route = route_resolution.into_route();
            let resolution = self.resolve_payload_drop_delivery_resolution(request, route, cx);
            self.status.record_route(request, resolution.route());
            return resolution;
        }

        self.reconcile_viewport_frame_except_window(request.event_receiver_window(), cx);
        let request = cx.read_entity(&self.controller, |_, app| {
            request
                .clone()
                .with_resampled_platform_target_context_from_app(app)
        });
        route_resolution = self
            .adapter
            .resolve_payload_drop_route_resolution(&request, &policy);
        if let Some(resolution) =
            self.resolve_accepted_routed_preview_resolution(&request, &route_resolution, cx)
        {
            self.status.record_route(&request, resolution.route());
            return resolution;
        }

        let route = route_resolution.into_route();
        let resolution = self.resolve_payload_drop_delivery_resolution(&request, route, cx);
        self.status.record_route(&request, resolution.route());
        resolution
    }

    fn route_resolution_targets_unrefreshable_window<C: open_gpui::AppContext>(
        &self,
        route_resolution: &DockViewportDropRouteResolution,
        cx: &mut C,
    ) -> bool {
        route_resolution
            .target_window(&self.adapter)
            .is_some_and(|window| self.window_ownership.is_window_unrefreshable(window, cx))
    }

    fn routed_preview_targets_unowned_unrefreshable_window<C: open_gpui::AppContext>(
        &self,
        cx: &mut C,
    ) -> bool {
        let Some(target) = self.routed_drop_preview.resolution_target_snapshot() else {
            return false;
        };
        let target_space = target.target_space();
        let Some(target_window_id) = target.target_window_id() else {
            return false;
        };
        self.adapter
            .window_for_space(target_space)
            .filter(|window| window.window_id() == target_window_id)
            .is_some_and(|window| {
                self.window_ownership
                    .is_unowned_window_unrefreshable(window, cx)
            })
    }

    fn resolve_payload_drop_delivery_resolution<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        cx.read_entity(&self.controller, |controller, _| {
            let workspace = controller.workspace();
            let payload_classes = workspace.payload_dock_classes_for_viewport_payload(
                request.payload(),
                request.source_node(),
            );
            self.resolve_payload_drop_delivery_resolution_with_workspace(
                request,
                route,
                workspace,
                &payload_classes,
            )
        })
    }

    fn resolve_payload_drop_delivery_resolution_with_workspace(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace: &crate::DockWorkspace,
        payload_classes: &crate::workspace_move_validation::DockPayloadDockClasses,
    ) -> DockViewportResolvedDropRoute {
        let mut route = route;
        let mut preview_target = None;
        let delivery_target = match crate::resolve_workspace_target_for_route(
            &self.adapter,
            &self.host_scenes,
            &route,
            request,
            workspace,
            payload_classes,
        ) {
            DockViewportWorkspaceRouteTarget::Resolved(target) => {
                preview_target = Some(target.clone());
                Some(target)
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget => None,
            DockViewportWorkspaceRouteTarget::RouteUnavailable => {
                route = DockViewportDropRoute::Unavailable;
                None
            }
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                preview_target = Some(target);
                route = DockViewportDropRoute::Rejected(reason);
                None
            }
            DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => None,
        };
        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            request,
            route.clone(),
            delivery_target,
        );
        DockViewportResolvedDropRoute::with_preview_target(route, delivery, preview_target)
    }

    #[cfg(test)]
    fn resolve_payload_drop_route_with_accepted_routed_preview<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &crate::DockPolicy,
        cx: &mut C,
    ) -> DockViewportDropRoute {
        let route_resolution = self
            .adapter
            .resolve_payload_drop_route_resolution(request, policy);
        let Some(accepted_preview_route) =
            self.resolve_accepted_routed_preview_route(request, &route_resolution, cx)
        else {
            return route_resolution.into_route();
        };
        accepted_preview_route
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> DockViewportDropRoute {
        self.reconcile_viewport_frame(cx);
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        self.resolve_payload_drop_route_with_accepted_routed_preview(request, &policy, cx)
    }

    #[cfg(test)]
    pub(crate) fn last_routed_viewport_identity_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockViewportIdentity> {
        let session = session?;
        self.active_drag
            .as_ref()
            .filter(|drag| drag.matches_session(session))
            .and_then(DockViewportActivePayloadDrag::last_routed_viewport_identity)
            .cloned()
    }

    #[cfg(test)]
    fn resolve_accepted_routed_preview_route<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        route_resolution: &DockViewportDropRouteResolution,
        cx: &mut C,
    ) -> Option<DockViewportDropRoute> {
        self.resolve_accepted_routed_preview_resolution(request, route_resolution, cx)
            .map(|resolution| resolution.route().clone())
    }

    fn resolve_accepted_routed_preview_resolution<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        route_resolution: &DockViewportDropRouteResolution,
        cx: &mut C,
    ) -> Option<DockViewportResolvedDropRoute> {
        if !self.can_replay_accepted_routed_preview(request, route_resolution) {
            return None;
        }
        let drag_session = request.drag_session()?;
        if !self
            .active_drag
            .as_ref()
            .is_some_and(|drag| drag.matches_session(drag_session))
        {
            return None;
        }
        let accepted = self
            .routed_drop_preview
            .accepted_for_drag_session(drag_session.id())?;
        let target = accepted.target().clone();
        let target_space = target.target_space().clone();
        let target_window_id = target.target_window_id()?;
        let accepted_target_key = accepted.target_key().clone();
        let host_position = self.accepted_routed_preview_host_position(request, &target_space)?;
        let facts_generation = self
            .adapter
            .snapshot_facts_generation(&target_space, target_window_id)?;
        let target_window = self.adapter.window_for_space(&target_space)?;
        if target_window.window_id() != target_window_id {
            return None;
        }
        let route = if target_space == *request.source_space() {
            DockViewportDropRoute::Local {
                host_position,
                window_id: target_window_id,
                facts_generation,
                authority: DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview,
            }
        } else {
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target_space.clone(),
                    target_window,
                    host_position,
                    facts_generation,
                ),
                authority: DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview,
            }
        };
        let resolution = cx.read_entity(&self.controller, |controller, _| {
            let workspace = controller.workspace();
            let payload_classes = workspace.payload_dock_classes_for_viewport_payload(
                request.payload(),
                request.source_node(),
            );
            crate::resolve_workspace_target_for_route(
                &self.adapter,
                &self.host_scenes,
                &route,
                request,
                workspace,
                &payload_classes,
            )
        });
        let (route, target) = match resolution {
            DockViewportWorkspaceRouteTarget::Resolved(target) => {
                if target.target_key() != &accepted_target_key {
                    return None;
                }
                (route, target)
            }
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                if target.target_key() != &accepted_target_key {
                    return None;
                }
                (DockViewportDropRoute::Rejected(reason), target)
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget
            | DockViewportWorkspaceRouteTarget::RouteUnavailable
            | DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => return None,
        };
        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            request,
            route.clone(),
            Some(target.clone()),
        );
        Some(DockViewportResolvedDropRoute::with_preview_target(
            route,
            delivery,
            Some(target),
        ))
    }

    fn can_replay_accepted_routed_preview(
        &self,
        request: &DockViewportDropRouteRequest,
        route_resolution: &DockViewportDropRouteResolution,
    ) -> bool {
        let replayable_coordinate_space = request.coordinate_space()
            == crate::DockViewportPointerCoordinateSpace::GlobalScreen
            || request.release_origin()
                == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly;
        if !replayable_coordinate_space {
            return false;
        }
        let Some(drag_session) = request.drag_session() else {
            return false;
        };
        let Some(accepted) = self
            .routed_drop_preview
            .accepted_for_drag_session(drag_session.id())
        else {
            return false;
        };
        match route_resolution.route_ref() {
            DockViewportDropRoute::Unavailable => {
                route_resolution.unavailable_reason()
                    == Some(crate::DockViewportDropRouteUnavailableReason::NoViewportAuthority)
            }
            DockViewportDropRoute::TearOff => {
                request.release_origin()
                    == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly
            }
            DockViewportDropRoute::Rejected(_) => false,
            DockViewportDropRoute::Local { window_id, .. } => {
                accepted.target_window_id() == Some(*window_id)
            }
            DockViewportDropRoute::KnownViewport { target, .. } => {
                accepted.matches_target(target.space(), target.window_id())
            }
        }
    }

    fn accepted_routed_preview_host_position(
        &self,
        request: &DockViewportDropRouteRequest,
        target_space: &DockSpaceId,
    ) -> Option<Point<Pixels>> {
        match request.coordinate_space() {
            crate::DockViewportPointerCoordinateSpace::GlobalScreen => self
                .adapter
                .global_screen_to_host(target_space, request.release_position()),
            crate::DockViewportPointerCoordinateSpace::SourceLocalOnly
                if request.release_origin()
                    == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly
                    && target_space == request.source_space() =>
            {
                self.adapter
                    .window_to_host(target_space, request.release_position())
            }
            crate::DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal
            | crate::DockViewportPointerCoordinateSpace::EventReceiverLocal
            | crate::DockViewportPointerCoordinateSpace::SourceLocalOnly => None,
        }
    }

    #[cfg(test)]
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

    pub(crate) fn begin_tear_off_request_with_focus(
        &mut self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        focus_item: Option<DockItemId>,
    ) -> DockViewportTearOffBeginOutcome {
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

    pub(crate) fn commit_prepared_tear_off_move(
        &mut self,
        pending: &DockViewportTearOffPending,
        cx: &mut App,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let Some(committed) = self.tear_off.take_committed(pending) else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        let action = self.commit_tear_off_move(pending, cx)?;
        debug_assert_eq!(&committed, pending);
        Ok(action)
    }

    pub(crate) fn complete_committed_tear_off_window(
        &mut self,
        pending: DockViewportTearOffPending,
        action: DockActionOutcome,
        window: impl Into<AnyWindowHandle>,
    ) -> DockViewportTearOffCompleted {
        self.complete_tear_off_registration(pending, action, window.into())
    }

    pub(crate) fn cancel_tear_off_if_source_unavailable(
        &mut self,
        pending: &DockViewportTearOffPending,
        key: &DockViewportTearOffKey,
        cx: &App,
    ) -> Option<DockViewportTearOffCancelled> {
        match self.tear_off_source_status(pending, cx) {
            DockViewportTearOffSourceStatus::Ready => None,
            DockViewportTearOffSourceStatus::Unavailable => self
                .cancel_tear_off_request(key, DockViewportTearOffCancelReason::SourceUnavailable),
        }
    }

    fn complete_tear_off_registration(
        &mut self,
        pending: DockViewportTearOffPending,
        action: DockActionOutcome,
        window: AnyWindowHandle,
    ) -> DockViewportTearOffCompleted {
        let registration = self.register_runtime_viewport(pending.target_space().clone(), window);
        DockViewportTearOffCompleted::new(
            pending,
            registration.outcome,
            registration.replaced_windows,
            action,
        )
    }

    fn focus_item_for_request(
        &self,
        request: &DockViewportTearOffRequest,
        cx: &App,
    ) -> Option<DockItemId> {
        self.controller
            .read(cx)
            .graph()
            .activation_focus_item_for_viewport_payload(
                request.payload(),
                request.source_node(),
                request
                    .drag_session()
                    .and_then(DockRuntimeDragSession::focus_item),
            )
    }

    pub(crate) fn drag_focus_item(
        &self,
        payload: &DockDragPayload,
        cx: &App,
    ) -> Option<DockItemId> {
        let focused_item = self.focus.focused_panel(&payload.source_space)?;
        self.controller
            .read(cx)
            .graph()
            .drag_focus_item_for_payload(payload, Some(focused_item))
    }

    fn focus_item_for_space(&self, space: &DockSpaceId, cx: &App) -> Option<DockItemId> {
        let focused_item = self.focus.focused_panel(space)?.clone();
        let controller = self.controller.read(cx);
        let graph = controller.graph();
        graph
            .find_item_in_space(space, &focused_item)
            .is_some()
            .then_some(focused_item)
    }

    /// Handles a GPUI window-closed notification by removing stale runtime mapping.
    ///
    /// Close policy is applied by [`Self::handle_window_should_close`] before GPUI accepts a close.
    /// Once a closed notification arrives, the platform window is already gone and docking must
    /// discard the runtime mapping even when the current policy is [`DockViewportClosePolicy::Prevent`].
    #[cfg(test)]
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        let outcome = self.cleanup_closed_window(window_id);
        self.status.record_close(&outcome);
        outcome
    }

    fn cleanup_closed_window(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        self.discard_owned_window(window_id);
        let outcome = self.adapter.handle_window_closed(window_id);
        if let Some(space) = outcome.space().cloned() {
            let _ = self.clear_runtime_window_state(&space, window_id, false);
        } else {
            self.host_scenes.unregister_window(window_id);
            let _ = self.clear_routed_drop_preview_if_window_matches(window_id);
        }
        outcome
    }

    /// Handles a GPUI window-closed notification with access to graph mutation context.
    pub(crate) fn handle_window_closed_with_app(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> DockViewportCloseOutcome {
        let pending_state = self.close_coordinator.take_window_close_state(window_id);
        let outcome = self.cleanup_closed_window(window_id);
        let outcome = match pending_state {
            Some(DockViewportClosePlanState::Pending(plan)) if outcome.space().is_some() => {
                let close_status =
                    crate::commit_prevalidated_merge_back_plan(&self.controller, &plan, cx);
                if close_status == DockViewportCloseStatus::MergedBack {
                    outcome.with_merge_back(plan)
                } else {
                    outcome.with_status(close_status)
                }
            }
            Some(DockViewportClosePlanState::Discarded) => {
                outcome.with_status(DockViewportCloseStatus::MergeBackFailed)
            }
            _ => outcome,
        };
        self.status.record_close(&outcome);
        outcome
    }
    pub(crate) fn activation_transaction_after_close(
        &mut self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> Option<DockViewportActivationTransaction> {
        if outcome.status() != DockViewportCloseStatus::MergedBack {
            return None;
        }
        let target_space = outcome.merge_target_space()?.clone();
        let focus_request = outcome.focus_item().cloned().map_or_else(
            DockViewportFocusRequest::no_panel_focus,
            DockViewportFocusRequest::panel,
        );
        match self.reusable_window_for_space(&target_space, cx) {
            DockViewportReusableWindow::Reused(window) => {
                Some(DockViewportActivationTransaction::close_recovery(
                    target_space,
                    window,
                    focus_request,
                ))
            }
            DockViewportReusableWindow::Missing | DockViewportReusableWindow::Stale => None,
        }
    }

    pub(crate) fn handle_window_should_close_with_app_and_refresh(
        &mut self,
        window_id: WindowId,
        cx: &mut App,
    ) -> (DockViewportShouldCloseOutcome, Vec<AnyWindowHandle>) {
        if self.adapter.window_close_requested(window_id) {
            let outcome = self.allowed_should_close_outcome(window_id);
            self.status.record_should_close(&outcome);
            return (outcome, Vec::new());
        }
        let outcome = self
            .adapter
            .should_close_viewport(window_id, self.close_policy());
        let focus_item = outcome
            .space
            .as_ref()
            .and_then(|space| self.focus_item_for_space(space, cx));
        let outcome = self.close_coordinator.apply_should_close_plan(
            outcome,
            self.close_policy(),
            focus_item,
            &self.controller,
            cx,
        );
        let (_, windows) = self.apply_allowed_should_close_route_invalidation(&outcome);
        self.status.record_should_close(&outcome);
        (outcome, windows)
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
    ) -> (bool, Vec<AnyWindowHandle>) {
        if outcome.status == crate::DockViewportShouldCloseStatus::Allowed {
            return self.mark_viewport_window_close_requested(outcome.window_id);
        }
        (false, Vec::new())
    }

    fn next_tear_off_tick(&mut self) -> DockViewportTearOffTick {
        let tick = self.tear_off_tick;
        self.tear_off_tick = self.tear_off_tick.saturating_add(1);
        tick
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

fn extend_unique_windows(
    windows: &mut Vec<AnyWindowHandle>,
    next_windows: impl IntoIterator<Item = AnyWindowHandle>,
) {
    for window in next_windows {
        if windows
            .iter()
            .any(|existing| existing.window_id() == window.window_id())
        {
            continue;
        }
        windows.push(window);
    }
}

pub(crate) enum DockViewportReusableWindow {
    Missing,
    Reused(AnyWindowHandle),
    Stale,
}
