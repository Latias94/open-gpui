//! Local read-only DevTools runtime sessions.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    DevtoolsCapture, DevtoolsCaptureDiff, DevtoolsRegistry, adapters::sanitize_sensitive_text,
};

/// Schema version used by serialized DevTools session exports.
pub const DEVTOOLS_SESSION_SCHEMA_VERSION: &str = "open-gpui-devtools-session/v1";

/// Local in-process protocol version used by DevTools sessions.
pub const DEVTOOLS_SESSION_PROTOCOL_VERSION: &str = "open-gpui-devtools-local/v1";

/// Default number of session frames retained in memory.
pub const DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT: usize = 8;

/// Connection state for a local DevTools session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsSessionConnectionState {
    /// Session is being initialized.
    Opening,
    /// Session can refresh captures from its registry.
    Connected,
    /// Session is closing.
    Closing,
    /// Session is closed and no longer refreshes.
    Closed,
}

impl DevtoolsSessionConnectionState {
    /// Returns the stable label for this state.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Connected => "connected",
            Self::Closing => "closing",
            Self::Closed => "closed",
        }
    }
}

/// One captured frame in a DevTools session history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsSessionFrame {
    /// Sanitized session id.
    pub session_id: String,
    /// Monotonic generation assigned by the session.
    pub generation: u64,
    /// Previous retained generation, if one existed at refresh time.
    pub previous_generation: Option<u64>,
    /// Sanitized capture for this generation.
    pub capture: DevtoolsCapture,
    /// Sanitized diff from the previous frame.
    pub diff_from_previous: Option<DevtoolsCaptureDiff>,
}

impl DevtoolsSessionFrame {
    fn new(
        session_id: impl Into<String>,
        generation: u64,
        previous: Option<&DevtoolsSessionFrame>,
        capture: DevtoolsCapture,
    ) -> Self {
        let capture = capture.sanitized();
        let (previous_generation, diff_from_previous) = previous
            .map(|previous| {
                (
                    Some(previous.generation),
                    Some(capture.diff_from(&previous.capture)),
                )
            })
            .unwrap_or((None, None));

        Self {
            session_id: sanitize_session_id(session_id),
            generation,
            previous_generation,
            capture,
            diff_from_previous,
        }
    }

    fn sanitized(mut self) -> Self {
        self.session_id = sanitize_session_id(self.session_id);
        self.capture = self.capture.sanitized();
        self
    }
}

/// Serialized DevTools session export.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsSessionExport {
    /// Export schema version.
    pub schema_version: String,
    /// Local protocol version.
    pub protocol_version: String,
    /// Sanitized session id.
    pub session_id: String,
    /// Connection state at export time.
    pub connection_state: DevtoolsSessionConnectionState,
    /// Configured frame history limit.
    pub history_limit: usize,
    /// Number of frames retained in this export.
    pub retained_frames: usize,
    /// Current generation, if a frame exists.
    pub current_generation: Option<u64>,
    /// Retained sanitized frames.
    pub frames: Vec<DevtoolsSessionFrame>,
}

impl DevtoolsSessionExport {
    /// Parses, validates, canonicalizes, and re-sanitizes a session export JSON string.
    pub fn from_json_str(
        json: &str,
        limits: DevtoolsSessionImportLimits,
    ) -> Result<Self, DevtoolsSessionImportError> {
        if json.len() > limits.max_json_bytes {
            return Err(DevtoolsSessionImportError::JsonTooLarge {
                max_bytes: limits.max_json_bytes,
                actual_bytes: json.len(),
            });
        }
        let export = serde_json::from_str::<Self>(json)?;
        export.validate_import(limits)
    }

