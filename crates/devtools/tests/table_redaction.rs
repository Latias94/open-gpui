#![cfg(feature = "ui-components")]

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use open_gpui_devtools::{
    DevtoolsArtifact, DevtoolsArtifactMetadata, DevtoolsArtifactRecord, DevtoolsInspectorState,
    DevtoolsRegistry, DevtoolsReport, DevtoolsSession, SnapshotKind, adapters::opaque_stable_id,
    ui_components::TableDevtoolsSession,
};
use open_gpui_ui_components::{
    Table,
    table::{TableBehaviorSnapshot, TableDebugSelector},
};
use open_gpui_ui_core::{TableColumn, TableRow, TableState, ui_px};

const TABLE_ID_CANARY: &str = "u11-table-id-canary-019f4ad7";
const TABLE_LABEL_CANARY: &str = "u11-table-label-canary-4d33";
const COLUMN_ID_CANARY: &str = "u11-column-id-canary-7573";
const COLUMN_LABEL_CANARY: &str = "u11-column-label-canary-ac26";
const ROW_ID_CANARY: &str = "u11-row-id-canary-94bc";
const INSTANCE_ID_CANARY: &str = "u11-instance-id-canary-135cc634";
const GROUP_VALUE_CANARY: &str = "u11-group-value-canary-table";
const CELL_VALUE_CANARY: &str = "u11-cell-value-canary-history";
const NEXT_ROW_CANARY: &str = "u11-next-row-canary-diff";

#[test]
fn table_canaries_never_cross_any_devtools_artifact_boundary() {
    let initial = table_snapshot(false);
    let changed = table_snapshot(true);
    let source_forms = sensitive_source_forms(&initial);
    let fixture_json = include_str!("fixtures/table-redaction.json");
    let fixture_value: serde_json::Value = serde_json::from_str(fixture_json).unwrap();
    let mut fixture_session = TableDevtoolsSession::default();
    let fixture_projection = fixture_session.snapshot(&initial);
    let fixture_root = &fixture_projection.tree().nodes[0];
    assert_eq!(
        serde_json::json!({
            "table_id": fixture_root.payload.as_ref().unwrap()["table_id"],
            "column_count": initial.columns().len(),
            "row_count": initial.rows().len(),
            "redacted_values": fixture_projection.redaction().redacted_values,
        }),
        fixture_value
    );

    let invocation = Arc::new(Mutex::new(0_u8));
    let table_session = Arc::new(Mutex::new(TableDevtoolsSession::default()));
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_snapshot_probe("ui-components.table", SnapshotKind::Element, {
            let invocation = Arc::clone(&invocation);
            let table_session = Arc::clone(&table_session);
            move || {
                let mut invocation = invocation.lock().expect("invocation lock");
                let snapshot = if *invocation == 0 { &initial } else { &changed };
                *invocation = invocation.saturating_add(1);
                Ok(table_session
                    .lock()
                    .expect("table DevTools session lock")
                    .snapshot(snapshot))
            }
        })
        .unwrap();

    let mut session = DevtoolsSession::new("table-redaction", registry);
    let first = session.refresh().unwrap();
    let second = session.refresh().unwrap();
    let export = session.export();
    let report = DevtoolsReport::from_session_export(&export);
    let diff = second
        .diff_from_previous
        .as_ref()
        .expect("second frame includes a diff");
    assert!(!diff.is_empty());

    let capture_json = serde_json::to_string(&first.capture).unwrap();
    let history_json = serde_json::to_string(&session.frames().collect::<Vec<_>>()).unwrap();
    let diff_json = serde_json::to_string(diff).unwrap();
    let export_json = serde_json::to_string(&export).unwrap();
    let capture_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("table-redaction-canary"),
        DevtoolsArtifact::capture(&first.capture),
    )
    .to_pretty_json()
    .unwrap();
    let export_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("table-redaction-canary"),
        DevtoolsArtifact::session_export(&export),
    )
    .to_pretty_json()
    .unwrap();
    let report_json = serde_json::to_string(&report).unwrap();
    let report_markdown = report.to_markdown();
    let report_artifact_json = DevtoolsArtifactRecord::new(
        DevtoolsArtifactMetadata::new("table-redaction-canary"),
        DevtoolsArtifact::report(&report),
    )
    .to_pretty_json()
    .unwrap();
    let inspector = DevtoolsInspectorState::from_capture(first.capture.clone());
    let inspector_detail_json =
        serde_json::to_string(&inspector.selected_detail_json().unwrap()).unwrap();
    let inspector_copy_json = inspector.copy_selected_detail().unwrap().pretty_json;
    let channels = [
        ("capture", capture_json.as_str()),
        ("history", history_json.as_str()),
        ("diff", diff_json.as_str()),
        ("Inspector detail", inspector_detail_json.as_str()),
        ("Inspector copy", inspector_copy_json.as_str()),
        ("export", export_json.as_str()),
        ("capture artifact", capture_artifact_json.as_str()),
        ("export artifact", export_artifact_json.as_str()),
        ("report", report_json.as_str()),
        ("report markdown", report_markdown.as_str()),
        ("report artifact", report_artifact_json.as_str()),
        ("fixture", fixture_json),
    ];
    let adapter_debug = format!(
        "{:?}",
        table_session.lock().expect("table DevTools session lock")
    );

    for source in source_forms {
        for (channel, output) in channels {
            assert!(
                !output.contains(&source),
                "{channel} leaked sensitive Table source form `{source}`"
            );
        }
        assert!(
            !adapter_debug.contains(&source),
            "TableDevtoolsSession Debug leaked sensitive source form `{source}`"
        );
    }

    assert!(capture_json.contains("table-1"));
    assert!(capture_json.contains("column-1"));
    assert!(capture_json.contains("row-1"));
    assert!(capture_json.contains("\"kind\":\"redacted\""));
    assert!(report.summary.redacted_value_count > 0);
}

