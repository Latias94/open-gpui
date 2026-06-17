use crate::{
    DockFloatingContainer, DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId, DockWorkspace,
    panel_registry::DockPanelRenderRegistration,
};
use open_gpui::{AnyView, Context, Pixels, Window};
use std::collections::HashMap;

pub(crate) enum DockHostPanelRenderResolution {
    Registered(AnyView),
    Missing { prefix: String, item: DockItemId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockFloatingChromeTarget {
    SingleTabs(DockNodeId),
    AmbiguousSplit,
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
    panel_closable: HashMap<DockItemId, bool>,
    selected_tabs: Vec<(DockNodeId, DockItemId)>,
    central_node: Option<DockNodeId>,
    central_keep_alive_when_empty: bool,
    central_passthrough_when_empty: bool,
    empty_message: String,
    missing_panel_prefix: String,
    splitter_handle_size: Pixels,
}

impl DockHostRenderSession {
    fn new(space: DockSpaceId, workspace: &DockWorkspace) -> Self {
        let central = workspace.graph().central_region(&space);
        let mut session = Self {
            selector_prefix: format!("dock:{space}"),
            root: workspace.graph().root(&space),
            floating_containers: workspace.graph().floating_containers(&space).to_vec(),
            nodes: HashMap::new(),
            panels: HashMap::new(),
            panel_titles: HashMap::new(),
            panel_closable: HashMap::new(),
            selected_tabs: Vec::new(),
            central_node: central.and_then(|central| central.node),
            central_keep_alive_when_empty: central
                .is_some_and(|central| central.keep_alive_when_empty),
            central_passthrough_when_empty: central
                .is_some_and(|central| central.passthrough_when_empty),
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
            DockNode::Tabs { items, selected } => {
                self.collect_tab_stack(workspace, node_id, items, selected);
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
        tabs: DockNodeId,
        items: &[DockItemId],
        selected: &Option<DockItemId>,
    ) {
        for item in items {
            self.collect_panel_metadata(workspace, item);
        }

        if let Some(selected_item) = selected_tab_item(items, selected) {
            self.selected_tabs.push((tabs, selected_item.clone()));
            self.collect_panel_registration(workspace, selected_item);
        }
    }

    fn collect_panel_metadata(&mut self, workspace: &DockWorkspace, item: &DockItemId) {
        if self.panel_titles.contains_key(item) {
            return;
        }

        let descriptor = workspace.panels().catalog().descriptor(item);
        let title = descriptor
            .map(|descriptor| descriptor.title().to_string())
            .unwrap_or_else(|| item.to_string());
        let closable = descriptor.is_some_and(|descriptor| descriptor.is_closable());
        self.panel_titles.insert(item.clone(), title);
        self.panel_closable.insert(item.clone(), closable);
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

    pub(crate) fn selected_tabs(&self) -> &[(DockNodeId, DockItemId)] {
        &self.selected_tabs
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
            Some(DockNode::Tabs { items, selected }) => selected_tab_item(items, selected)
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

    pub(crate) fn floating_chrome_target(
        &self,
        node_id: DockNodeId,
    ) -> Option<DockFloatingChromeTarget> {
        match self.node(node_id)? {
            DockNode::Floating { child } => match self.node(*child)? {
                DockNode::Tabs { .. } => Some(DockFloatingChromeTarget::SingleTabs(*child)),
                DockNode::Split { .. } | DockNode::Floating { .. } => {
                    Some(DockFloatingChromeTarget::AmbiguousSplit)
                }
            },
            DockNode::Tabs { .. } => Some(DockFloatingChromeTarget::SingleTabs(node_id)),
            DockNode::Split { .. } => Some(DockFloatingChromeTarget::AmbiguousSplit),
        }
    }

    pub(crate) fn floating_containers(&self) -> &[DockFloatingContainer] {
        &self.floating_containers
    }

    pub(crate) fn visible_panel_items(&self) -> Vec<DockItemId> {
        let mut items = Vec::new();
        if let Some(root) = self.root {
            self.collect_visible_panel_items_in_subtree(root, &mut items);
        }
        for container in &self.floating_containers {
            self.collect_visible_panel_items_in_subtree(container.node, &mut items);
        }
        items
    }

    pub(crate) fn empty_message(&self) -> &str {
        &self.empty_message
    }

    pub(crate) fn splitter_handle_size(&self) -> Pixels {
        self.splitter_handle_size
    }

    pub(crate) fn is_central_tabs(&self, node_id: DockNodeId) -> bool {
        self.central_node
            .is_some_and(|central| self.subtree_contains(central, node_id))
    }

    pub(crate) fn drop_root_for_tabs(&self, tabs: DockNodeId) -> Option<DockNodeId> {
        if let Some(root) = self.root
            && self.subtree_contains(root, tabs)
        {
            return Some(root);
        }

        self.floating_containers.iter().find_map(|container| {
            self.subtree_contains(container.node, tabs)
                .then_some(container.node)
        })
    }

    pub(crate) fn central_child_index(&self, children: &[DockNodeId]) -> Option<usize> {
        let central = self.central_node?;
        children
            .iter()
            .position(|child| self.subtree_contains(*child, central))
    }

    pub(crate) fn has_empty_central_region(&self) -> bool {
        self.root.is_none() && self.central_node.is_none() && self.central_keep_alive_when_empty
    }

    pub(crate) fn empty_central_passthrough(&self) -> bool {
        self.has_empty_central_region() && self.central_passthrough_when_empty
    }

    pub(crate) fn panel_title(&self, item: &DockItemId) -> String {
        self.panel_titles
            .get(item)
            .cloned()
            .unwrap_or_else(|| item.to_string())
    }

    pub(crate) fn panel_is_closable(&self, item: &DockItemId) -> bool {
        self.panel_closable.get(item).copied().unwrap_or(false)
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

    pub(crate) fn request_panel_focus(
        &self,
        item: &DockItemId,
        window: &mut Window,
        cx: &mut Context<DockHost>,
    ) -> bool {
        self.panels
            .get(item)
            .is_some_and(|panel| panel.request_focus(window, cx))
    }

    fn subtree_contains(&self, root: DockNodeId, target: DockNodeId) -> bool {
        if root == target {
            return true;
        }
        match self.node(root) {
            Some(DockNode::Split { children, .. }) => children
                .iter()
                .copied()
                .any(|child| self.subtree_contains(child, target)),
            Some(DockNode::Floating { child }) => self.subtree_contains(*child, target),
            Some(DockNode::Tabs { .. }) | None => false,
        }
    }

    fn collect_visible_panel_items_in_subtree(
        &self,
        node_id: DockNodeId,
        items: &mut Vec<DockItemId>,
    ) {
        let Some(node) = self.node(node_id) else {
            return;
        };

        match node {
            DockNode::Tabs {
                items: tabs,
                selected,
            } => {
                if let Some(item) = selected_tab_item(tabs, selected) {
                    items.push(item.clone());
                }
            }
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_visible_panel_items_in_subtree(*child, items);
                }
            }
            DockNode::Floating { child } => {
                self.collect_visible_panel_items_in_subtree(*child, items);
            }
        }
    }
}

impl DockHost {
    pub(crate) fn render_session(&self, cx: &Context<Self>) -> DockHostRenderSession {
        let space = self.space().clone();
        self.with_workspace(cx, |workspace| DockHostRenderSession::new(space, workspace))
    }
}

pub(crate) fn selected_index(items: &[DockItemId], selected: &Option<DockItemId>) -> Option<usize> {
    selected
        .as_ref()
        .and_then(|selected| items.iter().position(|item| item == selected))
}

fn selected_tab_item<'a>(
    items: &'a [DockItemId],
    selected: &Option<DockItemId>,
) -> Option<&'a DockItemId> {
    selected_index(items, selected).and_then(|selected| items.get(selected))
}

fn default_floating_title() -> String {
    "Floating".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockCentralRegion, DockGraph,
        host_test_support::{floating_overlay_graph, item, space, tabs_graph},
    };

    #[test]
    fn render_session_keeps_inactive_panel_views_out_of_snapshot() {
        let (graph, _root) = tabs_graph(&["selected", "inactive"]);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.register_panel_factory("selected", "Selected", |_| unreachable!());
        workspace.register_panel_factory("inactive", "Inactive", |_| unreachable!());

        let session = DockHostRenderSession::new(space(), &workspace);

        assert_eq!(session.panel_title(&item("selected")), "Selected");
        assert_eq!(session.panel_title(&item("inactive")), "Inactive");
        assert!(session.panels.contains_key(&item("selected")));
        assert!(!session.panels.contains_key(&item("inactive")));
    }

    #[test]
    fn render_session_does_not_repair_invalid_tab_selection() {
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b")],
            selected: Some(item("missing")),
        });
        graph.set_root(space(), root);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.register_panel_factory("a", "A", |_| unreachable!());
        workspace.register_panel_factory("b", "B", |_| unreachable!());

        let session = DockHostRenderSession::new(space(), &workspace);

        assert_eq!(session.selected_tabs(), &[]);
        assert!(!session.panels.contains_key(&item("a")));
        assert!(!session.panels.contains_key(&item("b")));
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
        assert_eq!(
            session.floating_chrome_target(floating),
            session
                .floating_child(floating)
                .map(DockFloatingChromeTarget::SingleTabs)
        );
    }

