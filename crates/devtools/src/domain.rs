//! Domain-scoped DevTools facts and capture collections.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsEventRecord, DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot,
    DevtoolsTargetTree, ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotEnvelope,
    SnapshotKind,
    adapters::{sanitize_json_value, sanitize_sensitive_text, stable_node_id, summary_payload},
};

/// Stable id for a DevTools domain snapshot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DevtoolsDomainId(String);

impl DevtoolsDomainId {
    /// Creates a sanitized domain id.
    pub fn new(id: impl Into<String>) -> Self {
        let id = sanitize_sensitive_text(&id.into());
        if id.trim().is_empty() {
            Self("domain".to_owned())
        } else {
            Self(id)
        }
    }

    /// Creates a deterministic domain id from sanitized path segments.
    pub fn from_parts<I, S>(parts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self(stable_node_id(parts))
    }

    /// Creates a deterministic domain id for a legacy probe snapshot.
    pub fn from_probe_snapshot(probe_id: &ProbeId, kind: &SnapshotKind) -> Self {
        Self::from_parts(["domain", probe_id.as_str(), kind.as_label().as_ref()])
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DevtoolsDomainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kind of DevTools domain.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsDomainKind {
    /// Element, scroll, bounds, and committed layout facts.
    Layout,
    /// Accessibility facts.
    Accessibility,
    /// Focus and input facts.
    Interaction,
    /// Theme and style facts.
    Theme,
    /// Motion runtime facts.
    Motion,
    /// Docking and multi-viewport facts.
    Docking,
    /// Form, resource, and async data facts.
    Data,
    /// Command registry, keybinding, and keymap facts.
    Command,
    /// Timeline and event facts.
    Timeline,
    /// Probe diagnostics.
    Diagnostic,
    /// Custom application-provided domain.
    Custom(String),
}

impl DevtoolsDomainKind {
    /// Returns the matching domain for a legacy snapshot kind.
    pub fn from_snapshot_kind(kind: &SnapshotKind) -> Self {
        match kind {
            SnapshotKind::Element | SnapshotKind::Scroll | SnapshotKind::Layout => Self::Layout,
            SnapshotKind::Accessibility => Self::Accessibility,
            SnapshotKind::Focus | SnapshotKind::Input => Self::Interaction,
            SnapshotKind::Theme => Self::Theme,
            SnapshotKind::Motion => Self::Motion,
            SnapshotKind::Docking => Self::Docking,
            SnapshotKind::Form | SnapshotKind::Resource => Self::Data,
            SnapshotKind::Command => Self::Command,
            SnapshotKind::Timeline => Self::Timeline,
            SnapshotKind::Diagnostic => Self::Diagnostic,
            SnapshotKind::Custom(label) => Self::Custom(sanitize_sensitive_text(label)),
        }
    }

    /// Returns the stable display label for this domain kind.
    pub fn as_label(&self) -> &str {
        match self {
            Self::Layout => "layout",
            Self::Accessibility => "accessibility",
            Self::Interaction => "interaction",
            Self::Theme => "theme",
            Self::Motion => "motion",
            Self::Docking => "docking",
            Self::Data => "data",
            Self::Command => "command",
            Self::Timeline => "timeline",
            Self::Diagnostic => "diagnostic",
            Self::Custom(label) => label.as_str(),
        }
    }

    /// Returns this kind with custom labels sanitized.
    pub fn sanitized(self) -> Self {
        match self {
            Self::Custom(label) => Self::Custom(sanitize_sensitive_text(&label)),
            other => other,
        }
    }
}

/// One domain-scoped output in a DevTools capture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsDomainSnapshot {
    /// Stable domain id.
    pub id: DevtoolsDomainId,
    /// Target that owns this domain output.
    pub target_id: DevtoolsTargetId,
    /// Domain kind.
    pub kind: DevtoolsDomainKind,
    /// Human-readable domain label.
    pub label: String,
    /// Optional sanitized summary payload.
    pub summary: Option<serde_json::Value>,
    /// Optional legacy snapshot envelope backing this domain.
    pub snapshot: Option<SnapshotEnvelope>,
    /// Diagnostics attached to this domain output.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

impl DevtoolsDomainSnapshot {
    /// Creates a sanitized domain snapshot.
    pub fn new(
        id: DevtoolsDomainId,
        target_id: DevtoolsTargetId,
        kind: DevtoolsDomainKind,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            target_id,
            kind: kind.sanitized(),
            label: sanitize_sensitive_text(&label.into()),
            summary: None,
            snapshot: None,
            diagnostics: Vec::new(),
        }
        .sanitized()
    }

    /// Creates a domain snapshot from a legacy snapshot envelope.
    pub fn from_snapshot(target_id: DevtoolsTargetId, snapshot: SnapshotEnvelope) -> Self {
        let snapshot = snapshot.sanitized();
        let kind = DevtoolsDomainKind::from_snapshot_kind(&snapshot.kind);
        let id = DevtoolsDomainId::from_probe_snapshot(&snapshot.probe_id, &snapshot.kind);
        let label = snapshot.kind.as_label().into_owned();
        let root_nodes = snapshot.tree.nodes.len();
        let redacted_values = snapshot.redaction.redacted_values;

        Self::new(id, target_id, kind, label)
            .with_summary(serde_json::json!({
                "probe_id": snapshot.probe_id.as_str(),
                "snapshot_kind": snapshot.kind.as_label(),
                "root_nodes": root_nodes,
                "redacted_values": redacted_values,
            }))
            .with_snapshot(snapshot)
    }

    /// Attaches a sanitized summary payload.
    pub fn with_summary<T>(mut self, summary: T) -> Self
    where
        T: Serialize,
    {
        self.summary = Some(summary_payload(summary));
        self
    }

    /// Attaches a legacy snapshot envelope.
    pub fn with_snapshot(mut self, snapshot: SnapshotEnvelope) -> Self {
        self.snapshot = Some(snapshot.sanitized());
        self
    }

    /// Attaches a diagnostic to this domain.
    pub fn with_diagnostic(mut self, diagnostic: SnapshotDiagnostic) -> Self {
        self.diagnostics.push(diagnostic.sanitized());
        self
    }

    /// Returns this domain with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.id = DevtoolsDomainId::new(self.id.0);
        self.target_id = DevtoolsTargetId::new(self.target_id.as_str());
        self.kind = self.kind.sanitized();
        self.label = sanitize_sensitive_text(&self.label);
        self.summary = self.summary.map(sanitize_json_value);
        self.snapshot = self.snapshot.map(SnapshotEnvelope::sanitized);
        self.diagnostics = self
            .diagnostics
            .into_iter()
            .map(SnapshotDiagnostic::sanitized)
            .collect();
        self
    }
}

