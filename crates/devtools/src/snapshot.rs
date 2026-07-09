use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};

use crate::{
    ProbeId, SnapshotRedactionSummary,
    adapters::{sanitize_json_value, sanitize_sensitive_text},
};

/// Kind of devtools snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotKind {
    /// Element, layout, or render-tree facts.
    Element,
    /// Accessibility-tree facts.
    Accessibility,
    /// Focus state.
    Focus,
    /// Input dispatch state.
    Input,
    /// Scroll viewport state.
    Scroll,
    /// Theme resolution state.
    Theme,
    /// Motion state.
    Motion,
    /// Docking state.
    Docking,
    /// Form state.
    Form,
    /// Async resource state.
    Resource,
    /// Probe diagnostic state.
    Diagnostic,
    /// Command registry, keybinding, and keymap resolution state.
    Command,
    /// Timeline, event, and span state.
    Timeline,
    /// Layout, bounds, and committed geometry state.
    Layout,
    /// Custom app-provided snapshot.
    Custom(String),
}

impl SnapshotKind {
    /// Returns the stable display label for this snapshot kind.
    pub fn as_label(&self) -> Cow<'_, str> {
        match self {
            Self::Element => Cow::Borrowed("element"),
            Self::Accessibility => Cow::Borrowed("accessibility"),
            Self::Focus => Cow::Borrowed("focus"),
            Self::Input => Cow::Borrowed("input"),
            Self::Scroll => Cow::Borrowed("scroll"),
            Self::Theme => Cow::Borrowed("theme"),
            Self::Motion => Cow::Borrowed("motion"),
            Self::Docking => Cow::Borrowed("docking"),
            Self::Form => Cow::Borrowed("form"),
            Self::Resource => Cow::Borrowed("resource"),
            Self::Diagnostic => Cow::Borrowed("diagnostic"),
            Self::Command => Cow::Borrowed("command"),
            Self::Timeline => Cow::Borrowed("timeline"),
            Self::Layout => Cow::Borrowed("layout"),
            Self::Custom(label) => Cow::Owned(sanitize_sensitive_text(label)),
        }
    }

    fn sanitized(self) -> Self {
        match self {
            Self::Custom(label) => Self::Custom(sanitize_sensitive_text(&label)),
            other => other,
        }
    }
}

impl Serialize for SnapshotKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Element => serializer.serialize_unit_variant("SnapshotKind", 0, "Element"),
            Self::Accessibility => {
                serializer.serialize_unit_variant("SnapshotKind", 1, "Accessibility")
            }
            Self::Focus => serializer.serialize_unit_variant("SnapshotKind", 2, "Focus"),
            Self::Input => serializer.serialize_unit_variant("SnapshotKind", 3, "Input"),
            Self::Scroll => serializer.serialize_unit_variant("SnapshotKind", 4, "Scroll"),
            Self::Theme => serializer.serialize_unit_variant("SnapshotKind", 5, "Theme"),
            Self::Motion => serializer.serialize_unit_variant("SnapshotKind", 6, "Motion"),
            Self::Docking => serializer.serialize_unit_variant("SnapshotKind", 7, "Docking"),
            Self::Form => serializer.serialize_unit_variant("SnapshotKind", 8, "Form"),
            Self::Resource => serializer.serialize_unit_variant("SnapshotKind", 9, "Resource"),
            Self::Diagnostic => serializer.serialize_unit_variant("SnapshotKind", 10, "Diagnostic"),
            Self::Command => serializer.serialize_unit_variant("SnapshotKind", 11, "Command"),
            Self::Timeline => serializer.serialize_unit_variant("SnapshotKind", 12, "Timeline"),
            Self::Layout => serializer.serialize_unit_variant("SnapshotKind", 13, "Layout"),
            Self::Custom(label) => serializer.serialize_newtype_variant(
                "SnapshotKind",
                14,
                "Custom",
                &sanitize_sensitive_text(label),
            ),
        }
    }
}