    /// Validates, canonicalizes, and re-sanitizes a deserialized session export.
    pub fn validate_import(
        mut self,
        limits: DevtoolsSessionImportLimits,
    ) -> Result<Self, DevtoolsSessionImportError> {
        self.schema_version = sanitize_sensitive_text(&self.schema_version);
        self.protocol_version = sanitize_sensitive_text(&self.protocol_version);
        self.session_id = sanitize_session_id(self.session_id);

        if self.schema_version != DEVTOOLS_SESSION_SCHEMA_VERSION {
            return Err(DevtoolsSessionImportError::UnsupportedSchema {
                expected: DEVTOOLS_SESSION_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.protocol_version != DEVTOOLS_SESSION_PROTOCOL_VERSION {
            return Err(DevtoolsSessionImportError::UnsupportedProtocol {
                expected: DEVTOOLS_SESSION_PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.frames.len() > limits.max_frames {
            return Err(DevtoolsSessionImportError::TooManyFrames {
                max_frames: limits.max_frames,
                actual_frames: self.frames.len(),
            });
        }

        let mut previous_frame: Option<DevtoolsSessionFrame> = None;
        let mut frames = Vec::with_capacity(self.frames.len());
        for frame in self.frames {
            if frame.capture.events.len() > limits.max_events_per_frame {
                return Err(DevtoolsSessionImportError::TooManyEvents {
                    generation: frame.generation,
                    max_events: limits.max_events_per_frame,
                    actual_events: frame.capture.events.len(),
                });
            }
            let frame = DevtoolsSessionFrame::new(
                self.session_id.clone(),
                frame.generation,
                previous_frame.as_ref(),
                frame.capture,
            );
            previous_frame = Some(frame.clone());
            frames.push(frame);
        }

        self.current_generation = frames.last().map(|frame| frame.generation);
        self.retained_frames = frames.len();
        self.history_limit = self.history_limit.max(1).min(limits.max_frames.max(1));
        self.frames = frames;
        Ok(self)
    }

    fn new(
        session_id: impl Into<String>,
        connection_state: DevtoolsSessionConnectionState,
        history_limit: usize,
        frames: impl IntoIterator<Item = DevtoolsSessionFrame>,
    ) -> Self {
        let frames = frames
            .into_iter()
            .map(DevtoolsSessionFrame::sanitized)
            .collect::<Vec<_>>();
        let retained_frames = frames.len();
        let current_generation = frames.last().map(|frame| frame.generation);
        Self {
            schema_version: DEVTOOLS_SESSION_SCHEMA_VERSION.to_owned(),
            protocol_version: DEVTOOLS_SESSION_PROTOCOL_VERSION.to_owned(),
            session_id: sanitize_session_id(session_id),
            connection_state,
            history_limit: history_limit.max(1),
            retained_frames,
            current_generation,
            frames,
        }
    }
}

/// Import bounds applied before replaying an offline session export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevtoolsSessionImportLimits {
    /// Maximum JSON payload bytes accepted by the import path.
    pub max_json_bytes: usize,
    /// Maximum frames accepted in an export.
    pub max_frames: usize,
    /// Maximum event records accepted per frame.
    pub max_events_per_frame: usize,
}

impl Default for DevtoolsSessionImportLimits {
    fn default() -> Self {
        Self {
            max_json_bytes: 4 * 1024 * 1024,
            max_frames: DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT,
            max_events_per_frame: 2048,
        }
    }
}

/// Local read-only runtime session over a DevTools registry.
pub struct DevtoolsSession {
    session_id: String,
    registry: DevtoolsRegistry,
    connection_state: DevtoolsSessionConnectionState,
    history_limit: usize,
    next_generation: u64,
    frames: VecDeque<DevtoolsSessionFrame>,
}

impl Default for DevtoolsSession {
    fn default() -> Self {
        Self::new("devtools.session", DevtoolsRegistry::default())
    }
}

impl DevtoolsSession {
    /// Creates a connected local session over a registry.
    pub fn new(session_id: impl Into<String>, registry: DevtoolsRegistry) -> Self {
        Self {
            session_id: sanitize_session_id(session_id),
            registry,
            connection_state: DevtoolsSessionConnectionState::Connected,
            history_limit: DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT,
            next_generation: 1,
            frames: VecDeque::new(),
        }
    }

    /// Sets the bounded frame history limit.
    pub fn with_history_limit(mut self, history_limit: usize) -> Self {
        self.history_limit = history_limit.max(1);
        self.truncate_history();
        self
    }

    /// Returns the sanitized session id.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the connection state.
    pub const fn connection_state(&self) -> DevtoolsSessionConnectionState {
        self.connection_state
    }

    /// Returns the configured history limit.
    pub const fn history_limit(&self) -> usize {
        self.history_limit
    }

    /// Returns the next generation that will be assigned.
    pub const fn next_generation(&self) -> u64 {
        self.next_generation
    }

    /// Returns the underlying registry.
    pub const fn registry(&self) -> &DevtoolsRegistry {
        &self.registry
    }

    /// Returns the underlying registry for app-owned registration changes.
    pub fn registry_mut(&mut self) -> &mut DevtoolsRegistry {
        &mut self.registry
    }

    /// Consumes the session and returns the underlying registry.
    pub fn into_registry(self) -> DevtoolsRegistry {
        self.registry
    }

    /// Refreshes the current capture and returns the new session frame.
    pub fn refresh(&mut self) -> Result<DevtoolsSessionFrame, DevtoolsSessionError> {
        if self.connection_state == DevtoolsSessionConnectionState::Closed {
            return Err(DevtoolsSessionError::Closed {
                session_id: self.session_id.clone(),
            });
        }

        self.connection_state = DevtoolsSessionConnectionState::Connected;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        let capture = self.registry.collect_capture();
        let frame = DevtoolsSessionFrame::new(
            self.session_id.clone(),
            generation,
            self.frames.back(),
            capture,
        );

        self.frames.push_back(frame.clone());
        self.truncate_history();
        Ok(frame)
    }

    /// Closes the session. Further refreshes return an error.
    pub fn close(&mut self) {
        self.connection_state = DevtoolsSessionConnectionState::Closing;
        self.connection_state = DevtoolsSessionConnectionState::Closed;
    }

    /// Returns true when the session is closed.
    pub const fn is_closed(&self) -> bool {
        matches!(
            self.connection_state,
            DevtoolsSessionConnectionState::Closed
        )
    }

    /// Clears retained frames without reopening or resetting generation.
    pub fn clear_history(&mut self) {
        self.frames.clear();
    }

    /// Returns the current frame, if one exists.
    pub fn current_frame(&self) -> Option<&DevtoolsSessionFrame> {
        self.frames.back()
    }

    /// Returns the previous retained frame, if one exists.
    pub fn previous_frame(&self) -> Option<&DevtoolsSessionFrame> {
        self.frames.iter().rev().nth(1)
    }

    /// Returns retained frames in generation order.
    pub fn frames(&self) -> impl ExactSizeIterator<Item = &DevtoolsSessionFrame> {
        self.frames.iter()
    }

    /// Exports a sanitized bounded session history.
    pub fn export(&self) -> DevtoolsSessionExport {
        DevtoolsSessionExport::new(
            self.session_id.clone(),
            self.connection_state,
            self.history_limit,
            self.frames.iter().cloned(),
        )
    }

    fn truncate_history(&mut self) {
        while self.frames.len() > self.history_limit {
            self.frames.pop_front();
        }
    }
}

/// Error returned by local session operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DevtoolsSessionError {
    /// Refresh was requested after the session closed.
    #[error("devtools session is closed: {session_id}")]
    Closed {
        /// Sanitized session id.
        session_id: String,
    },
}

/// Error returned while importing an offline session export.
#[derive(Debug, thiserror::Error)]
pub enum DevtoolsSessionImportError {
    /// JSON parsing failed.
    #[error("invalid devtools session JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// JSON input exceeded configured byte limit.
    #[error("devtools session JSON too large: {actual_bytes} bytes exceeds {max_bytes}")]
    JsonTooLarge {
        /// Maximum accepted bytes.
        max_bytes: usize,
        /// Actual input bytes.
        actual_bytes: usize,
    },
    /// Export schema version is not supported.
    #[error("unsupported devtools session schema: expected {expected}, got {actual}")]
    UnsupportedSchema {
        /// Expected schema version.
        expected: &'static str,
        /// Actual sanitized schema version.
        actual: String,
    },
    /// Export protocol version is not supported.
    #[error("unsupported devtools session protocol: expected {expected}, got {actual}")]
    UnsupportedProtocol {
        /// Expected protocol version.
        expected: &'static str,
        /// Actual sanitized protocol version.
        actual: String,
    },
    /// Export contains too many frames.
    #[error("too many devtools session frames: {actual_frames} exceeds {max_frames}")]
    TooManyFrames {
        /// Maximum accepted frame count.
        max_frames: usize,
        /// Actual frame count.
        actual_frames: usize,
    },
    /// A frame contains too many event records.
    #[error(
        "too many devtools events in generation {generation}: {actual_events} exceeds {max_events}"
    )]
    TooManyEvents {
        /// Generation that exceeded the event limit.
        generation: u64,
        /// Maximum accepted events per frame.
        max_events: usize,
        /// Actual event count.
        actual_events: usize,
    },
}

fn sanitize_session_id(session_id: impl Into<String>) -> String {
    let session_id = sanitize_sensitive_text(&session_id.into());
    if session_id.trim().is_empty() {
        "devtools.session".to_owned()
    } else {
        session_id
    }
}
