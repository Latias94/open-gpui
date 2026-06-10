use crate::{
    DockNodeId, DockPolicy, DockSpaceId, DockViewportDropRoute,
    drag::DockDragPayloadIdentity,
    drop_preview::DockDropPreview,
    drop_runtime::{DockDropRuntime, DockHostDropScene, DockHostDropSceneFact},
    drop_target::DockResolvedDropTarget,
    geometry,
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
pub(crate) struct DockOutsideReleasePollSession {
    id: u64,
    payload: DockDragPayloadIdentity,
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
        payload: DockDragPayloadIdentity,
    ) -> Option<DockOutsideReleasePollSession> {
        if self.outside_release_poll.is_some() {
            return None;
        }

        let id = self.next_outside_release_poll_id.wrapping_add(1);
        self.next_outside_release_poll_id = id;
        let session = DockOutsideReleasePollSession { id, payload };
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
        payload: &DockDragPayloadIdentity,
    ) -> bool {
        self.outside_release_poll_session_active(session) && &session.payload == payload
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
    use crate::{DockItemId, DockNodeId, drag::DockDragPayload};
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
        let payload = item_payload("a", "Panel A").identity();
        let session = runtime
            .begin_outside_release_poll(payload)
            .expect("poll session should start");
        assert!(runtime.outside_release_poll_running());
        assert_eq!(
            runtime.begin_outside_release_poll(item_payload("b", "Panel B").identity()),
            None
        );
        assert!(runtime.outside_release_poll_session_active(&session));
        assert!(runtime.finish_outside_release_poll(&session));
        assert!(!runtime.outside_release_poll_running());
        assert!(!runtime.finish_outside_release_poll(&session));
    }

    #[test]
    fn outside_release_poll_rejects_stale_session_finish() {
        let mut runtime = DockInteractionRuntime::default();

        let stale = runtime
            .begin_outside_release_poll(item_payload("a", "Panel A").identity())
            .expect("first poll session should start");
        assert!(runtime.cancel_outside_release_poll());
        let active = runtime
            .begin_outside_release_poll(item_payload("b", "Panel B").identity())
            .expect("second poll session should start");

        assert!(!runtime.finish_outside_release_poll(&stale));
        assert!(runtime.outside_release_poll_session_active(&active));
        assert!(runtime.finish_outside_release_poll(&active));
        assert!(!runtime.outside_release_poll_running());
    }

    #[test]
    fn outside_release_poll_session_rejects_different_payload_identity() {
        let mut runtime = DockInteractionRuntime::default();

        let session = runtime
            .begin_outside_release_poll(item_payload("a", "Panel A").identity())
            .expect("poll session should start");

        assert!(runtime.outside_release_poll_session_accepts_payload(
            &session,
            &item_payload("a", "Renamed Panel A").identity()
        ));
        assert!(!runtime.outside_release_poll_session_accepts_payload(
            &session,
            &item_payload("b", "Panel B").identity()
        ));
        assert!(runtime.finish_outside_release_poll(&session));
    }
}
