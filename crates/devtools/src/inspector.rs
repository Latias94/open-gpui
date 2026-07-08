use crate::{ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotNode};

/// Read-only inspector state over a snapshot collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsInspectorState {
    snapshots: Vec<SnapshotEnvelope>,
    diagnostics: Vec<SnapshotDiagnostic>,
    selected_probe_id: Option<ProbeId>,
    filter: String,
}

impl DevtoolsInspectorState {
    /// Creates inspector state for a collected snapshot pass.
    pub fn new(collection: SnapshotCollection) -> Self {
        let selected_probe_id = collection
            .snapshots
            .first()
            .map(|snapshot| snapshot.probe_id.clone());
        Self {
            snapshots: collection.snapshots,
            diagnostics: collection.diagnostics,
            selected_probe_id,
            filter: String::new(),
        }
    }

    /// Applies a case-insensitive filter over probe ids, kind labels, and node labels.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = normalize_filter(filter.into());
        if let Some(selected) = self.selected_probe_id.as_ref() {
            let selected_is_visible = self
                .snapshots
                .iter()
                .any(|snapshot| &snapshot.probe_id == selected && self.matches_filter(snapshot));
            if !selected_is_visible {
                self.selected_probe_id = self
                    .snapshots
                    .iter()
                    .find(|snapshot| self.matches_filter(snapshot))
                    .map(|snapshot| snapshot.probe_id.clone());
            }
        }
        self
    }

    /// Selects a probe by id without mutating the underlying snapshots.
    pub fn select_probe(mut self, probe_id: &ProbeId) -> Result<Self, DevtoolsInspectorError> {
        if !self
            .snapshots
            .iter()
            .any(|snapshot| &snapshot.probe_id == probe_id)
        {
            return Err(DevtoolsInspectorError::UnknownProbe(probe_id.clone()));
        }
        self.selected_probe_id = Some(probe_id.clone());
        Ok(self)
    }

    /// Returns the selected probe id.
    pub fn selected_probe_id(&self) -> Option<&ProbeId> {
        self.selected_probe_id.as_ref()
    }

    /// Returns probe diagnostics from failed snapshot collection.
    pub fn diagnostics(&self) -> &[SnapshotDiagnostic] {
        &self.diagnostics
    }

    /// Returns the current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Returns visible snapshot rows for the current filter.
    pub fn snapshot_rows(&self) -> Vec<DevtoolsSnapshotRow> {
        self.snapshots
            .iter()
            .filter(|snapshot| self.matches_filter(snapshot))
            .map(|snapshot| DevtoolsSnapshotRow {
                probe_id: snapshot.probe_id.clone(),
                kind_label: snapshot.kind.as_label().to_owned(),
                root_nodes: snapshot.tree.nodes.len(),
                total_nodes: snapshot.tree.nodes.iter().map(count_node_tree).sum(),
                redacted_values: snapshot.redaction.redacted_values,
                selected: self
                    .selected_probe_id
                    .as_ref()
                    .is_some_and(|selected| selected == &snapshot.probe_id),
            })
            .collect()
    }

    /// Returns the selected snapshot, if any.
    pub fn selected_snapshot(&self) -> Option<&SnapshotEnvelope> {
        let selected = self.selected_probe_id.as_ref()?;
        self.snapshots
            .iter()
            .find(|snapshot| &snapshot.probe_id == selected)
    }

    /// Returns the selected snapshot as redaction-preserving JSON.
    pub fn selected_snapshot_json(&self) -> Result<serde_json::Value, DevtoolsInspectorError> {
        let snapshot = self
            .selected_snapshot()
            .ok_or(DevtoolsInspectorError::NoSelectedSnapshot)?;
        serde_json::to_value(snapshot).map_err(DevtoolsInspectorError::SerializeSnapshot)
    }

    fn matches_filter(&self, snapshot: &SnapshotEnvelope) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter = self.filter.as_str();
        snapshot
            .probe_id
            .as_str()
            .to_ascii_lowercase()
            .contains(filter)
            || snapshot
                .kind
                .as_label()
                .to_ascii_lowercase()
                .contains(filter)
            || snapshot
                .tree
                .nodes
                .iter()
                .any(|node| node_matches_filter(node, filter))
    }
}

/// One row shown by a read-only devtools inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsSnapshotRow {
    /// Probe that produced this snapshot.
    pub probe_id: ProbeId,
    /// Stable snapshot kind label.
    pub kind_label: String,
    /// Number of root nodes in the snapshot tree.
    pub root_nodes: usize,
    /// Total node count across the snapshot tree.
    pub total_nodes: usize,
    /// Number of redacted values in the snapshot.
    pub redacted_values: usize,
    /// Whether this row is selected.
    pub selected: bool,
}

/// Error returned by read-only inspector state operations.
#[derive(Debug, thiserror::Error)]
pub enum DevtoolsInspectorError {
    /// The requested probe is not present in the snapshot collection.
    #[error("unknown devtools probe: {0}")]
    UnknownProbe(ProbeId),
    /// No snapshot is selected.
    #[error("no selected devtools snapshot")]
    NoSelectedSnapshot,
    /// The selected snapshot could not be serialized.
    #[error("failed to serialize devtools snapshot")]
    SerializeSnapshot(#[source] serde_json::Error),
}

fn normalize_filter(filter: String) -> String {
    filter.trim().to_ascii_lowercase()
}

fn count_node_tree(node: &SnapshotNode) -> usize {
    1 + node.children.iter().map(count_node_tree).sum::<usize>()
}

fn node_matches_filter(node: &SnapshotNode, filter: &str) -> bool {
    node.id.to_ascii_lowercase().contains(filter)
        || node.label.to_ascii_lowercase().contains(filter)
        || node
            .children
            .iter()
            .any(|child| node_matches_filter(child, filter))
}
