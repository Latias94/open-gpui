use crate::{
    AccessibilityAnnouncement, AccessibilityAnnouncementClearReason,
    AccessibilityAnnouncementDropReason, AccessibilityAnnouncementLifecycle,
    AccessibilityTreeScope, AccessibleAction, AnyElement, AnyView, App, AppContext as _, Bounds,
    Context, Element, ElementId, Entity, FocusHandle, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Render, Role,
    StatefulInteractiveElement, StyleRefinement, Styled, TestAppContext, Window, canvas, deferred,
    div, px, size,
};
use accesskit::{ActionRequest, Invalid, Node, NodeId, TreeId, TreeUpdate};
use std::collections::HashSet;

fn node_with_role(update: &TreeUpdate, role: Role) -> (NodeId, &Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == role)
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing {role:?} node"))
}

fn node_with_label<'a>(update: &'a TreeUpdate, label: &str) -> (NodeId, &'a Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("missing node labelled {label:?}"))
}

pub(super) struct AccessibilityScopeElement {
    scope: AccessibilityTreeScope,
    child: Option<AnyElement>,
}

pub(super) fn accessibility_scope(
    scope: AccessibilityTreeScope,
    child: impl IntoElement,
) -> AccessibilityScopeElement {
    AccessibilityScopeElement {
        scope,
        child: Some(child.into_any_element()),
    }
}

impl IntoElement for AccessibilityScopeElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AccessibilityScopeElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self
            .child
            .take()
            .expect("accessibility scope child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_accessibility_tree_scope(self.scope, |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

fn assert_accessibility_tree_is_normalized_and_closed(update: &TreeUpdate) {
    assert!(
        update.nodes.windows(2).all(|nodes| nodes[0].0 < nodes[1].0),
        "test-facing accessibility updates must be sorted by node ID"
    );
    let node_ids = update
        .nodes
        .iter()
        .map(|(id, _)| *id)
        .collect::<HashSet<_>>();
    let root = update
        .tree
        .as_ref()
        .expect("full update must include a tree")
        .root;
    assert!(node_ids.contains(&root));
    assert!(node_ids.contains(&update.focus));
    for (node_id, node) in &update.nodes {
        for referenced_id in node
            .children()
            .iter()
            .chain(node.controls())
            .chain(node.labelled_by())
        {
            assert!(
                node_ids.contains(referenced_id),
                "node {node_id:?} contains dangling reference {referenced_id:?}"
            );
        }
    }
}

mod action_lifecycle;
mod announcement_lifecycle;
mod semantics;
mod tree_lifecycle;
mod tree_scope;
