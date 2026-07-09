use std::collections::BTreeMap;

use crate::{
    ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope, SnapshotKind, SnapshotNode,
};

/// High-level family for a DevTools snapshot row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DevtoolsSnapshotCategory {
    /// Element, layout, scroll, and docking geometry facts.
    Layout,
    /// Accessibility facts.
    Accessibility,
    /// Focus and input facts.
    Interaction,
    /// Theme and style facts.
    Theme,
    /// Motion runtime facts.
    Motion,
    /// Form and resource state facts.
    Data,
    /// Command registry, keybinding, and resolution facts.
    Command,
    /// Timeline, event, and span facts.
    Timeline,
    /// Probe diagnostics.
    Diagnostic,
    /// Custom app-provided facts.
    Custom,
}

impl DevtoolsSnapshotCategory {
    /// Returns the category for a snapshot kind.
    pub const fn from_kind(kind: &SnapshotKind) -> Self {
        match kind {
            SnapshotKind::Element
            | SnapshotKind::Scroll
            | SnapshotKind::Docking
            | SnapshotKind::Layout => Self::Layout,
            SnapshotKind::Accessibility => Self::Accessibility,
            SnapshotKind::Focus | SnapshotKind::Input => Self::Interaction,
            SnapshotKind::Theme => Self::Theme,
            SnapshotKind::Motion => Self::Motion,
            SnapshotKind::Form | SnapshotKind::Resource => Self::Data,
            SnapshotKind::Command => Self::Command,
            SnapshotKind::Timeline => Self::Timeline,
            SnapshotKind::Diagnostic => Self::Diagnostic,
            SnapshotKind::Custom(_) => Self::Custom,
        }
    }

    /// Returns the stable display label for this category.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Accessibility => "accessibility",
            Self::Interaction => "interaction",
            Self::Theme => "theme",
            Self::Motion => "motion",
            Self::Data => "data",
            Self::Command => "command",
            Self::Timeline => "timeline",
            Self::Diagnostic => "diagnostic",
            Self::Custom => "custom",
        }
    }
}

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
        let collection = collection.sanitized();
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
                category: DevtoolsSnapshotCategory::from_kind(&snapshot.kind),
                category_label: DevtoolsSnapshotCategory::from_kind(&snapshot.kind)
                    .as_label()
                    .to_owned(),
                probe_id: snapshot.probe_id.clone(),
                kind_label: snapshot.kind.as_label().into_owned(),
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

    /// Returns category summaries for visible snapshots and diagnostics.
    pub fn category_summaries(&self) -> Vec<DevtoolsSnapshotCategorySummary> {
        let mut summaries =
            BTreeMap::<DevtoolsSnapshotCategory, DevtoolsCategorySummaryBuilder>::new();

        for snapshot in self
            .snapshots
            .iter()
            .filter(|snapshot| self.matches_filter(snapshot))
        {
            let category = DevtoolsSnapshotCategory::from_kind(&snapshot.kind);
            let summary = summaries
                .entry(category)
                .or_insert_with(|| DevtoolsCategorySummaryBuilder::new(category));
            summary.snapshot_count += 1;
            summary.root_nodes += snapshot.tree.nodes.len();
            summary.total_nodes += snapshot
                .tree
                .nodes
                .iter()
                .map(count_node_tree)
                .sum::<usize>();
            summary.redacted_values += snapshot.redaction.redacted_values;
        }

        let diagnostic_count = self
            .diagnostics
            .iter()
            .filter(|diagnostic| self.diagnostic_matches_filter(diagnostic))
            .count();
        if diagnostic_count > 0 {
            summaries
                .entry(DevtoolsSnapshotCategory::Diagnostic)
                .or_insert_with(|| {
                    DevtoolsCategorySummaryBuilder::new(DevtoolsSnapshotCategory::Diagnostic)
                })
                .diagnostics = diagnostic_count;
        }

        summaries
            .into_values()
            .map(DevtoolsCategorySummaryBuilder::build)
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
            || DevtoolsSnapshotCategory::from_kind(&snapshot.kind)
                .as_label()
                .contains(filter)
            || snapshot
                .tree
                .nodes
                .iter()
                .any(|node| node_matches_filter(node, filter))
    }

    fn diagnostic_matches_filter(&self, diagnostic: &SnapshotDiagnostic) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter = self.filter.as_str();
        diagnostic
            .probe_id
            .as_str()
            .to_ascii_lowercase()
            .contains(filter)
            || diagnostic.code.to_ascii_lowercase().contains(filter)
            || diagnostic.message.to_ascii_lowercase().contains(filter)
            || DevtoolsSnapshotCategory::Diagnostic
                .as_label()
                .contains(filter)
    }
}

/// One row shown by a read-only devtools inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsSnapshotRow {
    /// High-level category for the snapshot.
    pub category: DevtoolsSnapshotCategory,
    /// Stable category label.
    pub category_label: String,
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

/// Aggregate facts for one visible inspector category.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsSnapshotCategorySummary {
    /// High-level category.
    pub category: DevtoolsSnapshotCategory,
    /// Stable category label.
    pub category_label: String,
    /// Number of visible snapshots in this category.
    pub snapshot_count: usize,
    /// Number of root nodes across visible snapshots.
    pub root_nodes: usize,
    /// Total node count across visible snapshots.
    pub total_nodes: usize,
    /// Number of redacted values across visible snapshots.
    pub redacted_values: usize,
    /// Number of visible diagnostics in this category.
    pub diagnostics: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DevtoolsCategorySummaryBuilder {
    category: DevtoolsSnapshotCategory,
    snapshot_count: usize,
    root_nodes: usize,
    total_nodes: usize,
    redacted_values: usize,
    diagnostics: usize,
}

impl DevtoolsCategorySummaryBuilder {
    fn new(category: DevtoolsSnapshotCategory) -> Self {
        Self {
            category,
            snapshot_count: 0,
            root_nodes: 0,
            total_nodes: 0,
            redacted_values: 0,
            diagnostics: 0,
        }
    }

    fn build(self) -> DevtoolsSnapshotCategorySummary {
        DevtoolsSnapshotCategorySummary {
            category: self.category,
            category_label: self.category.as_label().to_owned(),
            snapshot_count: self.snapshot_count,
            root_nodes: self.root_nodes,
            total_nodes: self.total_nodes,
            redacted_values: self.redacted_values,
            diagnostics: self.diagnostics,
        }
    }
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
