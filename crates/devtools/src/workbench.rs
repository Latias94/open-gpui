//! App-owned DevTools workbench helpers.

use crate::{
    DevtoolsCapture, DevtoolsInspectorState, DevtoolsRegistry, DevtoolsSession,
    DevtoolsSessionError, DevtoolsSessionExport, DevtoolsSessionFrame,
};

/// Latest refresh outcome for an app-owned DevTools workbench.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevtoolsWorkbenchRefreshStatus {
    /// The workbench has initialized and no refresh outcome is currently being surfaced.
    Idle,
    /// The latest refresh produced the first frame, so no previous frame existed for diffing.
    NoPreviousFrame,
    /// The latest refresh produced at least one added, removed, changed, or collision diff row.
    Changed,
    /// The latest refresh diffed against a previous frame without changed rows.
    NoChange,
    /// The latest refresh failed before producing a frame.
    CaptureError,
}

impl DevtoolsWorkbenchRefreshStatus {
    /// Returns the stable label used by UI status readouts and contract tests.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::NoPreviousFrame => "no-previous-frame",
            Self::Changed => "changed",
            Self::NoChange => "no-change",
            Self::CaptureError => "capture-error",
        }
    }
}

/// Diff state for the current workbench frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevtoolsWorkbenchDiffState {
    /// No current frame or no previous frame exists.
    NoPreviousFrame,
    /// The current frame differs from the previous retained frame.
    Changed,
    /// The current frame has a previous retained frame but no changed rows.
    NoChange,
}

impl DevtoolsWorkbenchDiffState {
    /// Returns the stable label used by UI status readouts and contract tests.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::NoPreviousFrame => "no-previous-frame",
            Self::Changed => "changed",
            Self::NoChange => "no-change",
        }
    }
}

/// Renderer-neutral owner for a local DevTools session and its app-facing readouts.
///
/// `DevtoolsWorkbench` deliberately owns only sanitized, bounded DevTools session frames. The
/// application still owns runtime authority, capture provider registration, UI actions, and any
/// framework-specific inspector controller.
pub struct DevtoolsWorkbench {
    session: DevtoolsSession,
    refresh_status: DevtoolsWorkbenchRefreshStatus,
    last_error: Option<String>,
}

