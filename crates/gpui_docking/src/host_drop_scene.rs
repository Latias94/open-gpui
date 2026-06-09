use crate::{
    DockHost, DockNodeId,
    drag::DockDragPayload,
    drop_runtime::{DockHostDropScene, DockHostDropSceneFact},
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabLabelDropTarget,
    },
    host_interactions::DockHostInteractionOutcome,
};
use open_gpui::{Bounds, Context, Pixels, Point};

impl DockHost {
    pub(crate) fn begin_host_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        DockHostInteractionOutcome::from_session_changed(self.interaction_mut().begin_drop_scene(
            DockHostDropScene::new(position).excluding_tabs(payload.excluded_tabs_for_drop_scene()),
            &policy,
        ))
    }

    pub(crate) fn update_tabs_drop_interaction(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = DockHostDropSceneFact::Leaf(DockLeafDropTarget {
            root: target_tabs,
            target_tabs,
            bounds,
            is_central,
        });
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            &policy,
        ))
    }

    pub(crate) fn update_tab_reorder_drop_interaction(
        &mut self,
        payload: &DockDragPayload,
        target_tabs: DockNodeId,
        target_index: usize,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        is_central: bool,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
            target_tabs,
            target_index,
            bounds,
            is_central,
        });
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            &policy,
        ))
    }

    pub(crate) fn update_root_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = DockHostDropSceneFact::Root(DockRootDropTarget { root, bounds });
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            &policy,
        ))
    }

    pub(crate) fn update_empty_space_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let space = self.space().clone();
        let fact = DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget { space, bounds });
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            &policy,
        ))
    }

    pub(crate) fn update_floating_title_bar_drop_scene_interaction(
        &mut self,
        payload: &DockDragPayload,
        floating: DockNodeId,
        target_tabs: DockNodeId,
        title_bounds: Bounds<Pixels>,
        preview_bounds: Bounds<Pixels>,
        position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> DockHostInteractionOutcome {
        let policy = self.with_workspace(cx, |workspace| *workspace.policy());
        let fact = DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
            floating,
            target_tabs,
            title_bounds,
            preview_bounds,
        });
        DockHostInteractionOutcome::from_session_changed(self.push_drop_scene_fact_interaction(
            position,
            payload.excluded_tabs_for_drop_scene(),
            fact,
            &policy,
        ))
    }

    fn push_drop_scene_fact_interaction(
        &mut self,
        position: Point<Pixels>,
        excluded_tabs: Option<DockNodeId>,
        fact: DockHostDropSceneFact,
        policy: &crate::DockPolicy,
    ) -> bool {
        if let Some(runtime) = self.viewport_runtime()
            && let Some(window_id) = self.viewport_scene_window()
        {
            runtime.push_viewport_host_scene_fact(self.space(), window_id, fact.clone());
        }
        self.interaction_mut()
            .push_drop_scene_fact(position, excluded_tabs, fact, policy)
    }
}
