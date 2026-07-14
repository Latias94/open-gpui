//! DevTools adapters for `open-gpui-ui-components` public facts.

use std::collections::HashMap;

use open_gpui_ui_components::{
    ThemeSnapshot, component_contract::component_contract_entry,
    gpui_adapter::WindowOverlaySnapshot,
};
use open_gpui_ui_core::{Role, SemanticDescriptor};
use serde::Serialize;

use crate::{
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
};

/// Converts a component theme snapshot into a DevTools tree.
pub fn theme_probe_snapshot(snapshot: ThemeSnapshot<'_>) -> SnapshotProbeSnapshot {
    let mut root = snapshot_node_with_payload(
        ["theme"],
        "Theme",
        serde_json::json!({
            "mode": snapshot.mode().as_str(),
            "revision": snapshot.revision(),
            "color_count": snapshot.colors().len(),
        }),
    );

    for color in snapshot.colors() {
        root = root.with_child(snapshot_node_with_payload(
            ["theme", color.token().as_str(), color.state().as_str()],
            format!("{} {}", color.token().as_str(), color.state().as_str()),
            serde_json::json!({
                "token": color.token().as_str(),
                "state": color.state().as_str(),
                "rgb": format!("#{rgb:06x}", rgb = color.rgb()),
            }),
        ));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a window overlay runtime snapshot into an allowlisted DevTools tree.
///
/// Layer identities and parent references are projected as snapshot-local ordinals. Raw runtime
/// layer identities and the owning window identity never cross this adapter boundary.
pub fn window_overlay_probe_snapshot(snapshot: &WindowOverlaySnapshot) -> SnapshotProbeSnapshot {
    let ordinal_by_id = snapshot
        .layers()
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id(), overlay_layer_ordinal(index)))
        .collect::<HashMap<_, _>>();
    let mut root = snapshot_node_with_payload(
        ["window-overlay"],
        "Window overlay runtime",
        serde_json::json!({
            "layer_count": snapshot.layers().len(),
        }),
    );

    for (index, layer) in snapshot.layers().iter().enumerate() {
        let id = overlay_layer_ordinal(index);
        let parent = layer
            .parent()
            .and_then(|parent| ordinal_by_id.get(parent))
            .cloned();
        root = root.with_child(snapshot_node_with_payload(
            ["window-overlay", id.as_str()],
            format!("Overlay layer {}", index + 1),
            serde_json::json!({
                "id": id,
                "parent": parent,
                "kind": debug_variant_label(layer.kind()),
                "phase": debug_variant_label(layer.phase()),
                "presence": debug_variant_label(layer.presence()),
                "pending_open": layer.pending_open(),
                "pending_reason": layer.pending_intent().map(debug_variant_label),
                "keyboard_eligible": layer.keyboard_eligible(),
                "modal_pointer_barrier": layer.modal_pointer_barrier(),
                "focus_active": layer.focus_active(),
                "focus_entered": layer.focus_entered(),
            }),
        ));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Contract-owned identity attached to one resolved component semantic node.
///
/// Callers should source both fields from the canonical component contract row. They are product
/// metadata, not accessible text, and are the only component strings accepted by this adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSemanticIdentity {
    contract_id: &'static str,
    family: &'static str,
}

impl ComponentSemanticIdentity {
    /// Resolves an identity from the canonical component contract row.
    pub fn for_component(component: &str) -> Option<Self> {
        let row = component_contract_entry(component)?;
        Some(Self {
            contract_id: row.name,
            family: row.family?,
        })
    }

    /// Returns the canonical component contract id.
    pub const fn contract_id(self) -> &'static str {
        self.contract_id
    }

    /// Returns the canonical component family.
    pub const fn family(self) -> &'static str {
        self.family
    }
}

/// Opaque stable identity for one semantic node in a DevTools snapshot.
///
/// The value is app-assigned and intentionally carries no renderer id, label, or user text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueSemanticNodeId(u64);

impl OpaqueSemanticNodeId {
    /// Creates an opaque semantic node identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the opaque numeric value.
    pub const fn value(self) -> u64 {
        self.0
    }

    fn snapshot_label(self) -> String {
        format!("semantic-node-{:016x}", self.0)
    }
}

