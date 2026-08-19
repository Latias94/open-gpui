use crate::{
    DockItemId, DockNodeId, DockSpaceId, DockSplitResize, DockViewportFocusCommand,
    DockViewportFocusCommandSource, DockViewportRuntimeLineage, SplitAxis,
    drag::{DockDragPayload, DockDragPayloadIdentity, DockDragTearOffGeometry},
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{Bounds, Pixels, Point, point};
use open_gpui_ui_core::{resize_split_fractions_by_pixels, ui_px};

#[derive(Debug, Default)]
pub(crate) struct DockInteractionRuntime {
    splitter_drag: Option<SplitterDrag>,
    floating_drag: Option<FloatingDrag>,
    payload_drag_anchor: Option<DockPayloadDragAnchor>,
    viewport_host_scene_frame: Option<DockViewportHostSceneFrame>,
    next_focus_command_generation: u64,
    pending_focus_command: Option<DockPendingFocusCommand>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPendingFocusCommand {
    generation: u64,
    command: DockViewportFocusCommand,
}

impl DockPendingFocusCommand {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn command(&self) -> &DockViewportFocusCommand {
        &self.command
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SplitterDrag {
    axes: Vec<SplitterDragAxis>,
}

impl SplitterDrag {
    pub(crate) fn axis_count(&self) -> usize {
        self.axes.len()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SplitterDragAxis {
    pub(crate) axis: SplitAxis,
    pub(crate) split: DockNodeId,
    pub(crate) handle_index: usize,
    pub(crate) start_position: Pixels,
    pub(crate) split_extent: Pixels,
    pub(crate) initial_fractions: Vec<f32>,
}

impl SplitterDragAxis {
    pub(crate) fn new(
        axis: SplitAxis,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) -> Self {
        Self {
            axis,
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        }
    }

    fn is_horizontal(&self) -> bool {
        matches!(self.axis, SplitAxis::Horizontal)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FloatingDrag {
    pub(crate) space: DockSpaceId,
    pub(crate) floating: DockNodeId,
    pub(crate) start_position: Point<Pixels>,
    pub(crate) initial_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPayloadDragAnchor {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    position: Point<Pixels>,
    session: Option<DockRuntimeDragSession>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockSplitterResizeRequest {
    pub(crate) updates: Vec<DockSplitResize>,
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
    lineage: DockViewportRuntimeLineage,
    payload: DockDragPayloadIdentity,
    focus_item: Option<DockItemId>,
}

impl DockRuntimeDragSession {
    #[cfg(test)]
    pub(crate) fn new(id: u64, payload: &DockDragPayload) -> Self {
        Self::with_lineage_and_focus_item(id, DockViewportRuntimeLineage::Unmanaged, payload, None)
    }

    pub(crate) fn with_lineage_and_focus_item(
        id: u64,
        lineage: DockViewportRuntimeLineage,
        payload: &DockDragPayload,
        focus_item: Option<DockItemId>,
    ) -> Self {
        Self {
            id,
            lineage,
            payload: payload.identity(),
            focus_item,
        }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn lineage(&self) -> DockViewportRuntimeLineage {
        self.lineage
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPayloadDropRelease {
    payload: DockDragPayload,
    drag_session: Option<DockRuntimeDragSession>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    origin: DockPayloadDropReleaseOrigin,
    event_receiver_local_scene_proof: Option<DockViewportHostSceneFrame>,
    /// Host space that observed the release; runtime routing may choose a different target.
    host_space: DockSpaceId,
    window_position: Point<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPayloadDropReleaseOrigin {
    /// Release was observed by the host/window under the dragged payload.
    HoveredHost,
    /// Release was observed by the source-owned captured-pointer transport.
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

    #[cfg(test)]
    pub(crate) fn hovered_host_with_session(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        release_position: Point<Pixels>,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        Self::hovered_host_with_positions(payload, host_space, release_position, drag_session)
    }

    pub(crate) fn hovered_host_with_positions(
        payload: DockDragPayload,
        host_space: DockSpaceId,
        window_position: Point<Pixels>,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        Self {
            payload,
            drag_session,
            tear_off_geometry: None,
            origin: DockPayloadDropReleaseOrigin::HoveredHost,
            event_receiver_local_scene_proof: None,
            host_space,
            window_position,
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

    #[cfg(test)]
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
            window_position: release_position,
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
        self.window_position
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

impl DockInteractionRuntime {
    pub(crate) fn request_viewport_focus_command(
        &mut self,
        command: DockViewportFocusCommand,
    ) -> bool {
        if self
            .pending_focus_command
            .as_ref()
            .is_some_and(|pending| pending.command == command)
        {
            return false;
        }
        if self.pending_focus_command.as_ref().is_some_and(|existing| {
            matches!(
                (command.source(), existing.command.source()),
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
        self.next_focus_command_generation =
            self.next_focus_command_generation.wrapping_add(1).max(1);
        self.pending_focus_command = Some(DockPendingFocusCommand {
            generation: self.next_focus_command_generation,
            command,
        });
        true
    }

    #[cfg(test)]
    pub(crate) fn pending_focus_command(&self) -> Option<&DockViewportFocusCommand> {
        self.pending_focus_command
            .as_ref()
            .map(DockPendingFocusCommand::command)
    }

    pub(crate) fn pending_focus_command_ticket(&self) -> Option<DockPendingFocusCommand> {
        self.pending_focus_command.clone()
    }

    pub(crate) fn take_pending_focus_command(&mut self) -> Option<DockViewportFocusCommand> {
        self.pending_focus_command
            .take()
            .map(|pending| pending.command)
    }

    pub(crate) fn take_pending_focus_command_if_generation(
        &mut self,
        generation: u64,
    ) -> Option<DockViewportFocusCommand> {
        if self
            .pending_focus_command
            .as_ref()
            .is_some_and(|pending| pending.generation == generation)
        {
            return self.take_pending_focus_command();
        }
        None
    }

    pub(crate) fn reset_window_bound_state(&mut self) {
        self.splitter_drag = None;
        self.floating_drag = None;
        self.payload_drag_anchor = None;
        self.viewport_host_scene_frame = None;
    }
}

impl DockInteractionRuntime {
    pub(crate) fn record_payload_drag_anchor(
        &mut self,
        source_space: DockSpaceId,
        source_node: DockNodeId,
        position: Point<Pixels>,
    ) {
        self.payload_drag_anchor = Some(DockPayloadDragAnchor {
            source_space,
            source_node,
            position,
            session: None,
        });
    }

    pub(crate) fn bind_payload_drag_anchor_session(
        &mut self,
        payload: &DockDragPayload,
        session: &DockRuntimeDragSession,
    ) -> bool {
        let Some(anchor) = self.payload_drag_anchor.as_mut().filter(|anchor| {
            anchor.source_space == payload.source_space
                && anchor.source_node == payload.source_node
                && session.accepts_payload(payload)
        }) else {
            return false;
        };
        if anchor.session.as_ref() == Some(session) {
            return false;
        }
        anchor.session = Some(session.clone());
        true
    }

    pub(crate) fn payload_drag_anchor_position(
        &self,
        payload: &DockDragPayload,
    ) -> Option<Point<Pixels>> {
        self.payload_drag_anchor
            .as_ref()
            .filter(|anchor| {
                anchor.source_space == payload.source_space
                    && anchor.source_node == payload.source_node
            })
            .map(|anchor| anchor.position)
    }

    pub(crate) fn clear_any_payload_drag_anchor(&mut self) -> bool {
        self.payload_drag_anchor.take().is_some()
    }

    pub(crate) fn clear_payload_drag_anchor_for_session(
        &mut self,
        session: &DockRuntimeDragSession,
    ) -> bool {
        if self
            .payload_drag_anchor
            .as_ref()
            .is_some_and(|anchor| anchor.session.as_ref() == Some(session))
        {
            self.payload_drag_anchor.take();
            true
        } else {
            false
        }
    }

    pub(crate) fn start_splitter_drag(
        &mut self,
        axis: SplitAxis,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
    ) {
        self.splitter_drag = Some(SplitterDrag {
            axes: vec![SplitterDragAxis::new(
                axis,
                split,
                handle_index,
                start_position,
                split_extent,
                initial_fractions,
            )],
        });
    }

    pub(crate) fn start_corner_splitter_drag(
        &mut self,
        horizontal: SplitterDragAxis,
        vertical: SplitterDragAxis,
    ) {
        self.splitter_drag = Some(SplitterDrag {
            axes: vec![horizontal, vertical],
        });
    }

    pub(crate) fn resize_split_request(
        &self,
        position: Point<Pixels>,
        split_min_size: Pixels,
    ) -> Option<DockSplitterResizeRequest> {
        let drag = self.splitter_drag.as_ref()?;
        let updates = drag
            .axes
            .iter()
            .map(|axis| {
                let current_position = if axis.is_horizontal() {
                    position.x
                } else {
                    position.y
                };
                let delta = current_position - axis.start_position;
                resize_split_fractions_by_pixels(
                    &axis.initial_fractions,
                    axis.handle_index,
                    ui_px(f32::from(axis.split_extent)),
                    ui_px(f32::from(delta)),
                    ui_px(f32::from(split_min_size)),
                )
                .map(|fractions| DockSplitResize::new(axis.split, fractions))
            })
            .collect::<Option<Vec<_>>>()?;

        (!updates.is_empty()).then_some(DockSplitterResizeRequest { updates })
    }

    pub(crate) fn corner_splitter_drag_active(&self) -> bool {
        self.splitter_drag
            .as_ref()
            .is_some_and(|drag| drag.axis_count() > 1)
    }

    pub(crate) fn splitter_drag_active(&self) -> bool {
        self.splitter_drag.is_some()
    }

    pub(crate) fn splitter_drag_matches(&self, split: DockNodeId, handle_index: usize) -> bool {
        self.splitter_drag.as_ref().is_some_and(|drag| {
            drag.axes
                .iter()
                .any(|axis| axis.split == split && axis.handle_index == handle_index)
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

    pub(crate) fn floating_drag_active(&self) -> bool {
        self.floating_drag.is_some()
    }

    pub(crate) fn finish_floating_drag(&mut self) -> bool {
        self.floating_drag.take().is_some()
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

    #[cfg(test)]
    pub(crate) fn splitter_drag(&self) -> Option<&SplitterDrag> {
        self.splitter_drag.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn floating_drag(&self) -> Option<&FloatingDrag> {
        self.floating_drag.as_ref()
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

    fn item_payload_for_node(
        space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        item: &str,
    ) -> DockDragPayload {
        DockDragPayload::new_item(
            space.into(),
            source_node,
            DockItemId::from(item),
            item.to_string(),
        )
    }

    #[test]
    fn stale_focus_command_generation_cannot_consume_a_requeued_equal_command() {
        let mut runtime = DockInteractionRuntime::default();
        let command = DockViewportFocusCommand::viewport_activation(
            crate::DockViewportFocusRequest::panel("a"),
        );
        assert!(runtime.request_viewport_focus_command(command.clone()));
        let stale = runtime
            .pending_focus_command_ticket()
            .expect("the first focus command should have a ticket");
        assert_eq!(runtime.take_pending_focus_command(), Some(command.clone()));

        assert!(runtime.request_viewport_focus_command(command.clone()));
        let current = runtime
            .pending_focus_command_ticket()
            .expect("the requeued focus command should have a new ticket");
        assert_ne!(stale.generation(), current.generation());
        assert_eq!(
            runtime.take_pending_focus_command_if_generation(stale.generation()),
            None
        );
        assert_eq!(runtime.pending_focus_command(), Some(&command));
        assert_eq!(
            runtime.take_pending_focus_command_if_generation(current.generation()),
            Some(command)
        );
    }

    fn assert_resize_update(update: &DockSplitResize, split: DockNodeId, expected: &[f32]) {
        assert_eq!(update.split, split);
        assert_eq!(update.fractions.len(), expected.len());
        for (actual, expected) in update.fractions.iter().zip(expected.iter()) {
            assert!(
                (*actual - *expected).abs() < 0.0001,
                "expected fraction {actual} to be close to {expected}"
            );
        }
    }

    #[test]
    fn payload_drag_anchor_matches_any_payload_from_same_source_tabs() {
        let mut runtime = DockInteractionRuntime::default();
        let mut graph = crate::DockGraph::new();
        let source_tabs = graph.insert_node(crate::DockNode::Tabs {
            items: Vec::new(),
            selected: None,
        });
        let position = point(px(42.0), px(17.0));
        let source_space = DockSpaceId::from("main");
        let item_payload = item_payload_for_node(source_space.clone(), source_tabs, "a");
        let tabs_payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "tabs".to_string());

        runtime.record_payload_drag_anchor(source_space, source_tabs, position);

        assert_eq!(
            runtime.payload_drag_anchor_position(&item_payload),
            Some(position)
        );
        assert_eq!(
            runtime.payload_drag_anchor_position(&tabs_payload),
            Some(position)
        );
        assert_eq!(
            runtime.payload_drag_anchor_position(&item_payload_for_node(
                DockSpaceId::from("other"),
                source_tabs,
                "a",
            )),
            None
        );
        assert!(runtime.clear_any_payload_drag_anchor());
        assert_eq!(runtime.payload_drag_anchor_position(&item_payload), None);
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
    fn splitter_update_without_active_drag_has_no_action() {
        let runtime = DockInteractionRuntime::default();

        assert_eq!(
            runtime.resize_split_request(point(px(120.0), px(0.0)), px(96.0)),
            None
        );
    }

    #[test]
    fn splitter_drag_produces_resize_request() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_splitter_drag(
            SplitAxis::Horizontal,
            split,
            0,
            px(100.0),
            px(400.0),
            vec![0.5, 0.5],
        );

        assert_eq!(
            runtime.resize_split_request(point(px(180.0), px(0.0)), px(96.0)),
            Some(DockSplitterResizeRequest {
                updates: vec![DockSplitResize::new(split, [0.7, 0.3])],
            })
        );
    }

    #[test]
    fn corner_splitter_drag_produces_two_axis_resize_request() {
        let horizontal = DockNodeId::null();
        let vertical = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_corner_splitter_drag(
            SplitterDragAxis::new(
                SplitAxis::Horizontal,
                horizontal,
                0,
                px(100.0),
                px(400.0),
                vec![0.5, 0.5],
            ),
            SplitterDragAxis::new(
                SplitAxis::Vertical,
                vertical,
                0,
                px(60.0),
                px(200.0),
                vec![0.5, 0.5],
            ),
        );

        let request = runtime
            .resize_split_request(point(px(180.0), px(80.0)), px(20.0))
            .expect("corner drag should resize both axes");
        assert_eq!(request.updates.len(), 2);
        assert_resize_update(&request.updates[0], horizontal, &[0.7, 0.3]);
        assert_resize_update(&request.updates[1], vertical, &[0.6, 0.4]);
    }

    #[test]
    fn corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis() {
        let horizontal = DockNodeId::null();
        let vertical = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();
        runtime.start_corner_splitter_drag(
            SplitterDragAxis::new(
                SplitAxis::Horizontal,
                horizontal,
                0,
                px(100.0),
                px(400.0),
                vec![0.5, 0.5],
            ),
            SplitterDragAxis::new(
                SplitAxis::Vertical,
                vertical,
                0,
                px(100.0),
                px(400.0),
                vec![0.5, 0.5],
            ),
        );

        let request = runtime
            .resize_split_request(point(px(-100.0), px(180.0)), px(96.0))
            .expect("corner drag should keep valid resize updates");
        assert_eq!(request.updates.len(), 2);
        assert_resize_update(&request.updates[0], horizontal, &[0.24, 0.76]);
        assert_resize_update(&request.updates[1], vertical, &[0.7, 0.3]);
    }

    #[test]
    fn finishing_splitter_drag_reports_only_active_state_changes() {
        let split = DockNodeId::null();
        let mut runtime = DockInteractionRuntime::default();

        assert!(!runtime.finish_splitter_drag());
        runtime.start_splitter_drag(
            SplitAxis::Horizontal,
            split,
            0,
            px(100.0),
            px(400.0),
            vec![0.5, 0.5],
        );
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
}
