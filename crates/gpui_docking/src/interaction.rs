use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportDropRoute,
    drag::{DockDragPayload, DockDragPayloadIdentity},
    drop_preview::DockDropPreview,
    drop_runtime::{DockDropRuntime, DockHostDropScene, DockHostDropSceneFact},
    drop_target::DockResolvedDropTarget,
    geometry,
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{Bounds, Pixels, Point, point};

#[derive(Debug, Default)]
pub(crate) struct DockInteractionRuntime {
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    drop: DockDropRuntime,
    drop_route_preview: Option<DockDropPreview>,
    outside_release_poll: Option<DockOutsideReleasePollSession>,
    next_outside_release_poll_id: u64,
    viewport_host_scene_frame: Option<DockViewportHostSceneFrame>,
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
}

impl DockRuntimeDragSession {
    fn new(id: u64, payload: &DockDragPayload) -> Self {
        Self {
            id,
            payload: payload.identity(),
        }
    }

    fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.payload == payload.identity()
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

    fn accepts_payload(&self, payload: &DockDragPayload) -> bool {
        self.drag.accepts_payload(payload)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPayloadDropRelease {
    payload: DockDragPayload,
    /// Host space that observed the release; runtime routing may choose a different target.
    host_space: DockSpaceId,
    release_position: Point<Pixels>,
}

impl DockPayloadDropRelease {
    pub(crate) fn new(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self {
            payload,
            host_space,
            release_position,
        }
    }

    pub(crate) fn payload(&self) -> &DockDragPayload {
        &self.payload
    }

    pub(crate) fn host_space(&self) -> &DockSpaceId {
        &self.host_space
    }

    pub(crate) fn release_position(&self) -> Point<Pixels> {
        self.release_position
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOutsideReleasePollRequest {
    session: DockOutsideReleasePollSession,
    payload: Option<DockDragPayload>,
    left_button_pressed: Option<bool>,
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
            host_space,
            release_position,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockOutsideReleasePollDecision {
    Inactive,
    Continue,
    CommitRelease(DockPayloadDropRelease),
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockRenderedOutsideReleaseRequest {
    viewport_runtime_available: bool,
    payload: Option<DockDragPayload>,
    /// Host space that observed the rendered mouse-up outside event.
    host_space: DockSpaceId,
    release_position: Point<Pixels>,
}

impl DockRenderedOutsideReleaseRequest {
    pub(crate) fn new(
        viewport_runtime_available: bool,
        payload: Option<DockDragPayload>,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
    ) -> Self {
        Self {
            viewport_runtime_available,
            payload,
            host_space,
            release_position,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockRenderedOutsideReleaseDecision {
    Inactive,
    CommitRelease(DockPayloadDropRelease),
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

    pub(crate) fn begin_drop_scene(
        &mut self,
        scene: DockHostDropScene,
        policy: &DockPolicy,
    ) -> bool {
        self.drop.begin_scene(scene, policy)
    }

    pub(crate) fn push_drop_scene_fact(
        &mut self,
        position: Point<Pixels>,
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &DockPolicy,
    ) -> bool {
        self.drop
            .push_scene_fact(position, excluded_tabs, fact, policy)
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

    pub(crate) fn take_resolved_drop_target(&mut self) -> Option<DockResolvedDropTarget> {
        self.drop.take_resolved_target()
    }

    pub(crate) fn update_drop_route_preview(
        &mut self,
        route: &DockViewportDropRoute,
        host_position: Point<Pixels>,
    ) -> bool {
        self.set_drop_route_preview(DockDropPreview::from_viewport_route(route, host_position))
    }

    pub(crate) fn clear_drop_route_preview(&mut self) -> bool {
        self.set_drop_route_preview(None)
    }

    pub(crate) fn begin_outside_release_poll(
        &mut self,
        payload: &DockDragPayload,
    ) -> Option<DockOutsideReleasePollSession> {
        if self.outside_release_poll.is_some() {
            return None;
        }

        let id = self.next_outside_release_poll_id.wrapping_add(1);
        self.next_outside_release_poll_id = id;
        let session = DockOutsideReleasePollSession::new(id, payload);
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
        self.outside_release_poll.take().is_some()
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
            self.finish_outside_release_poll(session);
            return DockOutsideReleasePollDecision::Stop;
        };
        if !self.outside_release_poll_session_accepts_payload(session, &payload) {
            self.finish_outside_release_poll(session);
            return DockOutsideReleasePollDecision::Stop;
        }

        match request.left_button_pressed {
            Some(true) => DockOutsideReleasePollDecision::Continue,
            Some(false) => {
                self.finish_outside_release_poll(session);
                DockOutsideReleasePollDecision::CommitRelease(DockPayloadDropRelease::new(
                    payload,
                    request.host_space,
                    request.release_position,
                ))
            }
            None => {
                self.finish_outside_release_poll(session);
                DockOutsideReleasePollDecision::Stop
            }
        }
    }

    pub(crate) fn rendered_outside_release(
        &mut self,
        request: DockRenderedOutsideReleaseRequest,
    ) -> DockRenderedOutsideReleaseDecision {
        self.cancel_outside_release_poll();

        if !request.viewport_runtime_available {
            return DockRenderedOutsideReleaseDecision::Inactive;
        }

        let Some(payload) = request.payload else {
            return DockRenderedOutsideReleaseDecision::Inactive;
        };

        DockRenderedOutsideReleaseDecision::CommitRelease(DockPayloadDropRelease::new(
            payload,
            request.host_space,
            request.release_position,
        ))
    }

    pub(crate) fn drop_preview(&self) -> Option<DockDropPreview> {
        self.drop
            .drop_resolution()
            .and_then(DockDropPreview::from_resolution)
            .or_else(|| self.drop_route_preview.clone())
    }

    fn set_drop_route_preview(&mut self, preview: Option<DockDropPreview>) -> bool {
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
    use crate::{DockItemId, DockNodeId};
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
        viewport_runtime_available: bool,
        payload: Option<DockDragPayload>,
    ) -> DockRenderedOutsideReleaseRequest {
        DockRenderedOutsideReleaseRequest::new(
            viewport_runtime_available,
            payload,
            DockSpaceId::from("host"),
            point(px(120.0), px(80.0)),
        )
    }

    #[test]
    fn payload_drop_release_carries_payload_host_and_position() {
        let payload = item_payload("a", "Panel A");
        let host_space = DockSpaceId::from("host");
        let release_position = point(px(120.0), px(80.0));

        let release =
            DockPayloadDropRelease::new(payload.clone(), host_space.clone(), release_position);

        assert_eq!(release.payload(), &payload);
        assert_eq!(release.host_space(), &host_space);
        assert_eq!(release.release_position(), release_position);
    }

    #[test]
    fn rendered_outside_release_requires_viewport_runtime_and_payload() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        let host_space = DockSpaceId::from("host");
        let release_position = point(px(120.0), px(80.0));

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
            DockRenderedOutsideReleaseDecision::CommitRelease(DockPayloadDropRelease::new(
                payload,
                host_space,
                release_position,
            ))
        );
    }

    #[test]
    fn rendered_outside_release_stops_outside_poll_session() {
        let mut runtime = DockInteractionRuntime::default();
        let payload = item_payload("a", "Panel A");
        runtime
            .begin_outside_release_poll(&payload)
            .expect("poll session should start");

        assert_eq!(
            runtime.rendered_outside_release(rendered_request(true, Some(payload.clone()))),
            DockRenderedOutsideReleaseDecision::CommitRelease(DockPayloadDropRelease::new(
                payload,
                DockSpaceId::from("host"),
                point(px(120.0), px(80.0)),
            ))
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
            DockRenderedOutsideReleaseDecision::Inactive
        );
        assert!(
            !runtime.outside_release_poll_session_active(&missing_payload_session),
            "rendered outside release should stop stale poll even without a payload"
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

        assert!(runtime.update_drop_route_preview(
            &DockViewportDropRoute::Rejected(crate::DockPolicyError::PlatformViewportsDisabled),
            position,
        ));
        assert_eq!(
            runtime
                .drop_preview()
                .expect("route preview should be visible")
                .kind,
            crate::drop_preview::DockDropPreviewKind::RejectedRoute
        );

        runtime.begin_drop_scene(DockHostDropScene::new(position), &DockPolicy::default());
        runtime.push_drop_scene_fact(
            position,
            None,
            DockHostDropSceneFact::Leaf(crate::drop_target::DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 200.0, 160.0),
                is_central: false,
            }),
            &DockPolicy::default(),
        );

        assert_eq!(
            runtime
                .drop_preview()
                .expect("local preview should be visible")
                .kind,
            crate::drop_preview::DockDropPreviewKind::Local
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
            DockOutsideReleasePollDecision::CommitRelease(DockPayloadDropRelease::new(
                payload,
                DockSpaceId::from("host"),
                point(px(120.0), px(80.0)),
            ))
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
            DockOutsideReleasePollDecision::Stop
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
            DockOutsideReleasePollDecision::Stop
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
