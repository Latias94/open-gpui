use crate::{
    DockHost, DockNodeId, DockPolicy, DockSpaceId,
    drag::DockDragPayload,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_scene_fact,
    drop_target::{DockResolvedDropTarget, DockResolvedDropTargetKind},
    host_interaction_outcome::DockHostInteractionOutcome,
    workspace_move_validation::DockPayloadDockClasses,
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
        let target_validator =
            local_drop_target_validator(&default_space, &payload_classes, &policy);
        DockHostInteractionOutcome::from_session_changed(
            self.interaction_mut().begin_drop_scene_with_validator(
                DockHostDropScene::new(position)
                    .excluding_tabs(payload.excluded_tabs_for_drop_scene()),
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
        let target_validator =
            local_drop_target_validator(&default_space, &payload_classes, &policy);
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
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
        window: &Window,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let space = self.space().clone();
        let fact = drop_scene_fact::empty_space(space, bounds);
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
        let viewport_runtime = self.viewport_runtime().cloned();
        let frame = self.interaction().viewport_host_scene_frame().cloned();
        if let (Some(runtime), Some(frame)) = (viewport_runtime, frame)
            && frame.matches_viewport(self.space(), window.window_handle().window_id())
        {
            runtime.push_viewport_host_scene_frame_fact(&frame, fact.clone());
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

fn local_drop_target_validator<'a>(
    default_space: &'a DockSpaceId,
    payload_classes: &'a DockPayloadDockClasses,
    policy: &'a DockPolicy,
) -> impl Fn(&DockResolvedDropTarget) -> Result<(), crate::DockPolicyError> + 'a {
    move |target| {
        payload_classes.validate_target_space(local_target_space(default_space, target), policy)
    }
}

fn local_target_space<'a>(
    default_space: &'a DockSpaceId,
    target: &'a DockResolvedDropTarget,
) -> &'a DockSpaceId {
    match &target.kind {
        DockResolvedDropTargetKind::EmptyDockSpace { space } => space,
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::RootEdge { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::KnownViewport { .. }
        | DockResolvedDropTargetKind::TearOffCandidate { .. } => default_space,
    }
}
