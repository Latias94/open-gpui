use std::collections::BTreeMap;

use crate::{
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainSnapshot, DevtoolsEventRecord,
    DevtoolsTargetId, DevtoolsTargetSnapshot, ProbeId, SnapshotCollection, SnapshotDiagnostic,
    SnapshotEnvelope, SnapshotKind, SnapshotNode,
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
    targets: Vec<DevtoolsTargetSnapshot>,
    domains: Vec<DevtoolsDomainSnapshot>,
    events: Vec<DevtoolsEventRecord>,
    selected_probe_id: Option<ProbeId>,
    selected_target_id: Option<DevtoolsTargetId>,
    selected_domain_id: Option<DevtoolsDomainId>,
    selected_event_sequence: Option<u64>,
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
            targets: Vec::new(),
            domains: Vec::new(),
            events: Vec::new(),
            selected_probe_id,
            selected_target_id: None,
            selected_domain_id: None,
            selected_event_sequence: None,
            filter: String::new(),
        }
    }

    /// Creates inspector state for a target/domain/event capture.
    pub fn from_capture(capture: DevtoolsCapture) -> Self {
        let capture = capture.sanitized();
        let collection = capture.snapshot_collection();
        let selected_probe_id = collection
            .snapshots
            .first()
            .map(|snapshot| snapshot.probe_id.clone());
        let selected_target_id = capture
            .domains
            .first()
            .map(|domain| domain.target_id.clone())
            .or_else(|| {
                capture
                    .targets
                    .targets
                    .first()
                    .map(|target| target.id.clone())
            });
        let selected_domain_id = first_domain_for_target(&capture.domains, &selected_target_id);
        let selected_event_sequence = capture.events.first().map(DevtoolsEventRecord::sequence);

        Self {
            snapshots: collection.snapshots,
            diagnostics: collection.diagnostics,
            targets: capture.targets.targets,
            domains: capture.domains,
            events: capture.events,
            selected_probe_id,
            selected_target_id,
            selected_domain_id,
            selected_event_sequence,
            filter: String::new(),
        }
    }

    /// Applies a case-insensitive filter over targets, domains, events, probe ids, kind labels, and node labels.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = normalize_filter(filter.into());
        self.sync_target_selection_to_filter();
        self.sync_domain_selection_to_filter();
        self.sync_event_selection_to_filter();
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

    /// Selects a target by id without mutating captured data.
    pub fn select_target(
        mut self,
        target_id: &DevtoolsTargetId,
    ) -> Result<Self, DevtoolsInspectorError> {
        if !self.targets.iter().any(|target| &target.id == target_id) {
            return Err(DevtoolsInspectorError::UnknownTarget(target_id.clone()));
        }
        self.selected_target_id = Some(target_id.clone());
        self.sync_domain_selection_to_filter();
        self.sync_event_selection_to_filter();
        Ok(self)
    }

    /// Selects a domain by id and moves target selection to the owning target.
    pub fn select_domain(
        mut self,
        domain_id: &DevtoolsDomainId,
    ) -> Result<Self, DevtoolsInspectorError> {
        let domain = self
            .domains
            .iter()
            .find(|domain| &domain.id == domain_id)
            .ok_or_else(|| DevtoolsInspectorError::UnknownDomain(domain_id.clone()))?;
        self.selected_target_id = Some(domain.target_id.clone());
        self.selected_domain_id = Some(domain.id.clone());
        self.sync_event_selection_to_filter();
        Ok(self)
    }

    /// Selects an event by append-time sequence and moves target/domain selection when present.
    pub fn select_event(mut self, sequence: u64) -> Result<Self, DevtoolsInspectorError> {
        let event = self
            .events
            .iter()
            .find(|event| event.sequence() == sequence)
            .ok_or(DevtoolsInspectorError::UnknownEvent(sequence))?;
        if let Some(target_id) = event.target_id_ref() {
            self.selected_target_id = Some(target_id.clone());
        }
        if let Some(domain_id) = event.domain_id_ref() {
            self.selected_domain_id = Some(domain_id.clone());
        }
        self.selected_event_sequence = Some(sequence);
        Ok(self)
    }

    /// Returns the selected probe id.
    pub fn selected_probe_id(&self) -> Option<&ProbeId> {
        self.selected_probe_id.as_ref()
    }

    /// Returns the selected target id.
    pub fn selected_target_id(&self) -> Option<&DevtoolsTargetId> {
        self.selected_target_id.as_ref()
    }

    /// Returns the selected domain id.
    pub fn selected_domain_id(&self) -> Option<&DevtoolsDomainId> {
        self.selected_domain_id.as_ref()
    }

    /// Returns the selected event sequence.
    pub fn selected_event_sequence(&self) -> Option<u64> {
        self.selected_event_sequence
    }

    /// Returns the selected target, if any.
    pub fn selected_target(&self) -> Option<&DevtoolsTargetSnapshot> {
        let selected = self.selected_target_id.as_ref()?;
        self.targets.iter().find(|target| &target.id == selected)
    }

    /// Returns the selected domain, if any.
    pub fn selected_domain(&self) -> Option<&DevtoolsDomainSnapshot> {
        let selected = self.selected_domain_id.as_ref()?;
        self.domains.iter().find(|domain| &domain.id == selected)
    }

    /// Returns the selected event, if any.
    pub fn selected_event(&self) -> Option<&DevtoolsEventRecord> {
        let selected = self.selected_event_sequence?;
        self.events
            .iter()
            .find(|event| event.sequence() == selected)
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

    /// Returns visible target rows for the current filter.
    pub fn target_rows(&self) -> Vec<DevtoolsTargetRow> {
        self.targets
            .iter()
            .filter(|target| self.target_matches_filter(target))
            .map(|target| DevtoolsTargetRow {
                target_id: target.id.clone(),
                kind_label: target.kind.as_label().to_owned(),
                label: target.label.clone(),
                parent_id: target.parent_id.clone(),
                child_target_count: self
                    .targets
                    .iter()
                    .filter(|candidate| {
                        candidate
                            .parent_id
                            .as_ref()
                            .is_some_and(|parent_id| parent_id == &target.id)
                    })
                    .count(),
                domain_count: self
                    .domains
                    .iter()
                    .filter(|domain| domain.target_id == target.id)
                    .count(),
                event_count: self
                    .events
                    .iter()
                    .filter(|event| {
                        event
                            .target_id_ref()
                            .is_some_and(|target_id| target_id == &target.id)
                    })
                    .count(),
                selected: self
                    .selected_target_id
                    .as_ref()
                    .is_some_and(|selected| selected == &target.id),
            })
            .collect()
    }

    /// Returns visible domain rows for the selected target and current filter.
    pub fn domain_rows(&self) -> Vec<DevtoolsDomainRow> {
        self.domains
            .iter()
            .filter(|domain| self.domain_is_visible(domain))
            .map(|domain| DevtoolsDomainRow {
                domain_id: domain.id.clone(),
                target_id: domain.target_id.clone(),
                kind_label: domain.kind.as_label().to_owned(),
                label: domain.label.clone(),
                has_snapshot: domain.snapshot.is_some(),
                snapshot_root_nodes: domain
                    .snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.tree.nodes.len()),
                redacted_values: domain
                    .snapshot
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.redaction.redacted_values),
                event_count: self
                    .events
                    .iter()
                    .filter(|event| {
                        event
                            .domain_id_ref()
                            .is_some_and(|domain_id| domain_id == &domain.id)
                    })
                    .count(),
                diagnostic_count: domain.diagnostics.len(),
                selected: self
                    .selected_domain_id
                    .as_ref()
                    .is_some_and(|selected| selected == &domain.id),
            })
            .collect()
    }

    /// Returns visible event rows for the selected target/domain and current filter.
    pub fn event_rows(&self) -> Vec<DevtoolsEventRow> {
        self.events
            .iter()
            .filter(|event| self.event_is_visible(event))
            .map(|event| DevtoolsEventRow {
                sequence: event.sequence(),
                event_id: event.id().to_owned(),
                kind_label: event.kind().as_label().to_owned(),
                label: event.label().to_owned(),
                target_id: event.target_id_ref().cloned(),
                domain_id: event.domain_id_ref().cloned(),
                timestamp_ms: event.timestamp_ms_value(),
                duration_ms: event.duration_ms_value(),
                has_payload: event.payload().is_some(),
                selected: self.selected_event_sequence == Some(event.sequence()),
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

    /// Returns the selected detail using domain snapshot, selected event, then legacy snapshot priority.
    pub fn selected_detail(&self) -> Option<DevtoolsInspectorDetail> {
        if let Some(domain) = self.selected_domain() {
            if let Some(snapshot) = domain.snapshot.as_ref() {
                let json = serde_json::to_value(snapshot).ok()?;
                return Some(DevtoolsInspectorDetail::new(
                    DevtoolsInspectorDetailKind::DomainSnapshot,
                    format!("{} / {}", domain.label, domain.kind.as_label()),
                    json,
                ));
            }
        }

        if let Some(event) = self.selected_event() {
            let json = serde_json::to_value(event).ok()?;
            return Some(DevtoolsInspectorDetail::new(
                DevtoolsInspectorDetailKind::Event,
                format!("{} / {}", event.label(), event.kind().as_label()),
                json,
            ));
        }

        if let Some(snapshot) = self.selected_snapshot() {
            let json = serde_json::to_value(snapshot).ok()?;
            return Some(DevtoolsInspectorDetail::new(
                DevtoolsInspectorDetailKind::LegacySnapshot,
                format!(
                    "{} / {}",
                    snapshot.probe_id.as_str(),
                    snapshot.kind.as_label()
                ),
                json,
            ));
        }

        None
    }

    /// Returns the selected detail as JSON using the documented detail priority.
    pub fn selected_detail_json(&self) -> Result<serde_json::Value, DevtoolsInspectorError> {
        self.selected_detail()
            .map(|detail| detail.json)
            .ok_or(DevtoolsInspectorError::NoSelectedDetail)
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

    fn sync_target_selection_to_filter(&mut self) {
        let selected_is_visible = self.selected_target_id.as_ref().is_some_and(|selected| {
            self.targets
                .iter()
                .any(|target| &target.id == selected && self.target_matches_filter(target))
        });
        if !selected_is_visible {
            self.selected_target_id = self
                .targets
                .iter()
                .find(|target| self.target_matches_filter(target))
                .map(|target| target.id.clone());
        }
    }

    fn sync_domain_selection_to_filter(&mut self) {
        let selected_is_visible = self.selected_domain_id.as_ref().is_some_and(|selected| {
            self.domains
                .iter()
                .any(|domain| &domain.id == selected && self.domain_is_visible(domain))
        });
        if !selected_is_visible {
            self.selected_domain_id = self
                .domains
                .iter()
                .find(|domain| self.domain_is_visible(domain))
                .map(|domain| domain.id.clone());
        }
    }

    fn sync_event_selection_to_filter(&mut self) {
        let selected_is_visible = self.selected_event_sequence.is_some_and(|selected| {
            self.events
                .iter()
                .any(|event| event.sequence() == selected && self.event_is_visible(event))
        });
        if !selected_is_visible {
            self.selected_event_sequence = self
                .events
                .iter()
                .find(|event| self.event_is_visible(event))
                .map(DevtoolsEventRecord::sequence);
        }
    }

    fn domain_is_visible(&self, domain: &DevtoolsDomainSnapshot) -> bool {
        self.selected_target_id
            .as_ref()
            .is_none_or(|target_id| &domain.target_id == target_id)
            && self.domain_matches_filter(domain)
    }

    fn event_is_visible(&self, event: &DevtoolsEventRecord) -> bool {
        self.selected_target_id.as_ref().is_none_or(|target_id| {
            event
                .target_id_ref()
                .is_none_or(|event_target| event_target == target_id)
        }) && self.selected_domain_id.as_ref().is_none_or(|domain_id| {
            event
                .domain_id_ref()
                .is_none_or(|event_domain| event_domain == domain_id)
        }) && self.event_matches_filter(event)
    }

    fn target_matches_filter(&self, target: &DevtoolsTargetSnapshot) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter = self.filter.as_str();
        matches_text(target.id.as_str(), filter)
            || matches_text(target.kind.as_label(), filter)
            || matches_text(&target.label, filter)
            || target
                .parent_id
                .as_ref()
                .is_some_and(|parent_id| matches_text(parent_id.as_str(), filter))
            || target
                .metadata
                .as_ref()
                .is_some_and(|metadata| matches_text(&metadata.to_string(), filter))
            || self
                .domains
                .iter()
                .any(|domain| domain.target_id == target.id && self.domain_matches_filter(domain))
            || self.events.iter().any(|event| {
                event
                    .target_id_ref()
                    .is_some_and(|target_id| target_id == &target.id)
                    && self.event_matches_filter(event)
            })
    }

    fn domain_matches_filter(&self, domain: &DevtoolsDomainSnapshot) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter = self.filter.as_str();
        matches_text(domain.id.as_str(), filter)
            || matches_text(domain.target_id.as_str(), filter)
            || matches_text(domain.kind.as_label(), filter)
            || matches_text(&domain.label, filter)
            || domain
                .summary
                .as_ref()
                .is_some_and(|summary| matches_text(&summary.to_string(), filter))
            || domain
                .diagnostics
                .iter()
                .any(|diagnostic| self.diagnostic_matches_filter(diagnostic))
            || self.events.iter().any(|event| {
                event
                    .domain_id_ref()
                    .is_some_and(|domain_id| domain_id == &domain.id)
                    && self.event_matches_filter(event)
            })
    }

    fn event_matches_filter(&self, event: &DevtoolsEventRecord) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter = self.filter.as_str();
        matches_text(event.id(), filter)
            || matches_text(event.label(), filter)
            || matches_text(event.kind().as_label(), filter)
            || event
                .target_id_ref()
                .is_some_and(|target_id| matches_text(target_id.as_str(), filter))
            || event
                .domain_id_ref()
                .is_some_and(|domain_id| matches_text(domain_id.as_str(), filter))
            || event
                .payload()
                .is_some_and(|payload| matches_text(&payload.to_string(), filter))
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

/// One target row shown by a read-only devtools inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsTargetRow {
    /// Stable target id.
    pub target_id: DevtoolsTargetId,
    /// Stable target kind label.
    pub kind_label: String,
    /// Human-readable target label.
    pub label: String,
    /// Optional parent target id.
    pub parent_id: Option<DevtoolsTargetId>,
    /// Number of direct child targets.
    pub child_target_count: usize,
    /// Number of domains attached to this target.
    pub domain_count: usize,
    /// Number of events attached to this target.
    pub event_count: usize,
    /// Whether this target is selected.
    pub selected: bool,
}

/// One domain row shown by a read-only devtools inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsDomainRow {
    /// Stable domain id.
    pub domain_id: DevtoolsDomainId,
    /// Target that owns this domain row.
    pub target_id: DevtoolsTargetId,
    /// Stable domain kind label.
    pub kind_label: String,
    /// Human-readable domain label.
    pub label: String,
    /// Whether a legacy snapshot backs this domain.
    pub has_snapshot: bool,
    /// Number of root nodes in the backing legacy snapshot.
    pub snapshot_root_nodes: usize,
    /// Number of redacted values in the backing legacy snapshot.
    pub redacted_values: usize,
    /// Number of events attached to this domain.
    pub event_count: usize,
    /// Number of diagnostics attached to this domain.
    pub diagnostic_count: usize,
    /// Whether this domain is selected.
    pub selected: bool,
}