/// Rich target/domain/event capture produced by DevTools.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsCapture {
    /// Target tree for this capture.
    pub targets: DevtoolsTargetTree,
    /// Domain outputs attached to targets.
    pub domains: Vec<DevtoolsDomainSnapshot>,
    /// Bounded event records attached to targets or domains.
    pub events: Vec<DevtoolsEventRecord>,
    /// Legacy snapshots preserved for compatibility.
    pub snapshots: Vec<SnapshotEnvelope>,
    /// Capture-level diagnostics.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

impl DevtoolsCapture {
    /// Creates a sanitized capture from explicit parts.
    pub fn new(
        targets: DevtoolsTargetTree,
        domains: impl IntoIterator<Item = DevtoolsDomainSnapshot>,
        events: impl IntoIterator<Item = DevtoolsEventRecord>,
        snapshots: impl IntoIterator<Item = SnapshotEnvelope>,
        diagnostics: impl IntoIterator<Item = SnapshotDiagnostic>,
    ) -> Self {
        let targets = targets.sanitized().targets;
        let domains = domains
            .into_iter()
            .map(DevtoolsDomainSnapshot::sanitized)
            .collect::<Vec<_>>();
        let events = events
            .into_iter()
            .map(DevtoolsEventRecord::sanitized)
            .collect::<Vec<_>>();
        let snapshots = snapshots
            .into_iter()
            .map(SnapshotEnvelope::sanitized)
            .collect::<Vec<_>>();
        let mut diagnostics = diagnostics
            .into_iter()
            .map(SnapshotDiagnostic::sanitized)
            .collect::<Vec<_>>();
        diagnostics.extend(capture_identity_diagnostics(
            &targets, &domains, &events, &snapshots,
        ));
        let mut diagnostic_keys = BTreeSet::new();
        diagnostics.retain(|diagnostic| {
            diagnostic_keys.insert((
                diagnostic.probe_id.as_str().to_owned(),
                diagnostic.code.clone(),
                diagnostic.message.clone(),
            ))
        });
        Self {
            targets: DevtoolsTargetTree::new(targets),
            domains,
            events,
            snapshots,
            diagnostics,
        }
    }

