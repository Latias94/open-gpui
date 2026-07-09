//! Target identity and target-tree facts for DevTools captures.

use serde::{Deserialize, Serialize};

use crate::{
    ProbeId,
    adapters::{sanitize_json_value, sanitize_sensitive_text, stable_node_id, summary_payload},
};

/// Stable id for a DevTools target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DevtoolsTargetId(String);

impl DevtoolsTargetId {
    /// Creates a sanitized target id.
    pub fn new(id: impl Into<String>) -> Self {
        let id = sanitize_sensitive_text(&id.into());
        if id.trim().is_empty() {
            Self("target".to_owned())
        } else {
            Self(id)
        }
    }

    /// Creates a deterministic target id from sanitized path segments.
    pub fn from_parts<I, S>(parts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self(stable_node_id(parts))
    }

    /// Creates a deterministic target id for a legacy probe.
    pub fn from_probe_id(probe_id: &ProbeId) -> Self {
        Self::from_parts(["probe", probe_id.as_str()])
    }

    /// Returns the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DevtoolsTargetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kind of DevTools target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DevtoolsTargetKind {
    /// Application root.
    App,
    /// Native or platform window.
    Window,
    /// Platform viewport.
    Viewport,
    /// Docking space.
    Dockspace,
    /// Docked or floating panel.
    Panel,
    /// Legacy probe target.
    Probe,
    /// Runtime subsystem target.
    Runtime,
    /// Custom application-provided target.
    Custom(String),
}

impl DevtoolsTargetKind {
    /// Returns a stable display label for this target kind.
    pub fn as_label(&self) -> &str {
        match self {
            Self::App => "app",
            Self::Window => "window",
            Self::Viewport => "viewport",
            Self::Dockspace => "dockspace",
            Self::Panel => "panel",
            Self::Probe => "probe",
            Self::Runtime => "runtime",
            Self::Custom(label) => label.as_str(),
        }
    }

    /// Returns this kind with exported labels sanitized.
    pub fn sanitized(self) -> Self {
        match self {
            Self::Custom(label) => Self::Custom(sanitize_sensitive_text(&label)),
            other => other,
        }
    }
}

/// One target in a DevTools target tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsTargetSnapshot {
    /// Stable target id.
    pub id: DevtoolsTargetId,
    /// Target kind.
    pub kind: DevtoolsTargetKind,
    /// Human-readable target label.
    pub label: String,
    /// Optional parent target id.
    pub parent_id: Option<DevtoolsTargetId>,
    /// Optional sanitized target metadata.
    pub metadata: Option<serde_json::Value>,
}

impl DevtoolsTargetSnapshot {
    /// Creates a sanitized target snapshot.
    pub fn new(id: DevtoolsTargetId, kind: DevtoolsTargetKind, label: impl Into<String>) -> Self {
        Self {
            id,
            kind: kind.sanitized(),
            label: sanitize_sensitive_text(&label.into()),
            parent_id: None,
            metadata: None,
        }
        .sanitized()
    }

    /// Attaches a parent target id.
    pub fn parent_id(mut self, parent_id: DevtoolsTargetId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Attaches sanitized target metadata.
    pub fn with_metadata<T>(mut self, metadata: T) -> Self
    where
        T: Serialize,
    {
        self.metadata = Some(summary_payload(metadata));
        self
    }

    /// Returns this target with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.id = DevtoolsTargetId::new(self.id.0);
        self.kind = self.kind.sanitized();
        self.label = sanitize_sensitive_text(&self.label);
        self.parent_id = self
            .parent_id
            .map(|parent_id| DevtoolsTargetId::new(parent_id.0));
        self.metadata = self.metadata.map(sanitize_json_value);
        self
    }
}

/// Tree of DevTools targets captured in one collection pass.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsTargetTree {
    /// Targets in deterministic parent-before-child order.
    pub targets: Vec<DevtoolsTargetSnapshot>,
}

impl DevtoolsTargetTree {
    /// Creates a sanitized target tree.
    pub fn new(targets: impl IntoIterator<Item = DevtoolsTargetSnapshot>) -> Self {
        Self {
            targets: targets
                .into_iter()
                .map(DevtoolsTargetSnapshot::sanitized)
                .collect(),
        }
    }

    /// Returns true when the target tree has no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Returns this tree with every exported channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.targets = self
            .targets
            .into_iter()
            .map(DevtoolsTargetSnapshot::sanitized)
            .collect();
        self
    }
}
