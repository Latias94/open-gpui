use crate::{
    DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId, DockWorkspace,
    panel::DockPanelRenderRegistration,
};
use open_gpui::{AnyView, Context, Pixels};
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
            DockNode::Tabs { items, active } => {
                self.collect_tab_stack(workspace, items, *active);
            }
            DockNode::Floating { child } => {
                self.collect_subtree(workspace, *child);
            }
        }

        self.nodes.insert(node_id, node);
    }

    fn collect_tab_stack(
        &mut self,
        workspace: &DockWorkspace,
        items: &[DockItemId],
        active: usize,
    ) {
        for item in items {
            self.collect_panel_title(workspace, item);
        }

        if let Some(active_item) = active_tab_item(items, active) {
            self.collect_panel_registration(workspace, active_item);
        }
    }

    fn collect_panel_title(&mut self, workspace: &DockWorkspace, item: &DockItemId) {
        if self.panel_titles.contains_key(item) {
            return;
        }

        let title = workspace
            .panels()
            .catalog()
            .descriptor(item)
            .map(|descriptor| descriptor.title().to_string())
            .unwrap_or_else(|| item.to_string());
        self.panel_titles.insert(item.clone(), title);
    }

    fn collect_panel_registration(&mut self, workspace: &DockWorkspace, item: &DockItemId) {
        if self.panels.contains_key(item) {
            return;
        }

        if let Some(registration) = workspace.panels().render_registration(item) {
            self.panels.insert(item.clone(), registration);
        }
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

    pub(crate) fn floating_child(&self, node_id: DockNodeId) -> Option<DockNodeId> {
        match self.node(node_id)? {
            DockNode::Floating { child } => Some(*child),
            _ => None,
        }
    }

    pub(crate) fn floating_title(&self, node_id: DockNodeId) -> String {
        match self.node(node_id) {
            Some(DockNode::Tabs { items, active }) => active_tab_item(items, *active)
                .map(|item| self.panel_title(item))
                .unwrap_or_else(default_floating_title),
            Some(DockNode::Split { children, .. }) => children
                .first()
                .map(|child| self.floating_title(*child))
                .unwrap_or_else(default_floating_title),
            Some(DockNode::Floating { child }) => self.floating_title(*child),
            None => default_floating_title(),
        }
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
}

fn active_tab_item(items: &[DockItemId], active: usize) -> Option<&DockItemId> {
    let active = active.min(items.len().checked_sub(1)?);
    items.get(active)
}

fn default_floating_title() -> String {
    "Floating".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_test_support::{floating_overlay_graph, item, space, tabs_graph};

    #[test]
    fn render_session_keeps_inactive_panel_views_out_of_snapshot() {
        let (graph, _root) = tabs_graph(&["active", "inactive"], 0);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.register_panel_factory("active", "Active", |_| unreachable!());
        workspace.register_panel_factory("inactive", "Inactive", |_| unreachable!());

        let session = DockHostRenderSession::new(space(), &workspace);

        assert_eq!(session.panel_title(&item("active")), "Active");
        assert_eq!(session.panel_title(&item("inactive")), "Inactive");
        assert!(session.panels.contains_key(&item("active")));
        assert!(!session.panels.contains_key(&item("inactive")));
    }

    #[test]
    fn render_session_resolves_floating_title_from_snapshot() {
        let (graph, _root, floating) = floating_overlay_graph();
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.register_panel_factory("a", "Floating A", |_| unreachable!());
        workspace.register_panel_factory("b", "Root B", |_| unreachable!());

        let session = DockHostRenderSession::new(space(), &workspace);

        assert!(session.floating_child(floating).is_some());
        assert_eq!(session.floating_title(floating), "Floating A");
    }
}
