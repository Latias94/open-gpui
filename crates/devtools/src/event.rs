//! Bounded local event records for DevTools captures.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsDomainId, DevtoolsTargetId,
    adapters::{sanitize_json_value, sanitize_sensitive_text, stable_node_id},
};

/// Default maximum number of events retained by a DevTools event recorder.
pub const DEFAULT_DEVTOOLS_EVENT_LIMIT: usize = 256;

/// Default event scope id used by a recorder created without an explicit scope.
pub const DEFAULT_DEVTOOLS_EVENT_SCOPE_ID: &str = "app";

/// Default event scope label used by a recorder created without an explicit scope.
pub const DEFAULT_DEVTOOLS_EVENT_SCOPE_LABEL: &str = "Application";

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

/// Stable event-instance identity for diff, replay, and inspector selection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DevtoolsEventIdentity {
    /// Sanitized event scope id.
    pub scope_id: String,
    /// Recorder-assigned event sequence.
    pub sequence: u64,
    /// Sanitized event id.
    pub event_id: String,
}

impl DevtoolsEventIdentity {
    /// Creates a sanitized event identity.
    pub fn new(scope_id: impl Into<String>, sequence: u64, event_id: impl Into<String>) -> Self {
        let scope_id = sanitize_sensitive_text(&scope_id.into());
        let event_id = sanitize_sensitive_text(&event_id.into());
        Self {
            scope_id: if scope_id.trim().is_empty() {
                DEFAULT_DEVTOOLS_EVENT_SCOPE_ID.to_owned()
            } else {
                scope_id
            },
            sequence,
            event_id: if event_id.trim().is_empty() {
                "event".to_owned()
            } else {
                event_id
            },
        }
    }

    /// Builds the stable identity for an event record.
    pub fn from_event(event: &DevtoolsEventRecord) -> Self {
        Self::new(
            event
                .scope_id_ref()
                .unwrap_or(DEFAULT_DEVTOOLS_EVENT_SCOPE_ID),
            event.sequence(),
            event.id(),
        )
    }

    /// Returns a deterministic sanitized key for maps and UI rows.
    pub fn as_key(&self) -> String {
        let sequence = self.sequence.to_string();
        stable_node_id([
            self.scope_id.as_str(),
            sequence.as_str(),
            self.event_id.as_str(),
        ])
    }
}

impl std::fmt::Display for DevtoolsEventIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_key())
    }
}

/// One sanitized event record exported by DevTools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsEventRecord {
    sequence: u64,
    scope_id: Option<String>,
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
            scope_id: None,
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

    /// Attaches an event scope id.
    pub fn scope_id(mut self, scope_id: impl Into<String>) -> Self {
        self.scope_id = Some(sanitize_sensitive_text(&scope_id.into()));
        self
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

    /// Returns the event scope id, if present.
    pub fn scope_id_ref(&self) -> Option<&str> {
        self.scope_id.as_deref()
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

    /// Returns the stable event-instance identity used by diff, replay, and inspector selection.
    pub fn identity(&self) -> DevtoolsEventIdentity {
        DevtoolsEventIdentity::from_event(self)
    }

    /// Returns this event with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.scope_id = self
            .scope_id
            .map(|scope_id| sanitize_sensitive_text(&scope_id));
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

    fn with_default_scope(mut self, scope_id: &str) -> Self {
        if self.scope_id.is_none() {
            self.scope_id = Some(sanitize_sensitive_text(scope_id));
        }
        self
    }
}

/// Exported event batch from a bounded recorder.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsEventBatch {
    /// Sanitized scope id for this batch.
    pub scope_id: String,
    /// Human-readable sanitized scope label.
    pub scope_label: String,
    /// Events retained by the recorder.
    pub events: Vec<DevtoolsEventRecord>,
    /// Maximum event capacity used by the recorder.
    pub max_events: usize,
    /// Number of events retained in this batch.
    pub retained_events: usize,
    /// Number of older events omitted due to capacity.
    pub omitted_events: usize,
    /// Next append-time sequence that would be assigned by the recorder.
    pub next_sequence: u64,
}

impl DevtoolsEventBatch {
    /// Creates a sanitized event batch with the default application scope.
    pub fn new(
        events: impl IntoIterator<Item = DevtoolsEventRecord>,
        max_events: usize,
        omitted_events: usize,
    ) -> Self {
        Self::for_scope(
            DEFAULT_DEVTOOLS_EVENT_SCOPE_ID,
            DEFAULT_DEVTOOLS_EVENT_SCOPE_LABEL,
            events,
            max_events,
            omitted_events,
            0,
        )
    }

