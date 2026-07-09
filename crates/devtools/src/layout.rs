//! Renderer-neutral DevTools layout snapshots.

use serde::Serialize;

use crate::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsDomainSnapshot,
    DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree, ProbeId,
    ProbeSnapshotError, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbe,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_json_value, sanitize_sensitive_text, snapshot_node_with_payload},
};

/// Point facts exported by a layout producer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LayoutPointSnapshot {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl LayoutPointSnapshot {
    /// Creates a point snapshot.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Size facts exported by a layout producer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LayoutSizeSnapshot {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl LayoutSizeSnapshot {
    /// Creates a size snapshot.
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// Bounds facts exported by a layout producer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LayoutBoundsSnapshot {
    /// Origin point.
    pub origin: LayoutPointSnapshot,
    /// Bounds size.
    pub size: LayoutSizeSnapshot,
}

impl LayoutBoundsSnapshot {
    /// Creates bounds from an origin and size.
    pub const fn new(origin: LayoutPointSnapshot, size: LayoutSizeSnapshot) -> Self {
        Self { origin, size }
    }
}

/// One node in a layout snapshot tree.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNodeSnapshot {
    id: String,
    label: String,
    bounds: Option<LayoutBoundsSnapshot>,
    content_size: Option<LayoutSizeSnapshot>,
    scroll_offset: Option<LayoutPointSnapshot>,
    max_scroll_offset: Option<LayoutPointSnapshot>,
    payload: Option<serde_json::Value>,
    children: Vec<LayoutNodeSnapshot>,
}

impl LayoutNodeSnapshot {
    /// Creates a sanitized layout node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            bounds: None,
            content_size: None,
            scroll_offset: None,
            max_scroll_offset: None,
            payload: None,
            children: Vec::new(),
        }
    }

    /// Attaches committed bounds.
    pub const fn bounds(mut self, bounds: LayoutBoundsSnapshot) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Attaches committed content size.
    pub const fn content_size(mut self, content_size: LayoutSizeSnapshot) -> Self {
        self.content_size = Some(content_size);
        self
    }

    /// Attaches committed scroll offset.
    pub const fn scroll_offset(mut self, scroll_offset: LayoutPointSnapshot) -> Self {
        self.scroll_offset = Some(scroll_offset);
        self
    }

    /// Attaches committed maximum scroll offset.
    pub const fn max_scroll_offset(mut self, max_scroll_offset: LayoutPointSnapshot) -> Self {
        self.max_scroll_offset = Some(max_scroll_offset);
        self
    }

    /// Attaches sanitized producer-specific payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(sanitize_json_value(payload));
        self
    }

    /// Appends a child layout node.
    pub fn with_child(mut self, child: LayoutNodeSnapshot) -> Self {
        self.children.push(child.sanitized());
        self
    }

    /// Returns this node with every exported string and payload sanitized.
    pub fn sanitized(mut self) -> Self {
        self.id = sanitize_sensitive_text(&self.id);
        self.label = sanitize_sensitive_text(&self.label);
        self.payload = self.payload.map(sanitize_json_value);
        self.children = self
            .children
            .into_iter()
            .map(LayoutNodeSnapshot::sanitized)
            .collect();
        self
    }

    fn snapshot_node(&self) -> SnapshotNode {
        let mut node = snapshot_node_with_payload(
            ["layout", self.id.as_str()],
            self.label.as_str(),
            serde_json::json!({
                "bounds": self.bounds,
                "content_size": self.content_size,
                "scroll_offset": self.scroll_offset,
                "max_scroll_offset": self.max_scroll_offset,
                "payload": self.payload,
            }),
        );

        for child in &self.children {
            node = node.with_child(child.snapshot_node());
        }

        node
    }
}

/// Bounded public layout facts ready to convert into a DevTools snapshot tree.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSnapshot {
    id: String,
    label: String,
    nodes: Vec<LayoutNodeSnapshot>,
}

