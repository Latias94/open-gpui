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
    /// Custom app-provided snapshot.
    Custom(String),
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

/// Serializable envelope returned by a devtools probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    /// Probe that produced this snapshot.
    pub probe_id: ProbeId,
    /// Snapshot kind.
    pub kind: SnapshotKind,
    /// Root snapshot nodes.
    pub nodes: Vec<SnapshotNode>,
    /// Redaction summary.
    pub redaction: SnapshotRedactionSummary,
}