/// One ephemeral resolved semantic descriptor prepared for DevTools projection.
///
/// The descriptor remains borrowed and is consumed immediately. DevTools never stores this value
/// or reconstructs it from component evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSemanticNode<'a, NodeId> {
    component: ComponentSemanticIdentity,
    opaque_id: OpaqueSemanticNodeId,
    semantics: SemanticDescriptor<'a, NodeId>,
}

impl<'a, NodeId> ResolvedSemanticNode<'a, NodeId> {
    /// Creates a resolved node from canonical metadata, an opaque id, and the renderer projection.
    pub const fn new(
        component: ComponentSemanticIdentity,
        opaque_id: OpaqueSemanticNodeId,
        semantics: SemanticDescriptor<'a, NodeId>,
    ) -> Self {
        Self {
            component,
            opaque_id,
            semantics,
        }
    }

    /// Returns the canonical component identity.
    pub const fn component(&self) -> ComponentSemanticIdentity {
        self.component
    }

    /// Returns the opaque semantic node identity.
    pub const fn opaque_id(&self) -> OpaqueSemanticNodeId {
        self.opaque_id
    }

    /// Returns the borrowed resolved semantic projection.
    pub const fn semantics(&self) -> &SemanticDescriptor<'a, NodeId> {
        &self.semantics
    }
}

/// Converts resolved component semantics into an allowlisted, redacted DevTools tree.
///
/// Accessible label, description, value, placeholder, password, numeric, user-input, and
/// clipboard-derived content never enter a snapshot payload. Relation targets and renderer node
/// ids are reduced to counts or presence; only caller-provided opaque ids cross the boundary.
pub fn resolved_semantics_probe_snapshot<'a, NodeId: 'a>(
    nodes: impl IntoIterator<Item = ResolvedSemanticNode<'a, NodeId>>,
) -> SnapshotProbeSnapshot {
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    let mut root = snapshot_node_with_payload(
        ["accessibility"],
        "Resolved accessibility semantics",
        serde_json::json!({
            "node_count": nodes.len(),
        }),
    );
    let mut redaction = SnapshotRedactionSummary::default();

    for node in nodes {
        root = root.with_child(resolved_semantic_node(node, &mut redaction));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root])).with_redaction(redaction)
}