    /// Creates a sanitized event batch for an explicit scope.
    pub fn for_scope(
        scope_id: impl Into<String>,
        scope_label: impl Into<String>,
        events: impl IntoIterator<Item = DevtoolsEventRecord>,
        max_events: usize,
        omitted_events: usize,
        next_sequence: u64,
    ) -> Self {
        let scope_id = sanitize_sensitive_text(&scope_id.into());
        let events = events
            .into_iter()
            .map(|event| event.sanitized().with_default_scope(&scope_id))
            .collect::<Vec<_>>();
        let retained_events = events.len();
        Self {
            scope_id,
            scope_label: sanitize_sensitive_text(&scope_label.into()),
            events,
            max_events,
            retained_events,
            omitted_events,
            next_sequence,
        }
    }

    /// Merges multiple event batches into one deterministic sanitized batch.
    pub fn merged(
        scope_id: impl Into<String>,
        scope_label: impl Into<String>,
        batches: impl IntoIterator<Item = DevtoolsEventBatch>,
    ) -> Self {
        let mut max_events = 0usize;
        let mut omitted_events = 0usize;
        let mut next_sequence = 0u64;
        let mut events = Vec::new();

        for batch in batches {
            let batch = batch.sanitized();
            max_events = max_events.saturating_add(batch.max_events);
            omitted_events = omitted_events.saturating_add(batch.omitted_events);
            next_sequence = next_sequence.max(batch.next_sequence);
            events.extend(batch.events);
        }

        events.sort_by(|left, right| {
            left.sequence()
                .cmp(&right.sequence())
                .then_with(|| left.scope_id_ref().cmp(&right.scope_id_ref()))
                .then_with(|| left.id().cmp(right.id()))
        });

        Self::for_scope(
            scope_id,
            scope_label,
            events,
            max_events.max(1),
            omitted_events,
            next_sequence,
        )
    }

    /// Returns this batch with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.scope_id = sanitize_sensitive_text(&self.scope_id);
        self.scope_label = sanitize_sensitive_text(&self.scope_label);
        self.events = self
            .events
            .into_iter()
            .map(|event| event.sanitized().with_default_scope(&self.scope_id))
            .collect();
        self.retained_events = self.events.len();
        self
    }
}

/// Bounded in-memory recorder for local DevTools events.
#[derive(Clone, Debug)]
pub struct DevtoolsEventRecorder {
    scope_id: String,
    scope_label: String,
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
    /// Creates an event recorder for an explicit application/session scope.
    pub fn new(
        scope_id: impl Into<String>,
        scope_label: impl Into<String>,
        max_events: usize,
    ) -> Self {
        Self {
            scope_id: sanitize_sensitive_text(&scope_id.into()),
            scope_label: sanitize_sensitive_text(&scope_label.into()),
            max_events: max_events.max(1),
            next_sequence: 0,
            omitted_events: 0,
            events: VecDeque::new(),
        }
    }

    /// Creates an event recorder with a bounded capacity.
    pub fn with_capacity(max_events: usize) -> Self {
        Self::new(
            DEFAULT_DEVTOOLS_EVENT_SCOPE_ID,
            DEFAULT_DEVTOOLS_EVENT_SCOPE_LABEL,
            max_events,
        )
    }

    /// Returns the recorder scope id.
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    /// Returns the recorder scope label.
    pub fn scope_label(&self) -> &str {
        &self.scope_label
    }

    /// Returns the recorder capacity.
    pub const fn max_events(&self) -> usize {
        self.max_events
    }

    /// Returns how many events are currently retained.
    pub fn retained_events(&self) -> usize {
        self.events.len()
    }

    /// Returns how many older events were omitted.
    pub const fn omitted_events(&self) -> usize {
        self.omitted_events
    }

    /// Returns the next append-time sequence.
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Returns true when no events are retained.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Records one event and returns its assigned sequence.
    pub fn record(&mut self, event: DevtoolsEventRecord) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);

        if self.events.len() == self.max_events {
            self.events.pop_front();
            self.omitted_events = self.omitted_events.saturating_add(1);
        }

        self.events.push_back(
            event
                .sanitized()
                .with_default_scope(&self.scope_id)
                .with_sequence(sequence),
        );
        sequence
    }

    /// Exports a sanitized batch without clearing the recorder.
    pub fn snapshot(&self) -> DevtoolsEventBatch {
        DevtoolsEventBatch::for_scope(
            self.scope_id.clone(),
            self.scope_label.clone(),
            self.events.iter().cloned(),
            self.max_events,
            self.omitted_events,
            self.next_sequence,
        )
    }

    /// Exports a sanitized batch without clearing the recorder.
    pub fn export(&self) -> DevtoolsEventBatch {
        self.snapshot()
    }

    /// Exports a sanitized batch and clears retained events and omission counts.
    pub fn drain(&mut self) -> DevtoolsEventBatch {
        let batch = self.snapshot();
        self.events.clear();
        self.omitted_events = 0;
        batch
    }

    /// Clears retained events and omission counts.
    pub fn clear(&mut self) {
        self.events.clear();
        self.omitted_events = 0;
    }
}
