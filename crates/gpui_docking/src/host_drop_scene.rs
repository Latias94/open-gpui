use crate::{
    DockHost, DockNodeId, DockPolicy,
    drag::DockDragPayload,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_scene_fact,
    host_interaction_outcome::DockHostInteractionOutcome,
    workspace_move_validation::dock_target_validator,
};
use open_gpui::{Bounds, Context, Pixels, Point, Window};

impl DockHost {
    pub(crate) fn begin_host_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let (policy, payload_classes) = self.with_workspace(cx, |workspace| {
            (
                workspace.policy().clone(),
                workspace.payload_dock_classes_for_drag_payload(payload),
            )
        });
        let default_space = self.space().clone();
        let target_validator = dock_target_validator(&default_space, &payload_classes, &policy);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().begin_drop_scene_with_validator(
                DockHostDropScene::new(position)
                    .excluding_node(payload.excluded_node_for_drop_scene()),
                &policy,
                Some(&target_validator),
            ),
        )
    }

    pub(crate) fn update_drop_scene_fact_interaction(
        &mut self,
        payload: &DockDragPayload,
        fact: DockHostDropSceneFact,
        position: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let (policy, payload_classes) = self.with_workspace(cx, |workspace| {
            (
                workspace.policy().clone(),
                workspace.payload_dock_classes_for_drag_payload(payload),
            )
        });
        let default_space = self.space().clone();
        let target_validator = dock_target_validator(&default_space, &payload_classes, &policy);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_node_for_drop_scene(),
            fact,
            window,
            &policy,
            Some(&target_validator),
        ))
    }

    pub(crate) fn update_root_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let fact = drop_scene_fact::root(root, bounds);
        self.update_drop_scene_fact_interaction(payload, fact, position, window, cx)
    }

    pub(crate) fn update_empty_space_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        is_central: bool,
        window: &Window,
        cx: &Context<Self>,
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
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        window: &Window,
        policy: &DockPolicy,
        target_validator: Option<&crate::drop_target::DockDropTargetValidator<'_>>,
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
            excluded_tabs,
            fact,
            policy,
            target_validator,
        )
    }
}
