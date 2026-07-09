//! Sanitized capture diffing for DevTools session frames.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsCapture, DevtoolsDomainSnapshot, DevtoolsEventRecord, DevtoolsTargetSnapshot, ProbeId,
    SnapshotDiagnostic, SnapshotEnvelope,
    adapters::{sanitize_json_value, sanitize_sensitive_text},
};

/// Kind of capture item represented by a diff row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsDiffKind {
    /// Target tree entry.
    Target,
    /// Domain output.
    Domain,
    /// Event record.
    Event,
    /// Legacy snapshot envelope.
    Snapshot,
    /// Capture diagnostic.
    Diagnostic,
}

impl DevtoolsDiffKind {
    /// Returns the stable label for this diff kind.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Domain => "domain",
            Self::Event => "event",
            Self::Snapshot => "snapshot",
            Self::Diagnostic => "diagnostic",
        }
    }
}

/// Status of one capture diff row.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsDiffStatus {
    /// Item exists only in the current capture.
    Added,
    /// Item exists only in the previous capture.
    Removed,
    /// Item exists in both captures with different sanitized content.
    Changed,
    /// Item exists in both captures with identical sanitized content.
    Unchanged,
    /// Multiple sanitized items share one identity and cannot be collapsed safely.
    Collision,
}

impl DevtoolsDiffStatus {
    /// Returns the stable label for this diff status.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::Collision => "collision",
        }
    }
}

/// Aggregate counts for a capture diff.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsDiffSummary {
    /// Added row count.
    pub added: usize,
    /// Removed row count.
    pub removed: usize,
    /// Changed row count.
    pub changed: usize,
    /// Unchanged row count.
    pub unchanged: usize,
    /// Redaction or duplicate-identity collision count.
    pub collisions: usize,
}

impl DevtoolsDiffSummary {
    fn record(&mut self, status: DevtoolsDiffStatus) {
        match status {
            DevtoolsDiffStatus::Added => self.added += 1,
            DevtoolsDiffStatus::Removed => self.removed += 1,
            DevtoolsDiffStatus::Changed => self.changed += 1,
            DevtoolsDiffStatus::Unchanged => self.unchanged += 1,
            DevtoolsDiffStatus::Collision => self.collisions += 1,
        }
    }
}

/// One sanitized diff row between two captures.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsDiffRow {
    /// Capture category represented by this row.
    pub kind: DevtoolsDiffKind,
    /// Stable sanitized identity for the row.
    pub identity: String,
    /// Human-readable sanitized label.
    pub label: String,
    /// Diff status.
    pub status: DevtoolsDiffStatus,
    /// Previous sanitized value, if available.
    pub previous: Option<serde_json::Value>,
    /// Current sanitized value, if available.
    pub current: Option<serde_json::Value>,
    /// Diagnostic attached to collision rows.
    pub diagnostic: Option<SnapshotDiagnostic>,
}

impl DevtoolsDiffRow {
    fn new(
        kind: DevtoolsDiffKind,
        identity: impl Into<String>,
        label: impl Into<String>,
        status: DevtoolsDiffStatus,
        previous: Option<serde_json::Value>,
        current: Option<serde_json::Value>,
    ) -> Self {
        let identity = sanitize_sensitive_text(&identity.into());
        let label = sanitize_sensitive_text(&label.into());
        let diagnostic = (status == DevtoolsDiffStatus::Collision).then(|| {
            diff_diagnostic(format!(
                "{} identity collision: {}",
                kind.as_label(),
                identity
            ))
        });

        Self {
            kind,
            identity,
            label,
            status,
            previous,
            current,
            diagnostic,
        }
    }
}

/// Sanitized diff between two DevTools captures.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsCaptureDiff {
    /// Diff rows in deterministic kind and identity order.
    pub rows: Vec<DevtoolsDiffRow>,
    /// Aggregate row counts.
    pub summary: DevtoolsDiffSummary,
}

impl DevtoolsCaptureDiff {
    /// Computes a sanitized diff from `previous` to `current`.
    pub fn between(previous: &DevtoolsCapture, current: &DevtoolsCapture) -> Self {
        let previous = previous.clone().sanitized();
        let current = current.clone().sanitized();
        let mut diff = Self::default();

        diff_category(
            DevtoolsDiffKind::Target,
            target_entries(&previous.targets.targets),
            target_entries(&current.targets.targets),
            &mut diff,
        );
        diff_category(
            DevtoolsDiffKind::Domain,
            domain_entries(&previous.domains),
            domain_entries(&current.domains),
            &mut diff,
        );
        diff_category(
            DevtoolsDiffKind::Event,
            event_entries(&previous.events),
            event_entries(&current.events),
            &mut diff,
        );
        diff_category(
            DevtoolsDiffKind::Snapshot,
            snapshot_entries(&previous.snapshots),
            snapshot_entries(&current.snapshots),
            &mut diff,
        );
        diff_category(
            DevtoolsDiffKind::Diagnostic,
            diagnostic_entries(&previous.diagnostics),
            diagnostic_entries(&current.diagnostics),
            &mut diff,
        );

        diff
    }