fn resolved_semantic_node<NodeId>(
    node: ResolvedSemanticNode<'_, NodeId>,
    redaction: &mut SnapshotRedactionSummary,
) -> crate::SnapshotNode {
    let component = node.component();
    let opaque_id = node.opaque_id().snapshot_label();
    let semantics = node.semantics();
    let text = RedactedSemanticText::from_descriptor(semantics, redaction);
    let numeric = RedactedSemanticNumeric::from_descriptor(semantics, redaction);

    snapshot_node_with_payload(
        ["accessibility", "resolved", opaque_id.as_str()],
        format!("{} semantic node", component.contract_id()),
        serde_json::json!({
            "contract_id": component.contract_id(),
            "family": component.family(),
            "semantic_id": opaque_id,
            "role": debug_variant_label(semantics.role()),
            "text": text,
            "numeric": numeric,
            "text_structure": {
                "character_lengths": PresenceSummary::from_present(
                    !semantics.character_lengths().is_empty()
                ),
                "selection": PresenceSummary::from_present(
                    semantics.text_selection().is_some()
                ),
            },
            "relations": {
                "controls_count": semantics.controls().len(),
                "labelled_by_count": semantics.labelled_by().len(),
                "described_by_count": semantics.described_by().len(),
                "error_message": PresenceSummary::from_present(
                    semantics.error_message().is_some()
                ),
            },
            "state": {
                "selected": semantics.selected(),
                "required": semantics.required(),
                "invalid": semantics.invalid(),
                "busy": semantics.busy(),
                "read_only": semantics.read_only(),
                "hidden": semantics.hidden(),
                "modal": semantics.modal(),
                "disabled": semantics.disabled(),
                "expanded": semantics.expanded(),
                "toggled": semantics.toggled().map(debug_variant_label),
            },
            "collection": {
                "level": semantics.level(),
                "position_in_set": semantics.position_in_set(),
                "size_of_set": semantics.size_of_set(),
                "row_index": semantics.row_index(),
                "column_index": semantics.column_index(),
                "row_span": semantics.row_span(),
                "column_span": semantics.column_span(),
                "row_count": semantics.row_count(),
                "column_count": semantics.column_count(),
                "sort_direction": semantics.sort_direction().map(debug_variant_label),
                "orientation": semantics.orientation().map(debug_variant_label),
            },
            "actions": semantics
                .available_actions()
                .map(debug_variant_label)
                .collect::<Vec<_>>(),
        }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SensitiveSemanticMarker {
    Absent,
    Redacted,
    PasswordRedacted,
}

impl SensitiveSemanticMarker {
    fn from_text(
        value: Option<&str>,
        password: bool,
        note: &'static str,
        redaction: &mut SnapshotRedactionSummary,
    ) -> Self {
        if value.is_none() {
            return Self::Absent;
        }

        redaction.record_redacted(note);
        if password {
            Self::PasswordRedacted
        } else {
            Self::Redacted
        }
    }

    fn from_number(
        value: Option<f64>,
        note: &'static str,
        redaction: &mut SnapshotRedactionSummary,
    ) -> Self {
        if value.is_none() {
            return Self::Absent;
        }

        redaction.record_redacted(note);
        Self::Redacted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct RedactedSemanticText {
    label: SensitiveSemanticMarker,
    description: SensitiveSemanticMarker,
    value: SensitiveSemanticMarker,
    placeholder: SensitiveSemanticMarker,
}

impl RedactedSemanticText {
    fn from_descriptor<NodeId>(
        semantics: &SemanticDescriptor<'_, NodeId>,
        redaction: &mut SnapshotRedactionSummary,
    ) -> Self {
        Self {
            label: SensitiveSemanticMarker::from_text(
                semantics.label(),
                false,
                "resolved semantic accessible label redacted",
                redaction,
            ),
            description: SensitiveSemanticMarker::from_text(
                semantics.description(),
                false,
                "resolved semantic accessible description redacted",
                redaction,
            ),
            value: SensitiveSemanticMarker::from_text(
                semantics.value(),
                semantics.role() == Role::PasswordInput,
                if semantics.role() == Role::PasswordInput {
                    "resolved semantic password value redacted"
                } else {
                    "resolved semantic value text redacted"
                },
                redaction,
            ),
            placeholder: SensitiveSemanticMarker::from_text(
                semantics.placeholder(),
                false,
                "resolved semantic placeholder redacted",
                redaction,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct RedactedSemanticNumeric {
    value: SensitiveSemanticMarker,
    minimum: SensitiveSemanticMarker,
    maximum: SensitiveSemanticMarker,
}

impl RedactedSemanticNumeric {
    fn from_descriptor<NodeId>(
        semantics: &SemanticDescriptor<'_, NodeId>,
        redaction: &mut SnapshotRedactionSummary,
    ) -> Self {
        Self {
            value: SensitiveSemanticMarker::from_number(
                semantics.numeric_value(),
                "resolved semantic numeric value redacted",
                redaction,
            ),
            minimum: SensitiveSemanticMarker::from_number(
                semantics.min_numeric_value(),
                "resolved semantic numeric minimum redacted",
                redaction,
            ),
            maximum: SensitiveSemanticMarker::from_number(
                semantics.max_numeric_value(),
                "resolved semantic numeric maximum redacted",
                redaction,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum PresenceSummary {
    Absent,
    Present,
}

impl PresenceSummary {
    const fn from_present(present: bool) -> Self {
        if present { Self::Present } else { Self::Absent }
    }
}

fn overlay_layer_ordinal(index: usize) -> String {
    format!("overlay-layer-{}", index + 1)
}

fn debug_variant_label(value: impl std::fmt::Debug) -> String {
    let value = format!("{value:?}");
    let mut label = String::with_capacity(value.len());
    let mut previous_was_separator = true;

    for character in value.chars() {
        if character.is_ascii_uppercase() {
            if !previous_was_separator {
                label.push('-');
            }
            label.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if character.is_ascii_alphanumeric() {
            label.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            label.push('-');
            previous_was_separator = true;
        }
    }

    label.trim_matches('-').to_owned()
}
