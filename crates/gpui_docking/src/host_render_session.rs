use crate::{
    DockAction, DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId,
    DockPanelResolution, DockSpaceId,
};
use open_gpui::{AnyView, Bounds, Context, Pixels, Point};

pub(crate) enum DockHostPanelRenderResolution {
    Registered(AnyView),
    Missing { prefix: String, item: DockItemId },
}

impl DockHost {
    pub(crate) fn root_for_render(&self, cx: &Context<Self>) -> Option<DockNodeId> {
        self.with_workspace(cx, |workspace| workspace.graph().root(self.space()))
    }

    pub(crate) fn node_for_render(
        &self,
        node_id: DockNodeId,
        cx: &Context<Self>,
    ) -> Option<DockNode> {
        self.with_workspace(cx, |workspace| workspace.graph().node(node_id).cloned())
    }

    pub(crate) fn floating_containers_for_render(
        &self,
        cx: &Context<Self>,
    ) -> Vec<DockFloatingContainer> {
        self.with_workspace(cx, |workspace| {
            workspace.graph().floating_containers(self.space()).to_vec()
        })
    }

    pub(crate) fn empty_message_for_render(&self, cx: &Context<Self>) -> String {
        self.with_workspace(cx, |workspace| workspace.options().empty_message.clone())
    }

    pub(crate) fn missing_panel_prefix_for_render(&self, cx: &Context<Self>) -> String {
        self.with_workspace(cx, |workspace| {
            workspace.options().missing_panel_prefix.clone()
        })
    }

    pub(crate) fn split_min_size_for_render(&self, cx: &Context<Self>) -> Pixels {
        self.with_workspace(cx, |workspace| workspace.options().split_min_size)
    }

    pub(crate) fn splitter_handle_size_for_render(&self, cx: &Context<Self>) -> Pixels {
        self.with_workspace(cx, |workspace| workspace.options().splitter_handle_size)
    }

    pub(crate) fn allows_floating_for_render(&self, cx: &Context<Self>) -> bool {
        self.with_workspace(cx, |workspace| workspace.policy().allows_floating())
    }

    pub(crate) fn panel_title_for_render(&self, item: &DockItemId, cx: &Context<Self>) -> String {
        self.with_workspace(cx, |workspace| match workspace.panels().resolve(item) {
            DockPanelResolution::Registered(panel) => panel.title().to_string(),
            DockPanelResolution::Missing { item } => item.to_string(),
        })
    }

    pub(crate) fn panel_for_render(
        &self,
        item: &DockItemId,
        cx: &mut Context<Self>,
    ) -> DockHostPanelRenderResolution {
        if let Some(panel) =
            self.with_workspace(cx, |workspace| workspace.panels().get(item).cloned())
        {
            DockHostPanelRenderResolution::Registered(panel.resolve_view(cx))
        } else {
            DockHostPanelRenderResolution::Missing {
                prefix: self.missing_panel_prefix_for_render(cx),
                item: item.clone(),
            }
        }
    }

    pub(crate) fn select_tab_from_render(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        cx: &mut Context<Self>,
    ) -> bool {
        self.commit_action_from_render(DockAction::SelectTab { tabs, item }, cx)
    }

    pub(crate) fn drop_tab_from_render(
        &mut self,
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        target_space: DockSpaceId,
        target_tabs: DockNodeId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(intent) = self.take_tab_drop_intent(target_tabs) else {
            cx.notify();
            return false;
        };

        self.commit_action_from_render(
            DockAction::MoveTab {
                source_space,
                source_tabs,
                item,
                target_space,
                target_tabs: intent.target_tabs,
                zone: intent.zone,
                insert_index: intent.insert_index,
            },
            cx,
        )
    }

    pub(crate) fn begin_floating_drag_from_render(
        &mut self,
        space: DockSpaceId,
        floating: DockNodeId,
        start_position: Point<Pixels>,
        initial_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.allows_floating_for_render(cx) {
            return false;
        }

        self.start_floating_drag(space.clone(), floating, start_position, initial_bounds);
        let changed =
            self.commit_action_from_render(DockAction::RaiseFloating { space, floating }, cx);
        if !changed {
            cx.notify();
        }
        true
    }

    pub(crate) fn update_floating_drag_from_render(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.update_floating_drag(position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn finish_floating_drag_from_render(&mut self, cx: &mut Context<Self>) {
        self.finish_floating_drag();
        cx.notify();
    }

    pub(crate) fn begin_splitter_drag_from_render(
        &mut self,
        split: DockNodeId,
        handle_index: usize,
        start_position: Pixels,
        split_extent: Pixels,
        initial_fractions: Vec<f32>,
        cx: &mut Context<Self>,
    ) {
        self.start_splitter_drag(
            split,
            handle_index,
            start_position,
            split_extent,
            initial_fractions,
        );
        cx.notify();
    }

    pub(crate) fn update_splitter_drag_from_render(
        &mut self,
        position: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        let changed = self.update_splitter_drag(position, cx);
        if changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn finish_splitter_drag_from_render(&mut self, cx: &mut Context<Self>) {
        self.finish_splitter_drag();
        cx.notify();
    }

    fn commit_action_from_render(&mut self, action: DockAction, cx: &mut Context<Self>) -> bool {
        let Ok(outcome) = self.apply_action_from_host(&action, cx) else {
            return false;
        };
        if outcome.changed() {
            cx.notify();
            true
        } else {
            false
        }
    }
}
