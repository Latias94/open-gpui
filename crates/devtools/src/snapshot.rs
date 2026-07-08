use serde::{Deserialize, Serialize};

use crate::{ProbeId, SnapshotRedactionSummary};

/// Kind of devtools snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SnapshotKind {
    /// Element, layout, or render-tree facts.
    Element,
    /// Accessibility-tree facts.
    Accessibility,
    /// Focus state.
    Focus,
    /// Input dispatch state.
    Input,
    /// Scroll viewport state.
    Scroll,
    /// Theme resolution state.
    Theme,
    /// Motion state.
    Motion,
    /// Docking state.
    Docking,
    /// Form state.
    Form,
    /// Async resource state.
    Resource,
    /// Probe diagnostic state.
    Diagnostic,
    /// Custom app-provided snapshot.
    Custom(String),
}

/// Tree of inspectable snapshot nodes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotTree {
    /// Root snapshot nodes.
    pub nodes: Vec<SnapshotNode>,
}

impl SnapshotTree {
    /// Creates a snapshot tree from root nodes.
    pub fn new(nodes: impl IntoIterator<Item = SnapshotNode>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
        }
    }

    /// Returns true when the tree has no root nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// One node in an inspectable snapshot tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotNode {
    /// Stable node id within the snapshot.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional JSON payload for diagnostics.
    pub payload: Option<serde_json::Value>,
    /// Child nodes.
    pub children: Vec<SnapshotNode>,
}

impl SnapshotNode {
    /// Creates a snapshot node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            payload: None,
            children: Vec::new(),
        }
    }

    /// Attaches a JSON payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Appends a child node.
    pub fn with_child(mut self, child: SnapshotNode) -> Self {
        self.children.push(child);
        self
    }
}

/// Serializable envelope returned by a devtools probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    /// Probe that produced this snapshot.
    pub probe_id: ProbeId,
    /// Snapshot kind.
    pub kind: SnapshotKind,
    /// Snapshot tree.
    pub tree: SnapshotTree,
    /// Redaction summary.
    pub redaction: SnapshotRedactionSummary,
}

impl SnapshotEnvelope {
    /// Creates a snapshot envelope.
    pub fn new(probe_id: ProbeId, kind: SnapshotKind, tree: SnapshotTree) -> Self {
        Self {
            probe_id,
            kind,
            tree,
            redaction: SnapshotRedactionSummary::default(),
        }
    }

    /// Attaches a redaction summary.
    pub fn with_redaction(mut self, redaction: SnapshotRedactionSummary) -> Self {
        self.redaction = redaction;
        self
    }
}

/// Collection returned by a registry snapshot pass.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotCollection {
    /// Snapshots successfully collected or synthesized as diagnostics.
    pub snapshots: Vec<SnapshotEnvelope>,
    /// Diagnostics from probes that failed to collect.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

/// Diagnostic emitted when a probe cannot provide a snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiagnostic {
    /// Probe that emitted the diagnostic.
    pub probe_id: ProbeId,
    /// Human-readable diagnostic message.
    pub message: String,
}