impl LayoutSnapshot {
    /// Creates a sanitized layout snapshot.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        nodes: impl IntoIterator<Item = LayoutNodeSnapshot>,
    ) -> Self {
        Self {
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            nodes: nodes
                .into_iter()
                .map(LayoutNodeSnapshot::sanitized)
                .collect(),
        }
    }

    /// Returns the sanitized layout id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the sanitized layout label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns root layout nodes.
    pub fn nodes(&self) -> &[LayoutNodeSnapshot] {
        &self.nodes
    }

    /// Converts this layout snapshot into a sanitized DevTools tree.
    pub fn tree(&self) -> SnapshotTree {
        let mut root = snapshot_node_with_payload(
            ["layout", self.id.as_str(), "root"],
            self.label.as_str(),
            serde_json::json!({
                "id": self.id,
                "label": self.label,
                "root_nodes": self.nodes.len(),
            }),
        );

        for node in &self.nodes {
            root = root.with_child(node.snapshot_node());
        }

        SnapshotTree::new([root])
    }

    /// Converts this layout snapshot into a probe snapshot.
    pub fn probe_snapshot(&self) -> SnapshotProbeSnapshot {
        SnapshotProbeSnapshot::new(self.tree()).with_redaction(SnapshotRedactionSummary::default())
    }

    /// Converts this layout snapshot into an envelope.
    pub fn envelope(&self, probe_id: ProbeId) -> SnapshotEnvelope {
        SnapshotEnvelope::new(probe_id, SnapshotKind::Layout, self.tree())
            .with_redaction(SnapshotRedactionSummary::default())
    }

    /// Converts this layout snapshot into a first-party DevTools capture.
    pub fn capture(&self, probe_id: ProbeId) -> DevtoolsCapture {
        let envelope = self.envelope(probe_id.clone());
        let target_id = DevtoolsTargetId::from_parts(["layout", self.id(), probe_id.as_str()]);
        let domain_id = DevtoolsDomainId::from_parts(["layout", self.id(), probe_id.as_str()]);
        let target = DevtoolsTargetSnapshot::new(
            target_id.clone(),
            DevtoolsTargetKind::Runtime,
            self.label(),
        )
        .with_metadata(serde_json::json!({
            "probe_id": probe_id.as_str(),
            "domain": "layout",
            "layout_id": self.id(),
        }));
        let domain = DevtoolsDomainSnapshot::new(
            domain_id,
            target_id,
            DevtoolsDomainKind::Layout,
            self.label(),
        )
        .with_summary(serde_json::json!({
            "id": self.id(),
            "label": self.label(),
            "root_nodes": self.nodes().len(),
        }))
        .with_snapshot(envelope.clone());

        DevtoolsCapture::new(
            DevtoolsTargetTree::new([target]),
            [domain],
            Vec::new(),
            [envelope],
            Vec::new(),
        )
    }
}

/// Converts a layout snapshot into a probe snapshot.
pub fn layout_probe_snapshot(snapshot: &LayoutSnapshot) -> SnapshotProbeSnapshot {
    snapshot.probe_snapshot()
}

/// Converts a layout snapshot into an envelope.
pub fn layout_snapshot_envelope(probe_id: ProbeId, snapshot: &LayoutSnapshot) -> SnapshotEnvelope {
    snapshot.envelope(probe_id)
}

/// Converts a layout snapshot into a first-party DevTools capture.
pub fn layout_capture(probe_id: ProbeId, snapshot: &LayoutSnapshot) -> DevtoolsCapture {
    snapshot.capture(probe_id)
}

/// Builds a closure-backed layout snapshot probe.
pub fn layout_snapshot_probe<F>(
    id: impl Into<String>,
    snapshot: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> LayoutSnapshot + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Layout, move || {
        Ok(layout_probe_snapshot(&snapshot()))
    })
}