    /// Creates a target/domain capture from a legacy snapshot collection.
    pub fn from_snapshot_collection(collection: SnapshotCollection) -> Self {
        let collection = collection.sanitized();
        let app_target_id = DevtoolsTargetId::from_parts(["app"]);
        let app_target = DevtoolsTargetSnapshot::new(
            app_target_id.clone(),
            DevtoolsTargetKind::App,
            "Application",
        )
        .with_metadata(serde_json::json!({
            "snapshot_count": collection.snapshots.len(),
            "diagnostic_count": collection.diagnostics.len(),
        }));

        let mut targets = vec![app_target];
        let mut domains = Vec::new();

        for snapshot in &collection.snapshots {
            let target_id = DevtoolsTargetId::from_probe_id(&snapshot.probe_id);
            targets.push(
                DevtoolsTargetSnapshot::new(
                    target_id.clone(),
                    DevtoolsTargetKind::Probe,
                    snapshot.probe_id.as_str(),
                )
                .parent_id(app_target_id.clone())
                .with_metadata(serde_json::json!({
                    "probe_id": snapshot.probe_id.as_str(),
                    "snapshot_kind": snapshot.kind.as_label(),
                })),
            );
            domains.push(DevtoolsDomainSnapshot::from_snapshot(
                target_id,
                snapshot.clone(),
            ));
        }

        Self::new(
            DevtoolsTargetTree::new(targets),
            domains,
            Vec::new(),
            collection.snapshots,
            collection.diagnostics,
        )
    }

    /// Returns a legacy snapshot collection view of this capture.
    pub fn snapshot_collection(&self) -> SnapshotCollection {
        SnapshotCollection {
            snapshots: self.snapshots.clone(),
            diagnostics: self.diagnostics.clone(),
        }
        .sanitized()
    }

    /// Returns this capture with every exported channel sanitized.
    pub fn sanitized(self) -> Self {
        Self::new(
            self.targets,
            self.domains,
            self.events,
            self.snapshots,
            self.diagnostics,
        )
    }
}

const CAPTURE_DIAGNOSTIC_PROBE_ID: &str = "devtools.capture";

fn capture_identity_diagnostics(
    targets: &[DevtoolsTargetSnapshot],
    domains: &[DevtoolsDomainSnapshot],
    events: &[DevtoolsEventRecord],
    snapshots: &[SnapshotEnvelope],
) -> Vec<SnapshotDiagnostic> {
    let mut diagnostics = Vec::new();

    let mut target_ids = BTreeSet::new();
    for target in targets {
        let id = target.id.as_str().to_owned();
        if !target_ids.insert(id.clone()) {
            diagnostics.push(capture_diagnostic(
                "capture.duplicate_target",
                format!("duplicate target id: {id}"),
            ));
        }
    }

    let mut domain_ids = BTreeSet::new();
    for domain in domains {
        let id = domain.id.as_str().to_owned();
        if !domain_ids.insert(id.clone()) {
            diagnostics.push(capture_diagnostic(
                "capture.duplicate_domain",
                format!("duplicate domain id: {id}"),
            ));
        }
        if !target_ids.contains(domain.target_id.as_str()) {
            diagnostics.push(capture_diagnostic(
                "capture.missing_domain_target",
                format!(
                    "domain {} references missing target {}",
                    domain.id.as_str(),
                    domain.target_id.as_str()
                ),
            ));
        }
    }

    for event in events {
        if let Some(target_id) = event.target_id_ref() {
            if !target_ids.contains(target_id.as_str()) {
                diagnostics.push(capture_diagnostic(
                    "capture.missing_event_target",
                    format!(
                        "event {} references missing target {}",
                        event.id(),
                        target_id.as_str()
                    ),
                ));
            }
        }
        if let Some(domain_id) = event.domain_id_ref() {
            if !domain_ids.contains(domain_id.as_str()) {
                diagnostics.push(capture_diagnostic(
                    "capture.missing_event_domain",
                    format!(
                        "event {} references missing domain {}",
                        event.id(),
                        domain_id.as_str()
                    ),
                ));
            }
        }
    }

    let mut probe_ids = BTreeSet::new();
    for snapshot in snapshots {
        let id = snapshot.probe_id.as_str().to_owned();
        if !probe_ids.insert(id.clone()) {
            diagnostics.push(capture_diagnostic(
                "capture.duplicate_probe",
                format!("duplicate legacy probe id: {id}"),
            ));
        }
    }

    diagnostics
}

fn capture_diagnostic(code: &'static str, message: String) -> SnapshotDiagnostic {
    let probe_id = ProbeId::new(CAPTURE_DIAGNOSTIC_PROBE_ID)
        .expect("internal capture diagnostic probe id is non-empty");
    SnapshotDiagnostic::new(probe_id, code, message)
}