#[test]
fn table_opaque_ids_are_stable_only_inside_their_own_session() {
    let first_snapshot = table_snapshot(false);
    let mut first_session = TableDevtoolsSession::default();

    let first_projection = first_session.snapshot(&first_snapshot);
    let repeated_projection = first_session.snapshot(&first_snapshot);
    let first = serde_json::to_string(first_projection.tree()).unwrap();
    let repeated = serde_json::to_string(repeated_projection.tree()).unwrap();
    assert!(first.contains("table-1"));
    assert!(first.contains("row-1"));
    assert!(repeated.contains("table-1"));
    assert!(repeated.contains("row-1"));
    assert_eq!(first, repeated);
    let first_row_ids = opaque_row_ids(first_projection.tree());
    let repeated_row_ids = opaque_row_ids(repeated_projection.tree());
    assert!(
        first_row_ids
            .iter()
            .all(|row_id| repeated_row_ids.contains(row_id)),
        "existing typed row identities must retain their opaque ordinals within one session"
    );

    let unrelated = Table::new(
        "unrelated-table",
        "Unrelated table",
        TableState::new([TableRow::new("unrelated-row").with_cell("unrelated-column", "value")])
            .with_columns([TableColumn::new("unrelated-column", "Unrelated column")]),
    )
    .behavior_snapshot(ui_px(0.0), ui_px(160.0));
    let mut unrelated_session = TableDevtoolsSession::default();
    let unrelated_projection = unrelated_session.snapshot(&unrelated);
    let unrelated = serde_json::to_string(unrelated_projection.tree()).unwrap();

    assert!(unrelated.contains("table-1"));
    assert!(unrelated.contains("row-1"));
    assert!(!unrelated.contains(TABLE_ID_CANARY));
    assert!(!unrelated.contains(ROW_ID_CANARY));
}

#[test]
fn table_identity_projection_is_bounded_and_diagnostics_have_stable_unique_ids() {
    let diagnostics_snapshot = table_with_rows(
        "diagnostic-table",
        ["duplicate-a", "duplicate-a", "duplicate-b", "duplicate-b"],
    );
    let mut diagnostic_session = TableDevtoolsSession::default();
    let first = diagnostic_session.snapshot(&diagnostics_snapshot);
    let repeated = diagnostic_session.snapshot(&diagnostics_snapshot);
    let first_ids = identity_diagnostic_ids(first.tree());
    let repeated_ids = identity_diagnostic_ids(repeated.tree());

    assert_eq!(first_ids.len(), 2);
    assert_eq!(first_ids.iter().collect::<HashSet<_>>().len(), 2);
    assert_eq!(first_ids, repeated_ids);
    assert!(
        first_ids
            .iter()
            .all(|id| !id.contains("duplicate-a") && !id.contains("duplicate-b"))
    );

    let mut retention_session = TableDevtoolsSession::default().with_identity_retention(2);
    retention_session.snapshot(&table_with_rows("retained-table", ["retained-row-a"]));
    retention_session.snapshot(&table_with_rows("retained-table", ["retained-row-b"]));
    retention_session.snapshot(&table_with_rows("retained-table", ["retained-row-c"]));
    let retained_debug = format!("{retention_session:?}");
    assert!(retained_debug.contains("table_count: 1"));
    assert!(retained_debug.contains("column_identity_count: 1"));
    assert!(retained_debug.contains("row_identity_count: 2"));

    retention_session.snapshot(&table_with_rows(
        "replacement-table-a",
        ["replacement-row-a"],
    ));
    retention_session.snapshot(&table_with_rows(
        "replacement-table-b",
        ["replacement-row-b"],
    ));
    let replaced_debug = format!("{retention_session:?}");
    assert!(replaced_debug.contains("table_count: 2"));
    assert!(replaced_debug.contains("row_identity_count: 2"));
}

