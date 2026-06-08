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
}