impl<'de> Deserialize<'de> for SnapshotKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum SnapshotKindValue {
            Element,
            Accessibility,
            Focus,
            Input,
            Scroll,
            Theme,
            Motion,
            Docking,
            Form,
            Resource,
            Diagnostic,
            Command,
            Timeline,
            Layout,
            Custom(String),
        }

        Ok(match SnapshotKindValue::deserialize(deserializer)? {
            SnapshotKindValue::Element => Self::Element,
            SnapshotKindValue::Accessibility => Self::Accessibility,
            SnapshotKindValue::Focus => Self::Focus,
            SnapshotKindValue::Input => Self::Input,
            SnapshotKindValue::Scroll => Self::Scroll,
            SnapshotKindValue::Theme => Self::Theme,
            SnapshotKindValue::Motion => Self::Motion,
            SnapshotKindValue::Docking => Self::Docking,
            SnapshotKindValue::Form => Self::Form,
            SnapshotKindValue::Resource => Self::Resource,
            SnapshotKindValue::Diagnostic => Self::Diagnostic,
            SnapshotKindValue::Command => Self::Command,
            SnapshotKindValue::Timeline => Self::Timeline,
            SnapshotKindValue::Layout => Self::Layout,
            SnapshotKindValue::Custom(label) => Self::Custom(sanitize_sensitive_text(&label)),
        })
    }
}

/// Tree of inspectable snapshot nodes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotTree {
    /// Root snapshot nodes.
    pub nodes: Vec<SnapshotNode>,
}

impl SnapshotTree {
    /// Creates a snapshot tree from root nodes.
    pub fn new(nodes: impl IntoIterator<Item = SnapshotNode>) -> Self {
        Self {
            nodes: nodes.into_iter().map(SnapshotNode::sanitized).collect(),
        }
    }

    /// Returns true when the tree has no root nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a tree with sanitized node ids, labels, payloads, and children.
    pub fn sanitized(mut self) -> Self {
        self.nodes = self
            .nodes
            .into_iter()
            .map(SnapshotNode::sanitized)
            .collect();
        self
    }
}

/// One node in an inspectable snapshot tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotNode {
    /// Stable node id within the snapshot.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional JSON payload for diagnostics.
    pub payload: Option<serde_json::Value>,
    /// Child nodes.
    pub children: Vec<SnapshotNode>,
}

impl SnapshotNode {
    /// Creates a snapshot node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: sanitize_sensitive_text(&id.into()),
            label: sanitize_sensitive_text(&label.into()),
            payload: None,
            children: Vec::new(),
        }
    }

    /// Attaches a JSON payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(sanitize_json_value(payload));
        self
    }

    /// Appends a child node.
    pub fn with_child(mut self, child: SnapshotNode) -> Self {
        self.children.push(child.sanitized());
        self
    }

    /// Returns this node with sanitized id, label, payload, and children.
    pub fn sanitized(mut self) -> Self {
        self.id = sanitize_sensitive_text(&self.id);
        self.label = sanitize_sensitive_text(&self.label);
        self.payload = self.payload.map(sanitize_json_value);
        self.children = self
            .children
            .into_iter()
            .map(SnapshotNode::sanitized)
            .collect();
        self
    }
}

impl Serialize for SnapshotNode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let id = sanitize_sensitive_text(&self.id);
        let label = sanitize_sensitive_text(&self.label);
        let payload = self.payload.clone().map(sanitize_json_value);
        let mut state = serializer.serialize_struct("SnapshotNode", 4)?;
        state.serialize_field("id", &id)?;
        state.serialize_field("label", &label)?;
        state.serialize_field("payload", &payload)?;
        state.serialize_field("children", &self.children)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SnapshotNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SnapshotNodeValue {
            id: String,
            label: String,
            payload: Option<serde_json::Value>,
            children: Vec<SnapshotNode>,
        }

        let value = SnapshotNodeValue::deserialize(deserializer)?;
        Ok(Self {
            id: sanitize_sensitive_text(&value.id),
            label: sanitize_sensitive_text(&value.label),
            payload: value.payload.map(sanitize_json_value),
            children: value
                .children
                .into_iter()
                .map(SnapshotNode::sanitized)
                .collect(),
        })
    }
}

