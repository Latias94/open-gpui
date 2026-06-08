use crate::{
    DockAction, DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockWorkspace, panel::DockPanelRenderRegistration,
};
use open_gpui::{AnyView, Bounds, Context, Pixels, Point};
use std::collections::HashMap;

pub(crate) enum DockHostPanelRenderResolution {
    Registered(AnyView),
    Missing { prefix: String, item: DockItemId },
}

/// Read-only docking state captured for one GPUI render pass.
///
/// Render code reads this session instead of repeatedly reaching through `DockHost` into the
/// workspace or controller. UI event callbacks still commit through the live owner when they fire.
#[derive(Debug)]
pub(crate) struct DockHostRenderSession {
    space: DockSpaceId,
    selector_prefix: String,
    root: Option<DockNodeId>,
    nodes: HashMap<DockNodeId, DockNode>,
    floating_containers: Vec<DockFloatingContainer>,
    panels: HashMap<DockItemId, DockPanelRenderRegistration>,
    panel_titles: HashMap<DockItemId, String>,
    empty_message: String,
    missing_panel_prefix: String,
    splitter_handle_size: Pixels,
}

impl DockHostRenderSession {
    fn new(space: DockSpaceId, workspace: &DockWorkspace) -> Self {
        let mut session = Self {
            selector_prefix: format!("dock:{space}"),
            root: workspace.graph().root(&space),
            floating_containers: workspace.graph().floating_containers(&space).to_vec(),
            nodes: HashMap::new(),
            panels: HashMap::new(),
            panel_titles: HashMap::new(),
            empty_message: workspace.options().empty_message.clone(),
            missing_panel_prefix: workspace.options().missing_panel_prefix.clone(),
            splitter_handle_size: workspace.options().splitter_handle_size,
            space,
        };

        if let Some(root) = session.root {
            session.collect_subtree(workspace, root);
        }
        for container in session.floating_containers.clone() {
            session.collect_subtree(workspace, container.node);
        }

        session
    }

    fn collect_subtree(&mut self, workspace: &DockWorkspace, node_id: DockNodeId) {
        let Some(node) = workspace.graph().node(node_id).cloned() else {
            return;
        };

        match &node {
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_subtree(workspace, *child);
                }
            }
            DockNode::Tabs { items, .. } => {
                for item in items {
                    self.collect_panel(workspace, item);
                }
            }
            DockNode::Floating { child } => {
                self.collect_subtree(workspace, *child);
            }
        }

        self.nodes.insert(node_id, node);
    }

    fn collect_panel(&mut self, workspace: &DockWorkspace, item: &DockItemId) {
        if self.panel_titles.contains_key(item) {
            return;
        }

        let title = if let Some(registration) = workspace.panels().render_registration(item) {
            let title = registration.title().to_string();
            self.panels.insert(item.clone(), registration);
            title
        } else {
            item.to_string()
        };
        self.panel_titles.insert(item.clone(), title);
    }

    pub(crate) fn space(&self) -> &DockSpaceId {
        &self.space
    }

    pub(crate) fn selector_prefix(&self) -> &str {
        &self.selector_prefix
    }

    pub(crate) fn root(&self) -> Option<DockNodeId> {
        self.root
    }

    pub(crate) fn node(&self, node_id: DockNodeId) -> Option<&DockNode> {
        self.nodes.get(&node_id)
    }

    pub(crate) fn floating_containers(&self) -> &[DockFloatingContainer] {
        &self.floating_containers
    }

    pub(crate) fn empty_message(&self) -> &str {
        &self.empty_message
    }

    pub(crate) fn splitter_handle_size(&self) -> Pixels {
        self.splitter_handle_size
    }

    pub(crate) fn panel_title(&self, item: &DockItemId) -> String {
        self.panel_titles
            .get(item)
            .cloned()
            .unwrap_or_else(|| item.to_string())
    }

    pub(crate) fn panel_for_render(
        &self,
        item: &DockItemId,
        cx: &mut Context<DockHost>,
    ) -> DockHostPanelRenderResolution {
        self.panels
            .get(item)
            .map(|panel| DockHostPanelRenderResolution::Registered(panel.resolve_view(cx)))
            .unwrap_or_else(|| DockHostPanelRenderResolution::Missing {
                prefix: self.missing_panel_prefix.clone(),
                item: item.clone(),
            })
    }
}

impl DockHost {
    pub(crate) fn render_session(&self, cx: &Context<Self>) -> DockHostRenderSession {
        let space = self.space().clone();
        self.with_workspace(cx, |workspace| DockHostRenderSession::new(space, workspace))
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
        if !self.with_workspace(cx, |workspace| workspace.policy().allows_floating()) {
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