    #[test]
    fn render_session_marks_split_floating_chrome_as_ambiguous() {
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![item("root")],
            selected: Some(item("root")),
        });
        let orphan = graph.insert_node(DockNode::Tabs {
            items: vec![item("orphan")],
            selected: Some(item("orphan")),
        });
        graph.set_root(space(), root);
        let left = graph.insert_node(DockNode::Tabs {
            items: vec![item("left")],
            selected: Some(item("left")),
        });
        let right = graph.insert_node(DockNode::Tabs {
            items: vec![item("right")],
            selected: Some(item("right")),
        });
        let split = graph.insert_node(DockNode::Split {
            axis: crate::SplitAxis::Horizontal,
            children: vec![left, right],
            fractions: vec![0.5, 0.5],
        });
        let floating = graph.insert_node(DockNode::Floating { child: split });
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: open_gpui::Bounds::new(
                    open_gpui::point(open_gpui::px(0.0), open_gpui::px(0.0)),
                    open_gpui::size(open_gpui::px(320.0), open_gpui::px(200.0)),
                ),
            });
        let workspace = DockWorkspace::new(space(), graph);

        let session = DockHostRenderSession::new(space(), &workspace);

        assert_eq!(
            session.floating_chrome_target(floating),
            Some(DockFloatingChromeTarget::AmbiguousSplit)
        );
        assert_eq!(session.drop_root_for_tabs(left), Some(floating));
        assert_eq!(session.drop_root_for_tabs(right), Some(floating));
        assert_eq!(session.drop_root_for_tabs(root), Some(root));
        assert_eq!(session.drop_root_for_tabs(orphan), None);
    }

    #[test]
    fn render_session_exposes_empty_central_passthrough_semantics() {
        let mut graph = DockGraph::new();
        graph.set_central_region(
            space(),
            DockCentralRegion::empty().with_passthrough_when_empty(true),
        );
        let workspace = DockWorkspace::new(space(), graph);

        let session = DockHostRenderSession::new(space(), &workspace);

        assert!(session.has_empty_central_region());
        assert!(session.empty_central_passthrough());
    }
}
