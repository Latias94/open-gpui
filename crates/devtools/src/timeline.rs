//! Renderer-neutral DevTools timeline snapshots.

use crate::{
    ProbeId, ProbeSnapshotError, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbe,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_json_value, sanitize_sensitive_text, snapshot_node_with_payload},
};

/// Default upper bound for events exported by one timeline snapshot.
pub const DEFAULT_TIMELINE_EVENT_LIMIT: usize = 256;

/// One event exported into a DevTools timeline snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEventSnapshot {
    /// Stable event id within the snapshot.
    pub id: String,
    /// Human-readable event label.
    pub label: String,
    /// Event category, such as `motion`, `layout`, or `input`.
    pub category: String,
    /// Stable event order within the snapshot.
    pub order: u64,
    /// Optional timestamp in milliseconds from the producer's clock origin.
    pub timestamp_ms: Option<u64>,
    /// Optional duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Optional sanitized event payload.
    pub payload: Option<serde_json::Value>,
}

impl TimelineEventSnapshot {
    /// Creates a sanitized timeline event with stable ordering.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        category: impl Into<String>,
        order: u64,
    ) -> Self {
        Self {
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            category: sanitize_sensitive_text(&category.into()),
            order,
            timestamp_ms: None,
            duration_ms: None,
            payload: None,
        }
    }

    /// Attaches a producer timestamp in milliseconds.
    pub const fn timestamp_ms(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = Some(timestamp_ms);
        self
    }

    /// Attaches an event duration in milliseconds.
    pub const fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Attaches a sanitized JSON payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(sanitize_json_value(payload));
        self
    }

    /// Returns the event with every exported string and payload sanitized.
    pub fn sanitized(mut self) -> Self {
        self.id = sanitize_sensitive_text(&self.id);
        self.label = sanitize_sensitive_text(&self.label);
        self.category = sanitize_sensitive_text(&self.category);
        self.payload = self.payload.map(sanitize_json_value);
        self
    }
}

/// Bounded timeline snapshot ready to convert into a DevTools snapshot tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineSnapshot {
    id: String,
    label: String,
    events: Vec<TimelineEventSnapshot>,
    max_events: usize,
    omitted_events: usize,
}

impl TimelineSnapshot {
    /// Creates a timeline snapshot using `DEFAULT_TIMELINE_EVENT_LIMIT`.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        events: impl IntoIterator<Item = TimelineEventSnapshot>,
    ) -> Self {
        Self::with_event_limit(id, label, events, DEFAULT_TIMELINE_EVENT_LIMIT)
    }

    /// Creates a timeline snapshot with a producer-provided event limit.
    pub fn with_event_limit(
        id: impl Into<String>,
        label: impl Into<String>,
        events: impl IntoIterator<Item = TimelineEventSnapshot>,
        max_events: usize,
    ) -> Self {
        let max_events = max_events.max(1);
        let events = events
            .into_iter()
            .map(TimelineEventSnapshot::sanitized)
            .collect::<Vec<_>>();
        let omitted_events = events.len().saturating_sub(max_events);
        let events = events.into_iter().take(max_events).collect();

        Self {
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            events,
            max_events,
            omitted_events,
        }
    }

    /// Returns the sanitized timeline id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the sanitized timeline label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns events retained by the bounded snapshot.
    pub fn events(&self) -> &[TimelineEventSnapshot] {
        &self.events
    }

    /// Returns the event limit applied by this snapshot.
    pub const fn max_events(&self) -> usize {
        self.max_events
    }

    /// Returns how many input events were omitted by the event limit.
    pub const fn omitted_events(&self) -> usize {
        self.omitted_events
    }

    /// Converts this timeline into a sanitized DevTools tree.
    pub fn tree(&self) -> SnapshotTree {
        let mut root = snapshot_node_with_payload(
            ["timeline", self.id.as_str()],
            self.label.as_str(),
            serde_json::json!({
                "id": self.id,
                "label": self.label,
                "event_count": self.events.len(),
                "max_events": self.max_events,
                "omitted_events": self.omitted_events,
            }),
        );

        for event in &self.events {
            root = root.with_child(timeline_event_node(self, event));
        }

        SnapshotTree::new([root])
    }

    /// Converts this timeline into a probe snapshot.
    pub fn probe_snapshot(&self) -> SnapshotProbeSnapshot {
        SnapshotProbeSnapshot::new(self.tree()).with_redaction(SnapshotRedactionSummary::default())
    }

    /// Converts this timeline into an envelope.
    pub fn envelope(&self, probe_id: ProbeId) -> SnapshotEnvelope {
        SnapshotEnvelope::new(probe_id, SnapshotKind::Timeline, self.tree())
            .with_redaction(SnapshotRedactionSummary::default())
    }
}

/// Converts a timeline snapshot into a probe snapshot.
pub fn timeline_probe_snapshot(snapshot: &TimelineSnapshot) -> SnapshotProbeSnapshot {
    snapshot.probe_snapshot()
}

/// Converts a timeline snapshot into an envelope.
pub fn timeline_snapshot_envelope(
    probe_id: ProbeId,
    snapshot: &TimelineSnapshot,
) -> SnapshotEnvelope {
    snapshot.envelope(probe_id)
}

/// Builds a closure-backed timeline snapshot probe.
pub fn timeline_snapshot_probe<F>(
    id: impl Into<String>,
    snapshot: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> TimelineSnapshot + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Timeline, move || {
        Ok(timeline_probe_snapshot(&snapshot()))
    })
}

fn timeline_event_node(timeline: &TimelineSnapshot, event: &TimelineEventSnapshot) -> SnapshotNode {
    let order = event.order.to_string();
    snapshot_node_with_payload(
        [
            "timeline",
            timeline.id.as_str(),
            order.as_str(),
            event.id.as_str(),
        ],
        event.label.as_str(),
        serde_json::json!({
            "id": event.id,
            "label": event.label,
            "category": event.category,
            "order": event.order,
            "timestamp_ms": event.timestamp_ms,
            "duration_ms": event.duration_ms,
            "payload": event.payload,
        }),
    )
}