/// One event row shown by a read-only devtools inspector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsEventRow {
    /// Stable append-time event sequence.
    pub sequence: u64,
    /// Stable event id.
    pub event_id: String,
    /// Stable event kind label.
    pub kind_label: String,
    /// Human-readable event label.
    pub label: String,
    /// Optional target id.
    pub target_id: Option<DevtoolsTargetId>,
    /// Optional domain id.
    pub domain_id: Option<DevtoolsDomainId>,
    /// Optional producer timestamp in milliseconds.
    pub timestamp_ms: Option<u64>,
    /// Optional event duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Whether the event has a payload.
    pub has_payload: bool,
    /// Whether this event is selected.
    pub selected: bool,
}

/// Kind of selected inspector detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevtoolsInspectorDetailKind {
    /// Detail is the selected domain's legacy snapshot.
    DomainSnapshot,
    /// Detail is the selected event record.
    Event,
    /// Detail is the selected legacy snapshot fallback.
    LegacySnapshot,
}

impl DevtoolsInspectorDetailKind {
    /// Returns the stable display label for this detail kind.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::DomainSnapshot => "domain-snapshot",
            Self::Event => "event",
            Self::LegacySnapshot => "legacy-snapshot",
        }
    }
}

/// Selected inspector detail ready for copy or export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevtoolsInspectorDetail {
    /// Kind of selected detail.
    pub kind: DevtoolsInspectorDetailKind,
    /// Stable kind label.
    pub kind_label: String,
    /// Human-readable detail label.
    pub label: String,
    /// Redaction-preserving JSON payload for the selected detail.
    pub json: serde_json::Value,
    /// Readable copy control label.
    pub copy_label: String,
    /// Readable export control label.
    pub export_label: String,
    /// Readable success feedback for tests and UI copy.
    pub feedback_label: String,
}