fn opaque_row_ids(tree: &open_gpui_devtools::SnapshotTree) -> Vec<String> {
    tree.nodes[0]
        .children
        .iter()
        .filter_map(|node| {
            node.payload
                .as_ref()
                .and_then(|payload| payload.get("row_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn identity_diagnostic_ids(tree: &open_gpui_devtools::SnapshotTree) -> Vec<String> {
    tree.nodes[0]
        .children
        .iter()
        .filter(|node| {
            node.payload
                .as_ref()
                .is_some_and(|payload| payload.get("diagnostic_id").is_some())
        })
        .map(|node| node.id.clone())
        .collect()
}

fn table_with_rows<const N: usize>(table_id: &str, row_ids: [&str; N]) -> TableBehaviorSnapshot {
    let column_id = "bounded-column";
    let state = TableState::new(
        row_ids
            .into_iter()
            .map(|row_id| TableRow::new(row_id).with_cell(column_id, "redacted-value")),
    )
    .with_columns([TableColumn::new(column_id, "Bounded column")]);
    Table::new(table_id, "Bounded table", state).behavior_snapshot(ui_px(0.0), ui_px(320.0))
}

fn table_snapshot(changed: bool) -> TableBehaviorSnapshot {
    let mut rows = vec![
        TableRow::new(ROW_ID_CANARY)
            .with_instance_id(INSTANCE_ID_CANARY)
            .with_cell(COLUMN_ID_CANARY, GROUP_VALUE_CANARY),
        TableRow::new(ROW_ID_CANARY)
            .with_instance_id(INSTANCE_ID_CANARY)
            .with_cell(COLUMN_ID_CANARY, CELL_VALUE_CANARY),
    ];
    if changed {
        rows.push(TableRow::new(NEXT_ROW_CANARY).with_cell(COLUMN_ID_CANARY, GROUP_VALUE_CANARY));
    }

    let state = TableState::new(rows)
        .with_columns([TableColumn::new(COLUMN_ID_CANARY, COLUMN_LABEL_CANARY)])
        .with_grouping([COLUMN_ID_CANARY])
        .with_all_rows_expanded();
    Table::new(TABLE_ID_CANARY, TABLE_LABEL_CANARY, state)
        .behavior_snapshot(ui_px(0.0), ui_px(320.0))
}

fn sensitive_source_forms(snapshot: &TableBehaviorSnapshot) -> Vec<String> {
    let raw = [
        TABLE_ID_CANARY,
        TABLE_LABEL_CANARY,
        COLUMN_ID_CANARY,
        COLUMN_LABEL_CANARY,
        ROW_ID_CANARY,
        INSTANCE_ID_CANARY,
        GROUP_VALUE_CANARY,
        CELL_VALUE_CANARY,
        NEXT_ROW_CANARY,
    ];
    let mut forms = raw
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();

    for namespace in ["table", "table-column", "table-row", "table-cell"] {
        forms.extend(raw.iter().map(|value| opaque_stable_id(namespace, value)));
    }
    for row in snapshot.rows() {
        forms.push(format!("{:?}", row.identity()));
        forms.push(row.identity().debug_label());
        forms.push(row.identity().key().as_str().to_owned());
        forms.push(TableDebugSelector::row(TABLE_ID_CANARY, row.identity()));
        for cell in row.cells() {
            forms.push(TableDebugSelector::cell(
                TABLE_ID_CANARY,
                row.identity(),
                cell.column_id(),
            ));
        }
    }
    forms.extend(
        snapshot
            .row_identity_diagnostics()
            .iter()
            .map(|diagnostic| format!("{diagnostic:?}")),
    );
    forms.sort();
    forms.dedup();
    forms
}
