use crate::{
    DockItemId, DockNodeId, DockPolicy, DockSpaceId, DockViewportDropRoute,
    DockViewportFocusCommand, DockViewportFocusCommandSource, DockViewportResolvedDropRoute,
    drag::{DockDragPayload, DockDragPayloadIdentity, DockDragTearOffGeometry},
    drop_preview::{DockDropPreview, DockDropRoutePreview},
    drop_runtime::{DockDropRuntime, DockHostDropScene, DockHostDropSceneFact},
    drop_target::{DockDropTargetValidator, DockEdgePlanResolver, DockResolvedDropTarget},
    geometry::{self, DockDropGuideStyle},
    viewport_drop_scene::DockViewportHostSceneFrame,
    workspace_transaction::{DockWorkspacePayloadDropRequest, DockWorkspaceResolvedDropTarget},
};
use open_gpui::{Bounds, Pixels, Point, point};

#[derive(Debug, Default)]
pub(crate) struct DockInteractionRuntime {
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    drop: DockDropRuntime,
    drop_route_preview: Option<DockDropRoutePreview>,
    outside_release_poll: Option<DockOutsideReleasePollSession>,
    next_outside_release_poll_id: u64,
    viewport_host_scene_frame: Option<DockViewportHostSceneFrame>,
    pending_focus_command: Option<DockViewportFocusCommand>,
}

#[derive(Debug, Clone)]
pub(crate) struct SplitterDrag {
    pub(crate) split: DockNodeId,
    pub(crate) handle_index: usize,
    pub(crate) start_position: Pixels,
    pub(crate) split_extent: Pixels,
    pub(crate) initial_fractions: Vec<f32>,
}