impl DevtoolsInspectorDetail {
    fn new(
        kind: DevtoolsInspectorDetailKind,
        label: impl Into<String>,
        json: serde_json::Value,
    ) -> Self {
        Self {
            kind,
            kind_label: kind.as_label().to_owned(),
            label: label.into(),
            json,
            copy_label: "Copy selected detail JSON".to_owned(),
            export_label: "Export selected detail JSON".to_owned(),
            feedback_label: "Selected detail JSON is ready".to_owned(),
        }
    }
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
    /// The requested target is not present in the capture.
    #[error("unknown devtools target: {0}")]
    UnknownTarget(DevtoolsTargetId),
    /// The requested domain is not present in the capture.
    #[error("unknown devtools domain: {0}")]
    UnknownDomain(DevtoolsDomainId),
    /// The requested event sequence is not present in the capture.
    #[error("unknown devtools event sequence: {0}")]
    UnknownEvent(u64),
    /// No snapshot is selected.
    #[error("no selected devtools snapshot")]
    NoSelectedSnapshot,
    /// No target/domain/event/legacy detail is selected.
    #[error("no selected devtools detail")]
    NoSelectedDetail,
    /// The selected snapshot could not be serialized.
    #[error("failed to serialize devtools snapshot")]
    SerializeSnapshot(#[source] serde_json::Error),
}

fn normalize_filter(filter: String) -> String {
    filter.trim().to_ascii_lowercase()
}

fn first_domain_for_target(
    domains: &[DevtoolsDomainSnapshot],
    selected_target_id: &Option<DevtoolsTargetId>,
) -> Option<DevtoolsDomainId> {
    domains
        .iter()
        .find(|domain| {
            selected_target_id
                .as_ref()
                .is_none_or(|target_id| &domain.target_id == target_id)
        })
        .map(|domain| domain.id.clone())
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

fn matches_text(value: &str, filter: &str) -> bool {
    value.to_ascii_lowercase().contains(filter)
}