/// Serializable envelope returned by a devtools probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    /// Probe that produced this snapshot.
    pub probe_id: ProbeId,
    /// Snapshot kind.
    pub kind: SnapshotKind,
    /// Snapshot tree.
    pub tree: SnapshotTree,
    /// Redaction summary.
    pub redaction: SnapshotRedactionSummary,
}

impl SnapshotEnvelope {
    /// Creates a snapshot envelope.
    pub fn new(probe_id: ProbeId, kind: SnapshotKind, tree: SnapshotTree) -> Self {
        Self {
            probe_id,
            kind: kind.sanitized(),
            tree: tree.sanitized(),
            redaction: SnapshotRedactionSummary::default(),
        }
    }

    /// Attaches a redaction summary.
    pub fn with_redaction(mut self, redaction: SnapshotRedactionSummary) -> Self {
        self.redaction = redaction.sanitized();
        self
    }

    /// Returns an envelope with every exported string channel sanitized.
    pub fn sanitized(mut self) -> Self {
        self.kind = self.kind.sanitized();
        self.tree = self.tree.sanitized();
        self.redaction = self.redaction.sanitized();
        self
    }
}

/// Collection returned by a registry snapshot pass.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotCollection {
    /// Snapshots successfully collected or synthesized as diagnostics.
    pub snapshots: Vec<SnapshotEnvelope>,
    /// Diagnostics from probes that failed to collect.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

impl SnapshotCollection {
    /// Returns a collection with all snapshots and diagnostics sanitized.
    pub fn sanitized(mut self) -> Self {
        self.snapshots = self
            .snapshots
            .into_iter()
            .map(SnapshotEnvelope::sanitized)
            .collect();
        self.diagnostics = self
            .diagnostics
            .into_iter()
            .map(SnapshotDiagnostic::sanitized)
            .collect();
        self
    }
}

/// Diagnostic emitted when a probe cannot provide a snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotDiagnostic {
    /// Probe that emitted the diagnostic.
    pub probe_id: ProbeId,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl SnapshotDiagnostic {
    /// Stable code used when probe collection fails.
    pub const COLLECTION_FAILED: &'static str = "probe.collection_failed";

    /// Creates a sanitized diagnostic with a stable code.
    pub fn new(probe_id: ProbeId, code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            probe_id,
            code: if code.trim().is_empty() {
                "diagnostic.unknown".to_owned()
            } else {
                sanitize_sensitive_text(&code)
            },
            message: sanitize_sensitive_text(&message.into()),
        }
    }

    /// Creates a sanitized diagnostic for a probe collection failure.
    pub fn collection_failed(probe_id: ProbeId, message: impl Into<String>) -> Self {
        Self::new(probe_id, Self::COLLECTION_FAILED, message)
    }

    /// Returns this diagnostic with sanitized code and message.
    pub fn sanitized(mut self) -> Self {
        self.code = if self.code.trim().is_empty() {
            "diagnostic.unknown".to_owned()
        } else {
            sanitize_sensitive_text(&self.code)
        };
        self.message = sanitize_sensitive_text(&self.message);
        self
    }
}

impl Serialize for SnapshotDiagnostic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let diagnostic = self.clone().sanitized();
        let mut state = serializer.serialize_struct("SnapshotDiagnostic", 3)?;
        state.serialize_field("probe_id", &diagnostic.probe_id)?;
        state.serialize_field("code", &diagnostic.code)?;
        state.serialize_field("message", &diagnostic.message)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SnapshotDiagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SnapshotDiagnosticValue {
            probe_id: ProbeId,
            code: String,
            message: String,
        }

        let value = SnapshotDiagnosticValue::deserialize(deserializer)?;
        Ok(Self {
            probe_id: value.probe_id,
            code: value.code,
            message: value.message,
        }
        .sanitized())
    }
}