#[derive(Debug, Clone)]
pub(crate) struct FloatingDrag {
    pub(crate) space: DockSpaceId,
    pub(crate) floating: DockNodeId,
    pub(crate) start_position: Point<Pixels>,
    pub(crate) initial_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSplitterResizeRequest {
    pub(crate) split: DockNodeId,
    pub(crate) fractions: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockFloatingBoundsRequest {
    pub(crate) space: DockSpaceId,
    pub(crate) floating: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockRuntimeDragSession {
    id: u64,
    payload: DockDragPayloadIdentity,
    focus_item: Option<DockItemId>,
}

impl DockRuntimeDragSession {
    pub(crate) fn new(id: u64, payload: &DockDragPayload) -> Self {
        Self::with_focus_item(id, payload, None)
    }

    pub(crate) fn with_focus_item(
        id: u64,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> Self {
        Self {
            id,
            payload: payload.identity(),
            focus_item,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.payload == payload.identity()
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        self.payload.source_space()
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockOutsideReleasePollSession {
    drag: DockRuntimeDragSession,
}

impl DockOutsideReleasePollSession {
    fn new(id: u64, payload: &DockDragPayload) -> Self {
        Self {
            drag: DockRuntimeDragSession::new(id, payload),
        }
    }

    fn from_drag_session(drag: DockRuntimeDragSession) -> Self {
        Self { drag }
    }

    fn drag_session(&self) -> &DockRuntimeDragSession {
        &self.drag
    }

    fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.drag.accepts_payload(payload)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPayloadDropRelease {
    payload: DockDragPayload,
    drag_session: Option<DockRuntimeDragSession>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    origin: DockPayloadDropReleaseOrigin,
    event_receiver_local_scene_proof: Option<DockViewportHostSceneFrame>,
    /// Host space that observed the release; runtime routing may choose a different target.
    host_space: DockSpaceId,
    release_position: Point<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockLocalDropDelivery {
    source_space: DockSpaceId,
    payload: DockDragPayload,
    target_space: DockSpaceId,
    target: DockResolvedDropTarget,
    frozen_focus_item: Option<DockItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPayloadDropReleaseOrigin {
    /// Release was observed by the host/window under the dragged payload.
    HoveredHost,
    /// Release was observed by the source host after pointer capture or outside-window polling.
    SourceOnly,
}

impl DockPayloadDropRelease {
    #[cfg(test)]
    pub(crate) fn hovered_host(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self::hovered_host_with_session(payload, host_space, release_position, None)
    }

    pub(crate) fn hovered_host_with_session(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        Self {
            payload,
            drag_session,
            tear_off_geometry: None,
            origin: DockPayloadDropReleaseOrigin::HoveredHost,
            event_receiver_local_scene_proof: None,
            host_space,
            release_position,
        }
    }

    #[cfg(test)]
    pub(crate) fn source_only(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self::source_only_with_session(payload, host_space, release_position, None)
    }

    pub(crate) fn source_only_with_session(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        Self {
            payload,
            drag_session,
            tear_off_geometry: None,
            origin: DockPayloadDropReleaseOrigin::SourceOnly,
            event_receiver_local_scene_proof: None,
            host_space,
            release_position,
        }
    }

    pub(crate) fn payload(&self) -> &DockDragPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn tear_off_geometry(&self) -> Option<DockDragTearOffGeometry> {
        self.tear_off_geometry
    }

    pub(crate) fn origin(&self) -> DockPayloadDropReleaseOrigin {
        self.origin
    }

    pub(crate) fn event_receiver_local_scene_proof(&self) -> Option<&DockViewportHostSceneFrame> {
        self.event_receiver_local_scene_proof.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn host_space(&self) -> &DockSpaceId {
        &self.host_space
    }

    pub(crate) fn release_position(&self) -> Point<Pixels> {
        self.release_position
    }

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }

    pub(crate) fn with_event_receiver_local_scene_proof(
        mut self,
        proof: Option<DockViewportHostSceneFrame>,
    ) -> Self {
        self.event_receiver_local_scene_proof =
            if self.origin == DockPayloadDropReleaseOrigin::HoveredHost {
                proof
            } else {
                None
            };
        self
    }
}

impl DockLocalDropDelivery {
    fn from_release(release: &DockPayloadDropRelease, target: DockResolvedDropTarget) -> Self {
        let frozen_focus_item = release
            .drag_session()
            .and_then(|session| session.focus_item())
            .cloned();
        Self {
            source_space: release.payload.source_space.clone(),
            payload: release.payload.clone(),
            target_space: release.host_space.clone(),
            target,
            frozen_focus_item,
        }
    }

    pub(crate) fn workspace_request(&self) -> DockWorkspacePayloadDropRequest<'_> {
        DockWorkspacePayloadDropRequest {
            source_space: &self.source_space,
            payload: self.payload.as_workspace_payload(),
            target: DockWorkspaceResolvedDropTarget::new(
                self.target_space.clone(),
                self.target.clone(),
            ),
            frozen_focus_item: self.frozen_focus_item.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOutsideReleasePollRequest {
    session: DockOutsideReleasePollSession,
    payload: Option<DockDragPayload>,
    left_button_pressed: Option<bool>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    /// Host space that owns the polling window.
    host_space: DockSpaceId,
    release_position: Point<Pixels>,
}

impl DockOutsideReleasePollRequest {
    pub(crate) fn new(
        session: DockOutsideReleasePollSession,
        payload: Option<DockDragPayload>,
        left_button_pressed: Option<bool>,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self {
            session,
            payload,
            left_button_pressed,
            tear_off_geometry: None,
            host_space,
            release_position,
        }
    }

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockOutsideReleasePollDecision {
    Inactive,
    Continue,
    CommitRelease(DockPayloadDropRelease),
    Stop(DockRuntimeDragSession),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockRenderedOutsideReleaseRequest {
    platform_viewports_allowed: bool,
    payload: Option<DockDragPayload>,
    drag_session: Option<DockRuntimeDragSession>,
    left_button_pressed: Option<bool>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    /// Host space that observed the rendered mouse-up outside event.
    host_space: DockSpaceId,
    release_position: Point<Pixels>,
}

impl DockRenderedOutsideReleaseRequest {
    pub(crate) fn new(
        platform_viewports_allowed: bool,
        payload: Option<DockDragPayload>,
        left_button_pressed: Option<bool>,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self {
            platform_viewports_allowed,
            payload,
            drag_session: None,
            left_button_pressed,
            tear_off_geometry: None,
            host_space,
            release_position,
        }
    }

    pub(crate) fn with_drag_session(
        mut self,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        self.drag_session = drag_session;
        self
    }

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockRenderedOutsideReleaseDecision {
    Inactive,
    StopDragSession(DockRuntimeDragSession),
    CommitRelease(DockPayloadDropRelease),
}

impl DockInteractionRuntime {
    pub(crate) fn request_viewport_focus_command(
        &mut self,
        command: DockViewportFocusCommand,
    ) -> bool {
        if self.pending_focus_command.as_ref() == Some(&command) {
            return false;
        }
        if self.pending_focus_command.as_ref().is_some_and(|existing| {
            matches!(
                (command.source(), existing.source()),
                (
                    DockViewportFocusCommandSource::PlatformActivation,
                    DockViewportFocusCommandSource::ViewportActivation
                        | DockViewportFocusCommandSource::CloseRecovery,
                ) | (
                    DockViewportFocusCommandSource::ViewportActivation,
                    DockViewportFocusCommandSource::CloseRecovery
                )
            )
        }) {
            return false;
        }
        self.pending_focus_command = Some(command);
        true
    }

    pub(crate) fn pending_focus_command(&self) -> Option<&DockViewportFocusCommand> {
        self.pending_focus_command.as_ref()
    }

    pub(crate) fn take_pending_focus_command(&mut self) -> Option<DockViewportFocusCommand> {
        self.pending_focus_command.take()
    }
}

impl DockInteractionRuntime {
    pub(crate) fn start_splitter_drag(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) {
        self.splitter_drag = Some(SplitterDrag {
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        });
    }

    pub(crate) fn resize_split_request(
        &self,
        position: Pixels,
        split_min_size: Pixels,
    ) -> Option<DockSplitterResizeRequest> {
        let drag = self.splitter_drag.as_ref()?;
        let delta = position - drag.start_position;
        let fractions = geometry::resize_adjacent_split_fractions(
            &drag.initial_fractions,
            drag.initial_fractions.len(),
            drag.handle_index,
            drag.split_extent,
            delta,
            split_min_size,
        )?;

        Some(DockSplitterResizeRequest {
            split: drag.split,
            fractions,
        })
    }

    pub(crate) fn finish_splitter_drag(&mut self) -> bool {
        self.splitter_drag.take().is_some()
    }

    pub(crate) fn start_floating_drag(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
    ) {
        self.floating_drag = Some(FloatingDrag {
            space,
            floating,
            start_position,
            initial_bounds,
        });
    }

    pub(crate) fn floating_bounds_request(
        &self,
        position: Point<Pixels>,
    ) -> Option<DockFloatingBoundsRequest> {
        let drag = self.floating_drag.as_ref()?;
        let delta = position - drag.start_position;
        let bounds = Bounds::new(
            point(
                drag.initial_bounds.origin.x + delta.x,
                drag.initial_bounds.origin.y + delta.y,
            ),
            drag.initial_bounds.size,
        );

        Some(DockFloatingBoundsRequest {
            space: drag.space.clone(),
            floating: drag.floating,
            bounds,
        })
    }

    pub(crate) fn finish_floating_drag(&mut self) -> bool {
        self.floating_drag.take().is_some()
    }

    #[cfg(test)]
    pub(crate) fn begin_drop_scene(
        &mut self,
        scene: DockHostDropScene,
        policy: &DockPolicy,
    ) -> bool {
        self.drop.begin_scene(scene, policy)
    }

    pub(crate) fn ensure_drop_scene_with_validator(
        &mut self,
        scene: DockHostDropScene,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> bool {
        self.drop
            .ensure_scene_with_validator(scene, policy, target_validator, edge_plan_resolver)
    }

    #[cfg(test)]
    pub(crate) fn push_drop_scene_fact(
        &mut self,
        position: Point<Pixels>,
        excluded_nodes: Vec<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        self.drop
            .push_scene_fact(position, excluded_nodes, fact, policy)
    }

    pub(crate) fn push_drop_scene_fact_with_validator(
        &mut self,
        position: Point<Pixels>,
        payload_size: Option<open_gpui::Size<Pixels>>,
        drop_guide_style: DockDropGuideStyle,
        excluded_nodes: Vec<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> bool {
        self.drop.push_scene_fact_with_validator(
            position,
            payload_size,
            drop_guide_style,
            excluded_nodes,
            fact,
            policy,
            target_validator,
            edge_plan_resolver,
        )
    }

    pub(crate) fn set_viewport_host_scene_frame(
        &mut self,
        frame: Option<DockViewportHostSceneFrame>,
    ) -> bool {
        if self.viewport_host_scene_frame == frame {
            return false;
        }
        self.viewport_host_scene_frame = frame;
        true
    }

    pub(crate) fn viewport_host_scene_frame(&self) -> Option<&DockViewportHostSceneFrame> {
        self.viewport_host_scene_frame.as_ref()
    }

    pub(crate) fn take_local_drop_delivery(
        &mut self,
        release: &DockPayloadDropRelease,
        policy: &DockPolicy,
        target_validator: Option<&DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&DockEdgePlanResolver<'_>>,
    ) -> Option<DockLocalDropDelivery> {
        if release.origin() == DockPayloadDropReleaseOrigin::SourceOnly {
            return None;
        }
        let target = self.drop.take_accepted_target_at(
            release.release_position(),
            policy,
            target_validator,
            edge_plan_resolver,
        )?;
        Some(DockLocalDropDelivery::from_release(release, target))
    }

    pub(crate) fn clear_drop_acceptance(&mut self) -> bool {
        self.drop.clear()
    }

    pub(crate) fn update_drop_route_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        host_position: Point<Pixels>,
    ) -> bool {
        let route = resolution.route();
        if matches!(route, DockViewportDropRoute::Unavailable) {
            return self.clear_drop_route_preview();
        }
        let preview_changed =
            self.set_drop_route_preview(DockDropRoutePreview::from_route(route, host_position));
        preview_changed
    }

    pub(crate) fn clear_drop_route_preview(&mut self) -> bool {
        self.set_drop_route_preview(None)
    }

    #[cfg(test)]
    pub(crate) fn begin_outside_release_poll(
        &mut self,
        payload: &DockDragPayload,
    ) -> Option<DockOutsideReleasePollSession> {
        self.begin_outside_release_poll_with_session(payload, None)
    }

    pub(crate) fn begin_outside_release_poll_with_session(
        &mut self,
        payload: &DockDragPayload,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Option<DockOutsideReleasePollSession> {
        if self.outside_release_poll.is_some() {
            return None;
        }

        let session = match drag_session {
            Some(drag_session) => {
                if !drag_session.accepts_payload(payload) {
                    return None;
                }
                DockOutsideReleasePollSession::from_drag_session(drag_session)
            }
            None => {
                let id = self.next_outside_release_poll_id.wrapping_add(1);
                self.next_outside_release_poll_id = id;
                DockOutsideReleasePollSession::new(id, payload)
            }
        };
        self.outside_release_poll = Some(session.clone());
        Some(session)
    }

    pub(crate) fn finish_outside_release_poll(
        &mut self,
        session: &DockOutsideReleasePollSession,
    ) -> bool {
        if self.outside_release_poll.as_ref() != Some(session) {
            return false;
        }
        self.outside_release_poll = None;
        true
    }

    pub(crate) fn cancel_outside_release_poll(&mut self) -> bool {
        self.cancel_outside_release_poll_session().is_some()
    }

    fn cancel_outside_release_poll_session(&mut self) -> Option<DockOutsideReleasePollSession> {
        self.outside_release_poll.take()
    }

    pub(crate) fn outside_release_poll_session_active(
        &self,
        session: &DockOutsideReleasePollSession,
    ) -> bool {
        self.outside_release_poll.as_ref() == Some(session)
    }

    pub(crate) fn outside_release_poll_session_accepts_payload(
        &self,
        session: &DockOutsideReleasePollSession,
        payload: &DockDragPayload,
    ) -> bool {
        self.outside_release_poll_session_active(session) && session.accepts_payload(payload)
    }

    pub(crate) fn poll_outside_release(
        &mut self,
        request: DockOutsideReleasePollRequest,
    ) -> DockOutsideReleasePollDecision {
        let session = &request.session;
        if !self.outside_release_poll_session_active(session) {
            return DockOutsideReleasePollDecision::Inactive;
        }

        let Some(payload) = request.payload else {
            let drag_session = session.drag_session().clone();
            self.finish_outside_release_poll(session);
            return DockOutsideReleasePollDecision::Stop(drag_session);
        };
        if !self.outside_release_poll_session_accepts_payload(session, &payload) {
            let drag_session = session.drag_session().clone();
            self.finish_outside_release_poll(session);
            return DockOutsideReleasePollDecision::Stop(drag_session);
        }

        match request.left_button_pressed {
            Some(true) => DockOutsideReleasePollDecision::Continue,
            Some(false) => {
                self.finish_outside_release_poll(session);
                DockOutsideReleasePollDecision::CommitRelease(
                    DockPayloadDropRelease::source_only_with_session(
                        payload,
                        request.host_space,
                        request.release_position,
                        Some(session.drag_session().clone()),
                    )
                    .with_tear_off_geometry(request.tear_off_geometry),
                )
            }
            None => {
                let drag_session = session.drag_session().clone();
                self.finish_outside_release_poll(session);
                DockOutsideReleasePollDecision::Stop(drag_session)
            }
        }
    }

    pub(crate) fn rendered_outside_release(
        &mut self,
        request: DockRenderedOutsideReleaseRequest,
    ) -> DockRenderedOutsideReleaseDecision {
        if !request.platform_viewports_allowed {
            return match self.cancel_outside_release_poll_session() {
                Some(session) => DockRenderedOutsideReleaseDecision::StopDragSession(
                    session.drag_session().clone(),
                ),
                None => DockRenderedOutsideReleaseDecision::Inactive,
            };
        }

        let Some(payload) = request.payload else {
            return match self.cancel_outside_release_poll_session() {
                Some(session) => DockRenderedOutsideReleaseDecision::StopDragSession(
                    session.drag_session().clone(),
                ),
                None => DockRenderedOutsideReleaseDecision::Inactive,
            };
        };

        let drag_session = if let Some(session) = self.outside_release_poll.as_ref() {
            if !session.accepts_payload(&payload) {
                return DockRenderedOutsideReleaseDecision::Inactive;
            }
            Some(session.drag_session().clone())
        } else if let Some(drag_session) = request.drag_session {
            if !drag_session.accepts_payload(&payload) {
                return DockRenderedOutsideReleaseDecision::Inactive;
            }
            Some(drag_session)
        } else {
            return DockRenderedOutsideReleaseDecision::Inactive;
        };

        match request.left_button_pressed {
            Some(true) => DockRenderedOutsideReleaseDecision::Inactive,
            Some(false) => {
                self.cancel_outside_release_poll();
                DockRenderedOutsideReleaseDecision::CommitRelease(
                    DockPayloadDropRelease::source_only_with_session(
                        payload,
                        request.host_space,
                        request.release_position,
                        drag_session,
                    )
                    .with_tear_off_geometry(request.tear_off_geometry),
                )
            }
            None => match self.cancel_outside_release_poll_session() {
                Some(session) => DockRenderedOutsideReleaseDecision::StopDragSession(
                    session.drag_session().clone(),
                ),
                None => DockRenderedOutsideReleaseDecision::Inactive,
            },
        }
    }

    pub(crate) fn drop_preview(&self) -> Option<DockDropPreview> {
        self.drop
            .drop_resolution()
            .and_then(DockDropPreview::from_resolution)
    }

    pub(crate) fn finish_drop_acceptance_pass(&mut self) -> bool {
        self.drop.finish_acceptance_pass()
    }

    pub(crate) fn drop_route_preview(&self) -> Option<DockDropRoutePreview> {
        self.drop_route_preview.clone()
    }

    fn set_drop_route_preview(&mut self, preview: Option<DockDropRoutePreview>) -> bool {
        if self.drop_route_preview == preview {
            return false;
        }
        self.drop_route_preview = preview;
        true
    }

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.floating_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn resolved_drop_target(&self) -> Option<&DockResolvedDropTarget> {
        self.drop.resolved_target()
    }

    #[cfg(test)]
    pub(crate) fn outside_release_poll_running(&self) -> bool {
        self.outside_release_poll.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockItemId, DockNodeId,
        drop_target::{DockLeafDropTarget, DockResolvedDropTargetKind},
        workspace_transaction::DockWorkspaceDropPayload,
    };
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn item_payload(item: &str, title: &str) -> DockDragPayload {
        DockDragPayload::new_item(
            DockSpaceId::from("main"),
            DockNodeId::null(),
            DockItemId::from(item),
            title.to_string(),
        )
    }

    fn poll_request(
        session: &DockOutsideReleasePollSession,
        payload: Option<DockDragPayload>,
        left_button_pressed: Option<bool>,
    ) -> DockOutsideReleasePollRequest {
        DockOutsideReleasePollRequest::new(
            session.clone(),
            payload,
            left_button_pressed,
            DockSpaceId::from("host"),
            point(px(120.0), px(80.0)),
        )
    }

    fn rendered_request(
        platform_viewports_allowed: bool,
        payload: Option<DockDragPayload>,
    ) -> DockRenderedOutsideReleaseRequest {
        rendered_request_with_button_state(platform_viewports_allowed, payload, Some(false))
    }

    fn rendered_request_with_button_state(
        platform_viewports_allowed: bool,
        payload: Option<DockDragPayload>,
        left_button_pressed: Option<bool>,
    ) -> DockRenderedOutsideReleaseRequest {
        DockRenderedOutsideReleaseRequest::new(
            platform_viewports_allowed,
            payload,
            left_button_pressed,
            DockSpaceId::from("host"),
            point(px(120.0), px(80.0)),
        )
    }

    #[test]
    fn payload_drop_release_carries_payload_host_and_position() {
        let payload = item_payload("a", "Panel A");
        let host_space = DockSpaceId::from("host");
        let release_position = point(px(120.0), px(80.0));

        let release = DockPayloadDropRelease::hovered_host(
            payload.clone(),
            host_space.clone(),
            release_position,
        );

        assert_eq!(release.payload(), &payload);
        assert_eq!(release.drag_session(), None);
        assert_eq!(release.origin(), DockPayloadDropReleaseOrigin::HoveredHost);
        assert_eq!(release.host_space(), &host_space);
        assert_eq!(release.release_position(), release_position);

        let source_only = DockPayloadDropRelease::source_only(
            payload.clone(),
            host_space.clone(),
            release_position,
        );
        assert_eq!(source_only.payload(), &payload);
        assert_eq!(
            source_only.origin(),
            DockPayloadDropReleaseOrigin::SourceOnly
        );
        assert_eq!(source_only.drag_session(), None);
        assert_eq!(source_only.host_space(), &host_space);
        assert_eq!(source_only.release_position(), release_position);

        let drag_session = DockRuntimeDragSession::new(42, &payload);
        let session_release = DockPayloadDropRelease::source_only_with_session(
            payload.clone(),
            host_space,
            release_position,
            Some(drag_session.clone()),
        );
        assert_eq!(session_release.drag_session(), Some(&drag_session));
    }

    #[test]
    fn local_drop_delivery_packages_previously_accepted_target_for_workspace_commit() {
        let tabs = DockNodeId::null();
        let position = point(px(120.0), px(90.0));
        let mut runtime = DockInteractionRuntime::default();
        runtime.begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_drop_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 240.0, 180.0),
                is_central: false,
            }),
            &DockPolicy::default(),
        );
        assert!(runtime.finish_drop_acceptance_pass());

        let release = DockPayloadDropRelease::hovered_host(
            item_payload("a", "Panel A"),
            DockSpaceId::from("main"),
            position,
        );
        let delivery = runtime
            .take_local_drop_delivery(&release, &DockPolicy::default(), None, None)
            .expect("previously accepted target should produce a local delivery");
        let request = delivery.workspace_request();

        assert_eq!(request.source_space, &DockSpaceId::from("main"));
        assert_eq!(request.target.target_space(), &DockSpaceId::from("main"));
        assert!(matches!(
            &request.payload,
            DockWorkspaceDropPayload::Item { item, .. } if *item == &DockItemId::from("a")
        ));
        assert_eq!(
            request.target.target().kind,
            DockResolvedDropTargetKind::LeafCenter {
                root: tabs,
                target_tabs: tabs,
            }
        );
    }

    #[test]
    fn local_drop_delivery_preserves_frozen_focus_item_for_tabs_payload() {
        let tabs = DockNodeId::null();
        let position = point(px(120.0), px(90.0));
        let mut runtime = DockInteractionRuntime::default();
        runtime.begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_drop_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 240.0, 180.0),
                is_central: false,
            }),
            &DockPolicy::default(),
        );
        assert!(runtime.finish_drop_acceptance_pass());

        let payload =
            DockDragPayload::new_tabs(DockSpaceId::from("main"), tabs, "Tabs".to_string());
        let focus_item = DockItemId::from("a");
        let release = DockPayloadDropRelease::hovered_host_with_session(
            payload.clone(),
            DockSpaceId::from("main"),
            position,
            Some(DockRuntimeDragSession::with_focus_item(
                42,
                &payload,
                Some(focus_item.clone()),
            )),
        );
        let delivery = runtime
            .take_local_drop_delivery(&release, &DockPolicy::default(), None, None)
            .expect("previously accepted target should produce a local delivery");
        let request = delivery.workspace_request();

        assert_eq!(request.frozen_focus_item, Some(&focus_item));
        assert!(matches!(
            &request.payload,
            DockWorkspaceDropPayload::Tabs { source_tabs } if *source_tabs == tabs
        ));
    }

    #[test]
    fn source_only_release_cannot_consume_cached_local_drop_delivery() {
        let tabs = DockNodeId::null();
        let position = point(px(120.0), px(90.0));
        let mut runtime = DockInteractionRuntime::default();
        runtime.begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_drop_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 240.0, 180.0),
                is_central: false,
            }),
            &DockPolicy::default(),
        );
        assert!(runtime.finish_drop_acceptance_pass());

        let release = DockPayloadDropRelease::source_only(
            item_payload("a", "Panel A"),
            DockSpaceId::from("main"),
            position,
        );

        assert!(
            runtime
                .take_local_drop_delivery(&release, &DockPolicy::default(), None, None)
                .is_none(),
            "source-only releases must route through viewport authority instead of cached local delivery"
        );
    }

    #[test]
    fn rendered_outside_release_requires_viewport_runtime_and_payload() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request(false, Some(payload.clone()))),
            DockRenderedOutsideReleaseDecision::Inactive
        );
        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, None)),
            DockRenderedOutsideReleaseDecision::Inactive
        );
        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, Some(payload.clone()))),
            DockRenderedOutsideReleaseDecision::Inactive
        );
    }

    #[test]
    fn rendered_outside_release_carries_request_drag_session() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let drag_session = DockRuntimeDragSession::new(9, &payload);
        let stale_session = DockRuntimeDragSession::new(10, &item_payload("b", "Panel B"));

        assert_eq!(
            runtime.rendered_outside_release(
                rendered_request(true, Some(payload.clone()))
                    .with_drag_session(Some(stale_session))
            ),
            DockRenderedOutsideReleaseDecision::Inactive,
            "rendered release must reject a session that belongs to a different payload"
        );

        assert_eq!(
            runtime.rendered_outside_release(
                rendered_request(true, Some(payload.clone()))
                    .with_drag_session(Some(drag_session.clone()))
            ),
            DockRenderedOutsideReleaseDecision::CommitRelease(
                DockPayloadDropRelease::source_only_with_session(
                    payload,
                    DockSpaceId::from("host"),
                    point(px(120.0), px(80.0)),
                    Some(drag_session),
                )
            )
        );
    }

    #[test]
    fn rendered_outside_release_stops_outside_poll_session() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, Some(payload.clone()))),
            DockRenderedOutsideReleaseDecision::CommitRelease(
                DockPayloadDropRelease::source_only_with_session(
                    payload,
                    DockSpaceId::from("host"),
                    point(px(120.0), px(80.0)),
                    Some(session.drag_session().clone()),
                )
            )
        );
        assert!(
            !runtime.outside_release_poll_running(),
            "rendered outside release should own poll session cleanup"
        );

        let payload = item_payload("b", "Panel B");
        let missing_payload_session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should restart");
        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, None)),
            DockRenderedOutsideReleaseDecision::StopDragSession(
                missing_payload_session.drag_session().clone()
            )
        );
        assert!(
            !runtime.outside_release_poll_session_active(&missing_payload_session),
            "rendered outside release should stop stale poll even without a payload"
        );
    }

    #[test]
    fn rendered_outside_release_rejects_stale_payload_without_stopping_active_poll() {
        let mut runtime = DockInteractionRuntime::default();
        let stale_payload = item_payload("a", "Panel A");
        let active_payload = item_payload("b", "Panel B");
        let active = runtime
            .begin_outside_release_poll(&active_payload)
            .expect("active poll session should start");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, Some(stale_payload))),
            DockRenderedOutsideReleaseDecision::Inactive
        );
        assert!(
            runtime.outside_release_poll_session_active(&active),
            "a stale rendered release must not cancel the active drag session"
        );
        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, Some(active_payload.clone()))),
            DockRenderedOutsideReleaseDecision::CommitRelease(
                DockPayloadDropRelease::source_only_with_session(
                    active_payload,
                    DockSpaceId::from("host"),
                    point(px(120.0), px(80.0)),
                    Some(active.drag_session().clone()),
                )
            )
        );
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn rendered_outside_release_waits_while_platform_button_state_is_pressed() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let active = runtime
            .begin_outside_release_poll(&payload)
            .expect("active poll session should start");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request_with_button_state(
                true,
                Some(payload.clone()),
                Some(true),
            )),
            DockRenderedOutsideReleaseDecision::Inactive
        );
        assert!(
            runtime.outside_release_poll_session_active(&active),
            "a rendered release that contradicts platform button state must not stop the active session"
        );

        assert_eq!(
            runtime.rendered_outside_release(rendered_request_with_button_state(
                true,
                Some(payload.clone()),
                Some(false),
            )),
            DockRenderedOutsideReleaseDecision::CommitRelease(
                DockPayloadDropRelease::source_only_with_session(
                    payload,
                    DockSpaceId::from("host"),
                    point(px(120.0), px(80.0)),
                    Some(active.drag_session().clone()),
                )
            )
        );
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn rendered_outside_release_requires_known_released_button_state() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let active = runtime
            .begin_outside_release_poll(&payload)
            .expect("active poll session should start");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request_with_button_state(
                true,
                Some(payload.clone()),
                None,
            )),
            DockRenderedOutsideReleaseDecision::StopDragSession(active.drag_session().clone()),
            "unknown platform button state must not be treated as a release"
        );
        assert!(
            !runtime.outside_release_poll_running(),
            "unknown button state should stop the ambiguous outside release poll"
        );

        let drag_session = DockRuntimeDragSession::new(11, &payload);
        assert_eq!(
            runtime.rendered_outside_release(
                rendered_request_with_button_state(true, Some(payload), None)
                    .with_drag_session(Some(drag_session))
            ),
            DockRenderedOutsideReleaseDecision::Inactive,
            "a one-shot rendered outside event without button authority cannot commit"
        );
    }

    #[test]
    fn splitter_update_without_active_drag_has_no_action() {
        let runtime = DockInteractionRuntime::default();

        assert_eq!(runtime.resize_split_request(px(120.0), px(96.0)), None);
    }

    #[test]
    fn splitter_drag_produces_resize_request() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_splitter_drag(split, 0, px(100.0), px(400.0), vec![0.5, 0.5]);

        assert_eq!(
            runtime.resize_split_request(px(180.0), px(96.0)),
            Some(DockSplitterResizeRequest {
                split,
                fractions: vec![0.7, 0.3],
            })
        );
    }

    #[test]
    fn finishing_splitter_drag_reports_only_active_state_changes() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.finish_splitter_drag());
        runtime.start_splitter_drag(split, 0, px(100.0), px(400.0), vec![0.5, 0.5]);
        assert!(runtime.finish_splitter_drag());
        assert!(!runtime.finish_splitter_drag());
    }

    #[test]
    fn floating_drag_produces_bounds_request() {
        let floating = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_floating_drag(
            DockSpaceId::from("main"),
            floating,
            point(px(10.0), px(20.0)),
            bounds(40.0, 50.0, 200.0, 100.0),
        );

        assert_eq!(
            runtime.floating_bounds_request(point(px(25.0), px(35.0))),
            Some(DockFloatingBoundsRequest {
                space: DockSpaceId::from("main"),
                floating,
                bounds: bounds(55.0, 65.0, 200.0, 100.0),
            })
        );
    }

    #[test]
    fn finishing_floating_drag_reports_only_active_state_changes() {
        let floating = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.finish_floating_drag());
        runtime.start_floating_drag(
            DockSpaceId::from("main"),
            floating,
            point(px(10.0), px(20.0)),
            bounds(40.0, 50.0, 200.0, 100.0),
        );
        assert!(runtime.finish_floating_drag());
        assert!(!runtime.finish_floating_drag());
    }

    #[test]
    fn drop_preview_prefers_local_resolution_over_route_preview() {
        let tabs = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        let position = point(px(80.0), px(60.0));
        let rejected_route =
            DockViewportDropRoute::Rejected(crate::DockPolicyError::PlatformViewportsDisabled);
        let rejected_resolution = DockViewportResolvedDropRoute::new(rejected_route, None);

        assert!(runtime.update_drop_route_preview(&rejected_resolution, position,));
        assert!(
            runtime.drop_preview().is_none(),
            "route marker should not be exposed as a target drop preview"
        );
        assert_eq!(
            runtime
                .drop_route_preview()
                .expect("route preview should be visible")
                .kind,
            crate::drop_preview::DockDropRoutePreviewKind::Rejected
        );

        runtime.begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_drop_scene_fact(
            position,
            Vec::new(),
            DockHostDropSceneFact::Leaf(crate::drop_target::DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 200.0, 160.0),
                is_central: false,
            }),
            &DockPolicy::default(),
        );

        assert!(
            runtime.drop_preview().is_some(),
            "local target preview should be visible"
        );
        assert!(
            runtime.drop_route_preview().is_some(),
            "route marker remains separate from the local target preview"
        );
    }

    #[test]
    fn outside_release_poll_tracks_single_running_task() {
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.outside_release_poll_running());
        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");
        assert!(runtime.outside_release_poll_running());
        let other_payload = item_payload("b", "Panel B");
        assert_eq!(runtime.begin_outside_release_poll(&other_payload), None);
        assert!(runtime.outside_release_poll_session_active(&session));
        assert!(runtime.finish_outside_release_poll(&session));
        assert!(!runtime.outside_release_poll_running());
        assert!(!runtime.finish_outside_release_poll(&session));
    }

    #[test]
    fn outside_release_poll_reuses_runtime_drag_session() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let drag_session = DockRuntimeDragSession::new(77, &payload);
        let session = runtime
            .begin_outside_release_poll_with_session(&payload, Some(drag_session.clone()))
            .expect("poll session should reuse matching runtime drag session");

        assert_eq!(session.drag_session(), &drag_session);
        assert!(runtime.finish_outside_release_poll(&session));

        let mut runtime = DockInteractionRuntime::default();
        assert_eq!(
            runtime.begin_outside_release_poll_with_session(
                &item_payload("b", "Panel B"),
                Some(drag_session),
            ),
            None,
            "a runtime drag session must not be reused for another payload"
        );
    }

    #[test]
    fn outside_release_poll_rejects_stale_session_finish() {
        let mut runtime = DockInteractionRuntime::default();

        let stale_payload = item_payload("a", "Panel A");
        let stale = runtime
            .begin_outside_release_poll(&stale_payload)
            .expect("first poll session should start");
        assert!(runtime.cancel_outside_release_poll());
        let active_payload = item_payload("b", "Panel B");
        let active = runtime
            .begin_outside_release_poll(&active_payload)
            .expect("second poll session should start");

        assert!(!runtime.finish_outside_release_poll(&stale));
        assert!(runtime.outside_release_poll_session_active(&active));
        assert!(runtime.finish_outside_release_poll(&active));
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn outside_release_poll_session_rejects_different_active_payload() {
        let mut runtime = DockInteractionRuntime::default();

        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");

        assert!(runtime.outside_release_poll_session_accepts_payload(
            &session,
            &item_payload("a", "Renamed Panel A")
        ));
        assert!(
            !runtime.outside_release_poll_session_accepts_payload(
                &session,
                &item_payload("b", "Panel B")
            )
        );
        assert!(runtime.finish_outside_release_poll(&session));
    }

    #[test]
    fn outside_release_poll_decides_continue_and_commit_release() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");

        assert_eq!(
            runtime.poll_outside_release(poll_request(&session, Some(payload.clone()), Some(true))),
            DockOutsideReleasePollDecision::Continue
        );
        assert!(runtime.outside_release_poll_session_active(&session));
        assert_eq!(
            runtime.poll_outside_release(poll_request(
                &session,
                Some(payload.clone()),
                Some(false)
            )),
            DockOutsideReleasePollDecision::CommitRelease(
                DockPayloadDropRelease::source_only_with_session(
                    payload,
                    DockSpaceId::from("host"),
                    point(px(120.0), px(80.0)),
                    Some(session.drag_session().clone()),
                )
            )
        );
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn outside_release_poll_stops_without_committing_missing_or_changed_payload() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");

        assert_eq!(
            runtime.poll_outside_release(poll_request(&session, None, Some(false))),
            DockOutsideReleasePollDecision::Stop(session.drag_session().clone())
        );
        assert!(!runtime.outside_release_poll_running());

        let payload = item_payload("a", "Panel A");
        let session = runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should restart");
        let changed_payload = item_payload("b", "Panel B");

        assert_eq!(
            runtime.poll_outside_release(poll_request(
                &session,
                Some(changed_payload),
                Some(false)
            )),
            DockOutsideReleasePollDecision::Stop(session.drag_session().clone())
        );
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn outside_release_poll_inactive_decision_preserves_newer_session() {
        let mut runtime = DockInteractionRuntime::default();
        let stale_payload = item_payload("a", "Panel A");
        let stale = runtime
            .begin_outside_release_poll(&stale_payload)
            .expect("first poll session should start");
        assert!(runtime.cancel_outside_release_poll());
        let active_payload = item_payload("b", "Panel B");
        let active = runtime
            .begin_outside_release_poll(&active_payload)
            .expect("second poll session should start");

        assert_eq!(
            runtime.poll_outside_release(poll_request(
                &stale,
                Some(item_payload("b", "Panel B")),
                Some(false),
            )),
            DockOutsideReleasePollDecision::Inactive
        );
        assert!(runtime.outside_release_poll_session_active(&active));
        assert!(runtime.finish_outside_release_poll(&active));
    }
}
