//! Bounded local event records for DevTools captures.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsDomainId, DevtoolsTargetId,
    adapters::{sanitize_json_value, sanitize_sensitive_text},
};

/// Default maximum number of events retained by a DevTools event recorder.
pub const DEFAULT_DEVTOOLS_EVENT_LIMIT: usize = 256;

/// Kind of event recorded by DevTools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsEventKind {
    /// Point-in-time event.
    Instant,
    /// Event with a known duration.
    Duration,
    /// Diagnostic event.
    Diagnostic,
    /// Custom producer-provided event kind.
    Custom(String),
}

impl DevtoolsEventKind {
    /// Returns the stable display label for this event kind.
    pub fn as_label(&self) -> &str {
        match self {
            Self::Instant => "instant",
            Self::Duration => "duration",
            Self::Diagnostic => "diagnostic",
            Self::Custom(label) => label.as_str(),
        }
    }

    /// Returns this event kind with custom labels sanitized.
    pub fn sanitized(self) -> Self {
        match self {
            Self::Custom(label) => Self::Custom(sanitize_sensitive_text(&label)),
            other => other,
        }
    }
}

/// One sanitized event record exported by DevTools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsEventRecord {
    sequence: u64,
    id: String,
    label: String,
    kind: DevtoolsEventKind,
    target_id: Option<DevtoolsTargetId>,
    domain_id: Option<DevtoolsDomainId>,
    timestamp_ms: Option<u64>,
    duration_ms: Option<u64>,
    payload: Option<serde_json::Value>,
}

impl DevtoolsEventRecord {
    /// Creates a sanitized event record.
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: DevtoolsEventKind) -> Self {
        Self {
            sequence: 0,
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            kind: kind.sanitized(),
            target_id: None,
            domain_id: None,
            timestamp_ms: None,
            duration_ms: None,
            payload: None,
        }
    }

    /// Attaches a target id.
    pub fn target_id(mut self, target_id: DevtoolsTargetId) -> Self {
        self.target_id = Some(target_id);
        self
    }

    /// Attaches a domain id.
    pub fn domain_id(mut self, domain_id: DevtoolsDomainId) -> Self {
        self.domain_id = Some(domain_id);
        self
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

    /// Attaches sanitized event payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(sanitize_json_value(payload));
        self
    }

    /// Returns the event sequence assigned by the recorder.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the sanitized event id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the sanitized label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the event kind.
    pub const fn kind(&self) -> &DevtoolsEventKind {
        &self.kind
    }

    /// Returns the target id, if present.
    pub const fn target_id_ref(&self) -> Option<&DevtoolsTargetId> {
        self.target_id.as_ref()
    }

    /// Returns the domain id, if present.
    pub const fn domain_id_ref(&self) -> Option<&DevtoolsDomainId> {
        self.domain_id.as_ref()
    }

    /// Returns the optional timestamp in milliseconds.
    pub const fn timestamp_ms_value(&self) -> Option<u64> {
        self.timestamp_ms
    }

    /// Returns the optional duration in milliseconds.
    pub const fn duration_ms_value(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Returns the optional sanitized payload.
    pub const fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
    }

    /// Returns this event with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.id = sanitize_sensitive_text(&self.id);
        self.label = sanitize_sensitive_text(&self.label);
        self.kind = self.kind.sanitized();
        self.target_id = self
            .target_id
            .map(|target_id| DevtoolsTargetId::new(target_id.as_str()));
        self.domain_id = self
            .domain_id
            .map(|domain_id| DevtoolsDomainId::new(domain_id.as_str()));
        self.payload = self.payload.map(sanitize_json_value);
        self
    }

    fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }
}

/// Exported event batch from a bounded recorder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsEventBatch {
    /// Events retained by the recorder.
    pub events: Vec<DevtoolsEventRecord>,
    /// Maximum event capacity used by the recorder.
    pub max_events: usize,
    /// Number of older events omitted due to capacity.
    pub omitted_events: usize,
}

impl DevtoolsEventBatch {
    /// Creates a sanitized event batch.
    pub fn new(
        events: impl IntoIterator<Item = DevtoolsEventRecord>,
        max_events: usize,
        omitted_events: usize,
    ) -> Self {
        Self {
            events: events
                .into_iter()
                .map(DevtoolsEventRecord::sanitized)
                .collect(),
            max_events,
            omitted_events,
        }
    }

    /// Returns this batch with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.events = self
            .events
            .into_iter()
            .map(DevtoolsEventRecord::sanitized)
            .collect();
        self
    }
}

/// Bounded in-memory recorder for local DevTools events.
#[derive(Clone, Debug)]
pub struct DevtoolsEventRecorder {
    max_events: usize,
    next_sequence: u64,
    omitted_events: usize,
    events: VecDeque<DevtoolsEventRecord>,
}

impl Default for DevtoolsEventRecorder {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_DEVTOOLS_EVENT_LIMIT)
    }
}

impl DevtoolsEventRecorder {
    /// Creates an event recorder with a bounded capacity.
    pub fn with_capacity(max_events: usize) -> Self {
        Self {
            max_events: max_events.max(1),
            next_sequence: 0,
            omitted_events: 0,
            events: VecDeque::new(),
        }
    }

    /// Returns the recorder capacity.
    pub const fn max_events(&self) -> usize {
        self.max_events
    }

    /// Returns how many older events were omitted.
    pub const fn omitted_events(&self) -> usize {
        self.omitted_events
    }

    /// Records one event and returns its assigned sequence.
    pub fn record(&mut self, event: DevtoolsEventRecord) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);

        if self.events.len() == self.max_events {
            self.events.pop_front();
            self.omitted_events = self.omitted_events.saturating_add(1);
        }

        self.events
            .push_back(event.sanitized().with_sequence(sequence));
        sequence
    }

    /// Exports a sanitized batch without clearing the recorder.
    pub fn snapshot(&self) -> DevtoolsEventBatch {
        DevtoolsEventBatch::new(
            self.events.iter().cloned(),
            self.max_events,
            self.omitted_events,
        )
    }

    /// Clears retained events and omission counts.
    pub fn clear(&mut self) {
        self.events.clear();
        self.omitted_events = 0;
    }
}
