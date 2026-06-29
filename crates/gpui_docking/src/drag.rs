use crate::{
    DockGraph, DockItemId, DockNodeId, DockSpaceId,
    workspace_drop_transaction::DockWorkspaceDropPayload,
};
use open_gpui::{Bounds, Pixels, Point, Size};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockDragPayload {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_node: DockNodeId,
    pub(crate) kind: DockDragPayloadKind,
    title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockDragPayloadKind {
    Item { item: DockItemId },
    Tabs,
    Floating { floating: DockNodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockDragPayloadIdentity {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    kind: DockDragPayloadKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDragTearOffGeometry {
    source_bounds: Bounds<Pixels>,
    cursor_offset: Point<Pixels>,
    preferred_size: Option<Size<Pixels>>,
    display_work_area: Option<Bounds<Pixels>>,
}

impl DockDragPayload {
    pub(crate) fn new_item(
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_node: source_tabs,
            kind: DockDragPayloadKind::Item { item },
            title,
        }
    }

    pub(crate) fn new_tabs(
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_node: source_tabs,
            kind: DockDragPayloadKind::Tabs,
            title,
        }
    }

    pub(crate) fn new_floating(
        source_space: DockSpaceId,
        floating: DockNodeId,
        title: String,
    ) -> Self {
        Self {
            source_space,
            source_node: floating,
            kind: DockDragPayloadKind::Floating { floating },
            title,
        }
    }

    #[cfg(test)]
    pub(crate) fn item(&self) -> Option<&DockItemId> {
        match &self.kind {
            DockDragPayloadKind::Item { item } => Some(item),
            DockDragPayloadKind::Tabs | DockDragPayloadKind::Floating { .. } => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_tabs_stack(&self) -> bool {
        matches!(self.kind, DockDragPayloadKind::Tabs)
    }

    #[cfg(test)]
    pub(crate) fn floating(&self) -> Option<DockNodeId> {
        match self.kind {
            DockDragPayloadKind::Floating { floating } => Some(floating),
            DockDragPayloadKind::Item { .. } | DockDragPayloadKind::Tabs => None,
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn identity(&self) -> DockDragPayloadIdentity {
        DockDragPayloadIdentity {
            source_space: self.source_space.clone(),
            source_node: self.source_node,
            kind: self.kind.clone(),
        }
    }

    pub(crate) fn as_workspace_payload(&self) -> DockWorkspaceDropPayload<'_> {
        match &self.kind {
            DockDragPayloadKind::Item { item } => DockWorkspaceDropPayload::Item {
                source_tabs: self.source_node,
                item,
            },
            DockDragPayloadKind::Tabs => DockWorkspaceDropPayload::Tabs {
                source_tabs: self.source_node,
            },
            DockDragPayloadKind::Floating { floating } => DockWorkspaceDropPayload::Floating {
                floating: *floating,
            },
        }
    }

    pub(crate) fn excluded_nodes_for_drop_scene(&self, graph: &DockGraph) -> Vec<DockNodeId> {
        let source_node = match self.kind {
            DockDragPayloadKind::Item { .. } => return Vec::new(),
            DockDragPayloadKind::Tabs => self.source_node,
            DockDragPayloadKind::Floating { floating } => floating,
        };
        let nodes = graph.nodes_in_subtree(source_node);
        if nodes.is_empty() {
            vec![source_node]
        } else {
            nodes
        }
    }
}

impl DockDragPayloadIdentity {
    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }
}

impl DockDragTearOffGeometry {
    pub(crate) fn new(source_bounds: Bounds<Pixels>, cursor_offset: Point<Pixels>) -> Self {
        Self {
            source_bounds,
            cursor_offset,
            preferred_size: None,
            display_work_area: None,
        }
    }

    pub(crate) fn from_source_bounds(
        source_bounds: Bounds<Pixels>,
        cursor_position: Point<Pixels>,
    ) -> Self {
        Self::new(source_bounds, cursor_position - source_bounds.origin)
    }

    pub(crate) fn with_preferred_size(mut self, preferred_size: Size<Pixels>) -> Self {
        self.preferred_size = Some(preferred_size);
        self
    }

    pub(crate) fn with_display_work_area(mut self, display_work_area: Bounds<Pixels>) -> Self {
        self.display_work_area = Some(display_work_area);
        self
    }

    pub(crate) fn source_bounds(&self) -> Bounds<Pixels> {
        self.source_bounds
    }

    pub(crate) fn cursor_offset(&self) -> Point<Pixels> {
        self.cursor_offset
    }

    pub(crate) fn preferred_size(&self) -> Option<Size<Pixels>> {
        self.preferred_size
    }

    pub(crate) fn display_work_area(&self) -> Option<Bounds<Pixels>> {
        self.display_work_area
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::Key;

    #[test]
    fn drag_payload_represents_item_or_tabs_stack() {
        let source_space = DockSpaceId::from("main");
        let source_tabs = DockNodeId::null();
        let item_payload = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            DockItemId::from("a"),
            "Panel A".to_string(),
        );
        let floating = DockNodeId::null();
        let tabs_payload =
            DockDragPayload::new_tabs(source_space.clone(), source_tabs, "Stack".to_string());
        let floating_payload =
            DockDragPayload::new_floating(source_space, floating, "Floating".to_string());

        assert_eq!(item_payload.item(), Some(&DockItemId::from("a")));
        assert!(!item_payload.is_tabs_stack());
        assert_eq!(tabs_payload.item(), None);
        assert!(tabs_payload.is_tabs_stack());
        assert_eq!(floating_payload.floating(), Some(floating));
        assert_eq!(floating_payload.source_node, floating);
    }

    #[test]
    fn drag_payload_identity_ignores_preview_title() {
        let source_space = DockSpaceId::from("main");
        let source_tabs = DockNodeId::null();
        let original = DockDragPayload::new_item(
            source_space.clone(),
            source_tabs,
            DockItemId::from("a"),
            "Original title".to_string(),
        );
        let renamed = DockDragPayload::new_item(
            source_space,
            source_tabs,
            DockItemId::from("a"),
            "Renamed title".to_string(),
        );

        assert_eq!(original.identity(), renamed.identity());
        assert_ne!(original, renamed);
    }
}