impl std::fmt::Debug for DevtoolsWorkbench {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DevtoolsWorkbench")
            .field("session_id", &self.session_id())
            .field("current_generation", &self.current_generation())
            .field("retained_frames", &self.retained_frames())
            .field("refresh_status", &self.refresh_status)
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl DevtoolsWorkbench {
    /// Creates a workbench over a new connected local session.
    pub fn new(session_id: impl Into<String>, registry: DevtoolsRegistry) -> Self {
        Self::from_session(DevtoolsSession::new(session_id, registry))
    }

    /// Creates a workbench from an existing session.
    pub fn from_session(session: DevtoolsSession) -> Self {
        Self {
            session,
            refresh_status: DevtoolsWorkbenchRefreshStatus::Idle,
            last_error: None,
        }
    }

    /// Sets the bounded frame history limit.
    pub fn with_history_limit(mut self, history_limit: usize) -> Self {
        self.session = self.session.with_history_limit(history_limit);
        self
    }

    /// Returns the sanitized session id.
    pub fn session_id(&self) -> &str {
        self.session.session_id()
    }

    /// Returns the latest refresh status.
    pub const fn refresh_status(&self) -> DevtoolsWorkbenchRefreshStatus {
        self.refresh_status
    }

    /// Marks the workbench idle after app-owned initialization.
    pub fn mark_idle(&mut self) {
        self.refresh_status = DevtoolsWorkbenchRefreshStatus::Idle;
        self.last_error = None;
    }

    /// Returns the latest sanitized refresh error, if one exists.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Returns the underlying session.
    pub const fn session(&self) -> &DevtoolsSession {
        &self.session
    }

    /// Returns the underlying session for app-owned registry or lifecycle changes.
    pub fn session_mut(&mut self) -> &mut DevtoolsSession {
        &mut self.session
    }

    /// Refreshes the local session and updates workbench readouts.
    pub fn refresh(&mut self) -> Result<DevtoolsSessionFrame, DevtoolsSessionError> {
        match self.session.refresh() {
            Ok(frame) => {
                self.refresh_status = refresh_status_for_frame(&frame);
                self.last_error = None;
                Ok(frame)
            }
            Err(error) => {
                self.refresh_status = DevtoolsWorkbenchRefreshStatus::CaptureError;
                self.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Returns the inspector state for the current frame, or an empty state when no frame exists.
    pub fn inspector_state(&self) -> DevtoolsInspectorState {
        self.current_frame()
            .cloned()
            .map(DevtoolsInspectorState::from_session_frame)
            .unwrap_or_else(|| DevtoolsInspectorState::from_capture(DevtoolsCapture::default()))
    }

    /// Returns the current frame, if one exists.
    pub fn current_frame(&self) -> Option<&DevtoolsSessionFrame> {
        self.session.current_frame()
    }

    /// Returns the previous retained frame, if one exists.
    pub fn previous_frame(&self) -> Option<&DevtoolsSessionFrame> {
        self.session.previous_frame()
    }

    /// Returns the current generation, if a frame exists.
    pub fn current_generation(&self) -> Option<u64> {
        self.current_frame().map(|frame| frame.generation)
    }

    /// Returns the previous generation attached to the current frame.
    pub fn previous_generation(&self) -> Option<u64> {
        self.current_frame()
            .and_then(|frame| frame.previous_generation)
    }

    /// Returns the retained frame count.
    pub fn retained_frames(&self) -> usize {
        self.session.frames().len()
    }

    /// Returns the configured session history limit.
    pub const fn history_limit(&self) -> usize {
        self.session.history_limit()
    }

    /// Returns the current diff state.
    pub fn diff_state(&self) -> DevtoolsWorkbenchDiffState {
        let Some(diff) = self
            .current_frame()
            .and_then(|frame| frame.diff_from_previous.as_ref())
        else {
            return DevtoolsWorkbenchDiffState::NoPreviousFrame;
        };

        if diff_has_changes(diff) {
            DevtoolsWorkbenchDiffState::Changed
        } else {
            DevtoolsWorkbenchDiffState::NoChange
        }
    }

    /// Returns the current diff state label.
    pub fn diff_state_label(&self) -> &'static str {
        self.diff_state().as_label()
    }

    /// Returns the current diff row count.
    pub fn diff_row_count(&self) -> usize {
        self.current_frame()
            .and_then(|frame| frame.diff_from_previous.as_ref())
            .map_or(0, |diff| diff.rows.len())
    }

    /// Returns a compact current diff count label.
    pub fn diff_summary_label(&self) -> String {
        self.current_frame()
            .and_then(|frame| frame.diff_from_previous.as_ref())
            .map(|diff| {
                format!(
                    "added={} changed={} removed={} collisions={} rows={}",
                    diff.summary.added,
                    diff.summary.changed,
                    diff.summary.removed,
                    diff.summary.collisions,
                    diff.rows.len()
                )
            })
            .unwrap_or_else(|| {
                DevtoolsWorkbenchDiffState::NoPreviousFrame
                    .as_label()
                    .to_string()
            })
    }

    /// Exports the bounded sanitized session history.
    pub fn export(&self) -> DevtoolsSessionExport {
        self.session.export()
    }
}

fn refresh_status_for_frame(frame: &DevtoolsSessionFrame) -> DevtoolsWorkbenchRefreshStatus {
    let Some(diff) = frame.diff_from_previous.as_ref() else {
        return DevtoolsWorkbenchRefreshStatus::NoPreviousFrame;
    };

    if diff_has_changes(diff) {
        DevtoolsWorkbenchRefreshStatus::Changed
    } else {
        DevtoolsWorkbenchRefreshStatus::NoChange
    }
}

fn diff_has_changes(diff: &crate::DevtoolsCaptureDiff) -> bool {
    diff.summary.added > 0
        || diff.summary.changed > 0
        || diff.summary.removed > 0
        || diff.summary.collisions > 0
}
