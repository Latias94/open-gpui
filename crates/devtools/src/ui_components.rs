//! DevTools adapters for `open-gpui-ui-components` public facts.

use std::collections::HashMap;

use open_gpui_ui_components::{
    A11yValueMetadata, ComponentA11yContract, ComponentA11yEvidence, ThemeSnapshot,
    gpui_adapter::WindowOverlaySnapshot,
};

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

/// Converts component accessibility evidence rows into a DevTools tree.
pub fn a11y_evidence_probe_snapshot<'a>(
    evidence: impl IntoIterator<Item = &'a ComponentA11yEvidence>,
) -> SnapshotProbeSnapshot {
    let contracts = evidence
        .into_iter()
        .map(|evidence| {
            let mut contract = ComponentA11yContract::new(evidence.component, evidence.role)
                .with_label_source(evidence.label_source)
                .with_actions(evidence.actions);
            if let Some(value_kind) = evidence.value_kind {
                contract = contract.with_value_metadata(A11yValueMetadata::present(value_kind));
            }
            if let Some(orientation) = evidence.orientation {
                contract = contract.with_orientation(orientation);
            }
            contract
        })
        .collect::<Vec<_>>();

    a11y_contracts_probe_snapshot(contracts)
}

/// Converts component accessibility contracts into a DevTools tree.
pub fn a11y_contracts_probe_snapshot(
    contracts: impl IntoIterator<Item = ComponentA11yContract>,
) -> SnapshotProbeSnapshot {
    let contracts = contracts.into_iter().collect::<Vec<_>>();
    let mut root = snapshot_node_with_payload(
        ["accessibility"],
        "Accessibility contracts",
        serde_json::json!({
            "contract_count": contracts.len(),
        }),
    );

    for contract in contracts {
        root = root.with_child(a11y_contract_node(contract));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

fn a11y_contract_node(contract: ComponentA11yContract) -> crate::SnapshotNode {
    let validation = contract.validate();
    snapshot_node_with_payload(
        ["accessibility", contract.component()],
        contract.component(),
        serde_json::json!({
            "component": contract.component(),
            "role": debug_variant_label(contract.role()),
            "label_source": debug_variant_label(contract.label_source()),
            "description_source": debug_variant_label(contract.description_source()),
            "selected": contract.selected(),
            "checked": contract.checked().map(debug_variant_label),
            "expanded": contract.expanded(),
            "disabled": contract.disabled(),
            "value": contract.value().map(value_metadata_payload),
            "orientation": contract.orientation().map(debug_variant_label),
            "actions": contract
                .actions()
                .iter()
                .copied()
                .map(debug_variant_label)
                .collect::<Vec<_>>(),
            "valid": validation.is_ok(),
            "violation": validation.err().map(|violation| debug_variant_label(violation.error())),
        }),
    )
}

fn value_metadata_payload(value: A11yValueMetadata) -> serde_json::Value {
    serde_json::json!({
        "kind": debug_variant_label(value.kind()),
        "present": value.is_present(),
    })
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
