use crate::{
    DockFloatingContainer, DockGraph, DockHost, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockVisualStyle, DockWorkspace, geometry::DockDropGuideMetrics, host::DockHostOptions,
    panel_registry::DockPanelRenderRegistration,
};
use open_gpui::{AnyView, Context, EntityId, Pixels};
use open_gpui_motion::MotionPreference;
use std::{collections::HashMap, ops::Deref, rc::Rc};

pub(crate) enum DockHostPanelRenderResolution {
    Registered(AnyView),
    Missing { prefix: String, item: DockItemId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockFloatingChromeTarget {
    SingleTabs(DockNodeId),
    AmbiguousSplit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockHostPresentationKind {
    Workspace,
    ProvisionalShell,
    LivePayloadProjection,
    PayloadRecoveryProjection,
}

/// Read-only docking state captured for one GPUI render pass.
///
/// Render code reads this session instead of repeatedly reaching through `DockHost` into the
/// workspace or controller. UI event callbacks still commit through the live owner when they fire.
#[derive(Debug, Clone)]
pub(crate) struct DockHostPresentationSession {
    kind: DockHostPresentationKind,
    space: DockSpaceId,
    selector_prefix: String,
    root: Option<DockNodeId>,
    nodes: HashMap<DockNodeId, DockNode>,
    floating_containers: Vec<DockFloatingContainer>,
    visible_panel_items: Vec<DockItemId>,
    panels: HashMap<DockItemId, DockPanelRenderRegistration>,
    panel_titles: HashMap<DockItemId, String>,
    panel_closable: HashMap<DockItemId, bool>,
    central_node: Option<DockNodeId>,
    central_keep_alive_when_empty: bool,
    central_passthrough_when_empty: bool,
    empty_message: String,
    missing_panel_prefix: String,
    splitter_handle_size: Pixels,
    drop_guide_metrics: DockDropGuideMetrics,
    motion_preference: MotionPreference,
}

impl DockHostPresentationSession {
    fn new(space: DockSpaceId, workspace: &DockWorkspace) -> Self {
        Self::from_graph(space, workspace.graph(), workspace)
    }

    pub(crate) fn from_graph(
        space: DockSpaceId,
        graph: &DockGraph,
        workspace: &DockWorkspace,
    ) -> Self {
        Self::from_graph_with_kind(space, graph, workspace, DockHostPresentationKind::Workspace)
    }

    pub(crate) fn live_payload_projection(
        space: DockSpaceId,
        graph: &DockGraph,
        workspace: &DockWorkspace,
    ) -> Self {
        Self::from_graph_with_kind(
            space,
            graph,
            workspace,
            DockHostPresentationKind::LivePayloadProjection,
        )
    }

    pub(crate) fn payload_recovery_projection(
        space: DockSpaceId,
        graph: &DockGraph,
        workspace: &DockWorkspace,
    ) -> Self {
        Self::from_graph_with_kind(
            space,
            graph,
            workspace,
            DockHostPresentationKind::PayloadRecoveryProjection,
        )
    }

    fn from_graph_with_kind(
        space: DockSpaceId,
        graph: &DockGraph,
        workspace: &DockWorkspace,
        kind: DockHostPresentationKind,
    ) -> Self {
        let central = graph.central_region(&space);
        let mut session = Self {
            kind,
            selector_prefix: format!("dock:{space}"),
            root: graph.root(&space),
            floating_containers: graph.floating_containers(&space).to_vec(),
            visible_panel_items: Vec::new(),
            nodes: HashMap::new(),
            panels: HashMap::new(),
            panel_titles: HashMap::new(),
            panel_closable: HashMap::new(),
            central_node: central.and_then(|central| central.node),
            central_keep_alive_when_empty: central
                .is_some_and(|central| central.keep_alive_when_empty),
            central_passthrough_when_empty: central
                .is_some_and(|central| central.passthrough_when_empty),
            empty_message: workspace.options().empty_message.clone(),
            missing_panel_prefix: workspace.options().missing_panel_prefix.clone(),
            splitter_handle_size: workspace.options().splitter_handle_size,
            drop_guide_metrics: workspace.options().drop_guide_metrics,
            motion_preference: workspace.options().motion_preference,
            space,
        };

        if let Some(root) = session.root {
            session.collect_subtree(graph, workspace, root);
        }
        for container in session.floating_containers.clone() {
            session.collect_subtree(graph, workspace, container.node);
        }

        session
    }

    fn provisional_shell(space: DockSpaceId) -> Self {
        let options = DockHostOptions::default();
        Self {
            kind: DockHostPresentationKind::ProvisionalShell,
            selector_prefix: format!("dock:{space}:provisional"),
            root: None,
            floating_containers: Vec::new(),
            visible_panel_items: Vec::new(),
            nodes: HashMap::new(),
            panels: HashMap::new(),
            panel_titles: HashMap::new(),
            panel_closable: HashMap::new(),
            central_node: None,
            central_keep_alive_when_empty: false,
            central_passthrough_when_empty: false,
            empty_message: options.empty_message,
            missing_panel_prefix: options.missing_panel_prefix,
            splitter_handle_size: options.splitter_handle_size,
            drop_guide_metrics: options.drop_guide_metrics,
            motion_preference: options.motion_preference,
            space,
        }
    }

    pub(crate) const fn kind(&self) -> DockHostPresentationKind {
        self.kind
    }

    pub(crate) const fn is_provisional_shell(&self) -> bool {
        matches!(self.kind, DockHostPresentationKind::ProvisionalShell)
    }

    fn collect_subtree(
        &mut self,
        graph: &DockGraph,
        workspace: &DockWorkspace,
        node_id: DockNodeId,
    ) {
        let Some(node) = graph.node(node_id).cloned() else {
            return;
        };

        match &node {
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_subtree(graph, workspace, *child);
                }
            }
            DockNode::Tabs { items, selected } => {
                self.collect_tab_stack(workspace, items, selected)
            }
            DockNode::Floating { child } => {
                self.collect_subtree(graph, workspace, *child);
            }
        }

        self.nodes.insert(node_id, node);
    }

    fn collect_tab_stack(
        &mut self,
        workspace: &DockWorkspace,
        items: &[DockItemId],
        selected: &Option<DockItemId>,
    ) {
        for item in items {
            self.collect_panel_metadata(workspace, item);
        }

        if let Some(selected_item) = selected_tab_item(items, selected) {
            self.visible_panel_items.push(selected_item.clone());
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
            Some(DockNode::Split { .. }) => default_floating_title(),
            Some(DockNode::Floating { child }) => self.floating_title(*child),
            None => default_floating_title(),
        }
    }

    pub(crate) fn preview_tab_titles_for_node(&self, node_id: DockNodeId) -> Vec<String> {
        match self.node(node_id) {
            Some(DockNode::Tabs { items, .. }) => {
                items.iter().map(|item| self.panel_title(item)).collect()
            }
            Some(DockNode::Floating { child }) => self.preview_tab_titles_for_node(*child),
            Some(DockNode::Split { .. }) | None => Vec::new(),
        }
    }

    pub(crate) fn multi_preview_tab_titles_for_node(
        &self,
        node_id: DockNodeId,
    ) -> Option<Vec<String>> {
        let titles = self.preview_tab_titles_for_node(node_id);
        (titles.len() > 1).then_some(titles)
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

    pub(crate) fn visible_panel_items(&self) -> &[DockItemId] {
        &self.visible_panel_items
    }

    pub(crate) fn resolved_visible_panel_entity_ids(&self) -> Vec<EntityId> {
        self.visible_panel_items
            .iter()
            .filter_map(|item| self.panels.get(item))
            .filter_map(DockPanelRenderRegistration::resolved_view)
            .map(|view| view.entity_id())
            .collect()
    }

    pub(crate) fn empty_message(&self) -> &str {
        &self.empty_message
    }

    pub(crate) fn splitter_handle_size(&self) -> Pixels {
        self.splitter_handle_size
    }

    pub(crate) fn drop_guide_metrics(&self) -> DockDropGuideMetrics {
        self.drop_guide_metrics
    }

    pub(crate) fn motion_preference(&self) -> MotionPreference {
        self.motion_preference
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

    pub(crate) fn empty_central_requests_platform_pointer_passthrough(&self) -> bool {
        self.empty_central_passthrough() && self.floating_containers.is_empty()
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

    pub(crate) fn visible_panel_registration(
        &self,
        item: &DockItemId,
    ) -> Option<DockPanelRenderRegistration> {
        if !self.visible_panel_items.contains(item) {
            return None;
        }
        self.panels.get(item).cloned()
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
}

/// Paint-only extension of a structural presentation snapshot.
#[derive(Debug, Clone)]
pub(crate) struct DockHostRenderSession {
    presentation: DockHostPresentationSession,
    visual_style: Rc<DockVisualStyle>,
}

impl DockHostRenderSession {
    fn new(presentation: DockHostPresentationSession, visual_style: Rc<DockVisualStyle>) -> Self {
        Self {
            presentation,
            visual_style,
        }
    }

    pub(crate) fn visual_style(&self) -> &DockVisualStyle {
        &self.visual_style
    }
}

impl Deref for DockHostRenderSession {
    type Target = DockHostPresentationSession;

    fn deref(&self) -> &Self::Target {
        &self.presentation
    }
}

impl DockHost {
    pub(crate) fn presentation_session(&self, cx: &Context<Self>) -> DockHostPresentationSession {
        if let Some(presentation) = self.live_presentation_session() {
            return presentation.clone();
        }
        let space = self.space().clone();
        if self.is_provisional_viewport() {
            return DockHostPresentationSession::provisional_shell(space);
        }
        self.with_workspace(cx, |workspace| {
            DockHostPresentationSession::new(space, workspace)
        })
    }

    pub(crate) fn render_session_with_visual_style(
        &self,
        visual_style: Rc<DockVisualStyle>,
        cx: &Context<Self>,
    ) -> DockHostRenderSession {
        DockHostRenderSession::new(self.presentation_session(cx), visual_style)
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

        let session = DockHostPresentationSession::new(space(), &workspace);

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

        let session = DockHostPresentationSession::new(space(), &workspace);

        assert!(!session.panels.contains_key(&item("a")));
        assert!(!session.panels.contains_key(&item("b")));
    }

    #[test]
    fn render_session_resolves_floating_title_from_snapshot() {
        let (graph, _root, floating) = floating_overlay_graph();
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.register_panel_factory("a", "Floating A", |_| unreachable!());
        workspace.register_panel_factory("b", "Root B", |_| unreachable!());

        let session = DockHostPresentationSession::new(space(), &workspace);

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

        let session = DockHostPresentationSession::new(space(), &workspace);

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

        let session = DockHostPresentationSession::new(space(), &workspace);

        assert!(session.has_empty_central_region());
        assert!(session.empty_central_passthrough());
    }
}
