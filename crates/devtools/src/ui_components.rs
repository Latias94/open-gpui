//! DevTools adapters for `open-gpui-ui-components` public facts.

use open_gpui_ui_components::{
    A11yValueMetadata, ComponentA11yContract, ComponentA11yEvidence, ThemeSnapshot,
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
            "role": format!("{:?}", contract.role()),
            "label_source": format!("{:?}", contract.label_source()),
            "description_source": format!("{:?}", contract.description_source()),
            "selected": contract.selected(),
            "checked": contract.checked().map(|checked| format!("{checked:?}")),
            "expanded": contract.expanded(),
            "disabled": contract.disabled(),
            "value": contract.value().map(value_metadata_payload),
            "orientation": contract.orientation().map(|orientation| format!("{orientation:?}")),
            "actions": contract
                .actions()
                .iter()
                .map(|action| format!("{action:?}"))
                .collect::<Vec<_>>(),
            "valid": validation.is_ok(),
            "violation": validation.err().map(|violation| format!("{:?}", violation.error())),
        }),
    )
}

fn value_metadata_payload(value: A11yValueMetadata) -> serde_json::Value {
    serde_json::json!({
        "kind": format!("{:?}", value.kind()),
        "present": value.is_present(),
    })
}