    /// Returns true when no added, removed, changed, or collision rows exist.
    pub const fn is_empty(&self) -> bool {
        self.summary.added == 0
            && self.summary.removed == 0
            && self.summary.changed == 0
            && self.summary.collisions == 0
    }
}

impl DevtoolsCapture {
    /// Computes a sanitized diff from `previous` to this capture.
    pub fn diff_from(&self, previous: &Self) -> DevtoolsCaptureDiff {
        DevtoolsCaptureDiff::between(previous, self)
    }
}

#[derive(Clone, Debug)]
struct DiffEntry {
    label: String,
    value: serde_json::Value,
}

type EntryMap = BTreeMap<String, Vec<DiffEntry>>;

fn diff_category(
    kind: DevtoolsDiffKind,
    previous: EntryMap,
    current: EntryMap,
    diff: &mut DevtoolsCaptureDiff,
) {
    let keys = previous
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for key in keys {
        let previous_entries = previous.get(&key).cloned().unwrap_or_default();
        let current_entries = current.get(&key).cloned().unwrap_or_default();
        let previous_value = entries_value(&previous_entries);
        let current_value = entries_value(&current_entries);
        let status = if previous_entries.len() > 1 || current_entries.len() > 1 {
            DevtoolsDiffStatus::Collision
        } else {
            match (previous_value.as_ref(), current_value.as_ref()) {
                (None, Some(_)) => DevtoolsDiffStatus::Added,
                (Some(_), None) => DevtoolsDiffStatus::Removed,
                (Some(previous), Some(current)) if previous == current => {
                    DevtoolsDiffStatus::Unchanged
                }
                (Some(_), Some(_)) => DevtoolsDiffStatus::Changed,
                (None, None) => continue,
            }
        };
        let label = current_entries
            .first()
            .or_else(|| previous_entries.first())
            .map(|entry| entry.label.clone())
            .unwrap_or_else(|| key.clone());

        diff.summary.record(status);
        diff.rows.push(DevtoolsDiffRow::new(
            kind,
            key,
            label,
            status,
            previous_value,
            current_value,
        ));
    }
}

fn target_entries(targets: &[DevtoolsTargetSnapshot]) -> EntryMap {
    let mut entries = EntryMap::new();
    for target in targets {
        push_entry(
            &mut entries,
            target.id.as_str(),
            &target.label,
            to_sanitized_value(target),
        );
    }
    entries
}

fn domain_entries(domains: &[DevtoolsDomainSnapshot]) -> EntryMap {
    let mut entries = EntryMap::new();
    for domain in domains {
        push_entry(
            &mut entries,
            domain.id.as_str(),
            &domain.label,
            to_sanitized_value(domain),
        );
    }
    entries
}

fn event_entries(events: &[DevtoolsEventRecord]) -> EntryMap {
    let mut entries = EntryMap::new();
    for event in events {
        let identity = event.identity();
        push_entry(
            &mut entries,
            identity.as_key(),
            event.label(),
            to_sanitized_value(event),
        );
    }
    entries
}

fn snapshot_entries(snapshots: &[SnapshotEnvelope]) -> EntryMap {
    let mut entries = EntryMap::new();
    for snapshot in snapshots {
        let identity = format!(
            "{}:{}",
            snapshot.probe_id.as_str(),
            snapshot.kind.as_label()
        );
        push_entry(
            &mut entries,
            identity,
            snapshot.kind.as_label(),
            to_sanitized_value(snapshot),
        );
    }
    entries
}

fn diagnostic_entries(diagnostics: &[SnapshotDiagnostic]) -> EntryMap {
    let mut entries = EntryMap::new();
    for diagnostic in diagnostics {
        push_entry(
            &mut entries,
            format!("{}:{}", diagnostic.probe_id.as_str(), diagnostic.code),
            &diagnostic.code,
            to_sanitized_value(diagnostic),
        );
    }
    entries
}

fn push_entry(
    entries: &mut EntryMap,
    identity: impl Into<String>,
    label: impl Into<String>,
    value: serde_json::Value,
) {
    entries
        .entry(sanitize_sensitive_text(&identity.into()))
        .or_default()
        .push(DiffEntry {
            label: sanitize_sensitive_text(&label.into()),
            value,
        });
}

fn entries_value(entries: &[DiffEntry]) -> Option<serde_json::Value> {
    match entries {
        [] => None,
        [entry] => Some(entry.value.clone()),
        many => Some(serde_json::Value::Array(
            many.iter()
                .map(|entry| {
                    serde_json::json!({
                        "label": entry.label,
                        "value": entry.value,
                    })
                })
                .collect(),
        )),
    }
}

fn to_sanitized_value<T>(value: &T) -> serde_json::Value
where
    T: Serialize,
{
    serde_json::to_value(value)
        .map(sanitize_json_value)
        .unwrap_or_else(|error| {
            serde_json::json!({
                "serialization_error": sanitize_sensitive_text(&error.to_string()),
            })
        })
}

fn diff_diagnostic(message: String) -> SnapshotDiagnostic {
    SnapshotDiagnostic::new(diff_probe_id(), "diff.identity_collision", message)
}

fn diff_probe_id() -> ProbeId {
    ProbeId::new("devtools.diff").expect("internal diff diagnostic probe id is non-empty")
}
