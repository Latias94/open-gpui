use crate::{
    DockEdgeDockSizing, DockHost, DockNodeId, DockPolicy, DropZone,
    drag::DockDragPayload,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_scene_fact,
    geometry::DockDropGuideStyle,
    host_interaction_outcome::DockHostInteractionOutcome,
    host_render_actions::DockRenderedPointerPosition,
    workspace_move_validation::dock_target_validator,
};
use open_gpui::{Bounds, Context, Pixels, Point, Size, Window};

impl DockHost {
    pub(crate) fn ensure_host_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let (policy, payload_classes, drop_guide_style, graph) =
            self.with_workspace(cx, |workspace| {
                (
                    workspace.policy().clone(),
                    workspace.payload_dock_classes_for_drag_payload(payload),
                    workspace.options().drop_guide_style,
                    workspace.graph().clone(),
                )
            });
        let default_space = self.space().clone();
        let target_validator = dock_target_validator(&default_space, &payload_classes, &policy);
        let excluded_nodes = payload.excluded_nodes_for_drop_scene(&graph);
        let edge_plan_space = default_space.clone();
        let edge_plan_resolver =
            move |target_node: DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
                graph.edge_dock_plan_with_sizing(&edge_plan_space, target_node, zone, sizing)
            };
        let payload_size = self.active_payload_drag_size(payload);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().begin_drop_scene_with_validator(
                DockHostDropScene::new(position)
                    .with_payload_size(payload_size)
                    .with_drop_guide_style(drop_guide_style)
                    .excluding_nodes(excluded_nodes),
                &policy,
                Some(&target_validator),
                Some(&edge_plan_resolver),
            ),
        )
    }

    pub(crate) fn update_drop_scene_fact_interaction(
        &mut self,
        payload: &DockDragPayload,
        fact: DockHostDropSceneFact,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let position = position.into();
        let (policy, payload_classes, drop_guide_style, graph) =
            self.with_workspace(cx, |workspace| {
                (
                    workspace.policy().clone(),
                    workspace.payload_dock_classes_for_drag_payload(payload),
                    workspace.options().drop_guide_style,
                    workspace.graph().clone(),
                )
            });
        let default_space = self.space().clone();
        let target_validator = dock_target_validator(&default_space, &payload_classes, &policy);
        let excluded_nodes = payload.excluded_nodes_for_drop_scene(&graph);
        let edge_plan_space = default_space.clone();
        let edge_plan_resolver =
            move |target_node: DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
                graph.edge_dock_plan_with_sizing(&edge_plan_space, target_node, zone, sizing)
            };
        let payload_size = self.active_payload_drag_size(payload);
        let scene_outcome = DockHostInteractionOutcome::from_session_changed(
            self.push_drop_scene_fact_interaction(
                position.layout,
                payload_size,
                drop_guide_style,
                excluded_nodes,
                fact,
                window,
                &policy,
                Some(&target_validator),
                Some(&edge_plan_resolver),
            ),
        );
        let route_outcome = self.update_viewport_drop_route_preview_interaction(
            payload,
            position.window,
            window,
            cx,
        );

        scene_outcome.merge(route_outcome)
    }

    pub(crate) fn update_root_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: impl Into<DockRenderedPointerPosition>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let fact = drop_scene_fact::root(root, bounds);
        self.update_drop_scene_fact_interaction(payload, fact, position, window, cx)
    }

    pub(crate) fn update_empty_space_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: impl Into<DockRenderedPointerPosition>,
        bounds: Bounds<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> DockHostInteractionOutcome {
        let space = self.space().clone();
        let fact = if is_central {
            drop_scene_fact::empty_central_space(space, bounds)
        } else {
            drop_scene_fact::empty_space(space, bounds)
        };
        self.update_drop_scene_fact_interaction(payload, fact, position, window, cx)
    }

    fn push_drop_scene_fact_interaction(
        &mut self,
        position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        drop_guide_style: DockDropGuideStyle,
        excluded_nodes: Vec<DockNodeId>,
        fact: DockHostDropSceneFact,
        window: &Window,
        policy: &DockPolicy,
        target_validator: Option<&crate::drop_target::DockDropTargetValidator<'_>>,
        edge_plan_resolver: Option<&crate::drop_target::DockEdgePlanResolver<'_>>,
    ) -> bool {
        let viewport_runtime = self.viewport_runtime().clone();
        let frame = self.interaction().viewport_host_scene_frame().cloned();
        if let Some(frame) = frame
            && frame.matches_viewport(self.space(), window.window_handle().window_id())
            && let Some(next_frame) =
                viewport_runtime.push_viewport_host_scene_frame_fact(&frame, fact.clone())
        {
            self.interaction_mut()
                .set_viewport_host_scene_frame(Some(next_frame));
        }
        self.interaction_mut().push_drop_scene_fact_with_validator(
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

    fn active_payload_drag_size(&self, payload: &DockDragPayload) -> Option<Size<Pixels>> {
        let drag_session = self.active_payload_drag_session(payload);
        self.active_payload_drag_tear_off_geometry(drag_session.as_ref())
            .map(|geometry| {
                geometry
                    .preferred_size()
                    .unwrap_or_else(|| geometry.source_bounds().size)
            })
    }
}
