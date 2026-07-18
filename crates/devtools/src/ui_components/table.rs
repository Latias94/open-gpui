use std::{collections::HashMap, hash::Hash};

use open_gpui_ui_components::table::{
    TableBehaviorSnapshot, TableCellBehaviorSnapshot, TableColumnBehaviorSnapshot,
    TableRowBehaviorSnapshot,
};
use open_gpui_ui_core::{
    TableCellEditor, TableColumnId, TableRowChildrenLoadState, TableRowId, TableRowIdentity,
    TableRowIdentityDiagnostic, TableRowInstanceId, TableStageMode,
};
use serde::Serialize;

use crate::{
    DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT, SnapshotProbeSnapshot, SnapshotRedactionSummary,
    SnapshotTree, adapters::snapshot_node_with_payload,
};

/// Session-local Table identity projector for DevTools snapshots.
///
/// Caller-owned table, column, row, instance, group, and cell identities remain typed map keys in
/// memory. The exported tree contains only sequential ordinals whose meaning is scoped to this
/// projector's lifetime. No source string, formatted identity, or deterministic hash is persisted.
pub struct TableDevtoolsSession {
    projection_generation: u64,
    identity_retention: u64,
    next_table_ordinal: u64,
    tables: HashMap<String, TableIdentityScope>,
}

impl Default for TableDevtoolsSession {
    fn default() -> Self {
        Self {
            projection_generation: 0,
            identity_retention: u64::try_from(DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT)
                .unwrap_or(u64::MAX),
            next_table_ordinal: 0,
            tables: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for TableDevtoolsSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TableDevtoolsSession")
            .field("table_count", &self.tables.len())
            .field(
                "column_identity_count",
                &self
                    .tables
                    .values()
                    .map(|scope| scope.columns.len())
                    .sum::<usize>(),
            )
            .field(
                "row_identity_count",
                &self
                    .tables
                    .values()
                    .map(|scope| scope.rows.len())
                    .sum::<usize>(),
            )
            .finish()
    }
}

impl TableDevtoolsSession {
    /// Bounds source identity retention to the latest `projection_count` snapshot calls.
    ///
    /// Set this to the owning [`crate::DevtoolsSession`] history limit when one Table projection is
    /// produced per frame. Identities absent from that bounded window are released and receive new
    /// opaque ordinals if they later reappear.
    pub fn with_identity_retention(mut self, projection_count: usize) -> Self {
        self.identity_retention = u64::try_from(projection_count.max(1)).unwrap_or(u64::MAX);
        self.prune_identities();
        self
    }

    /// Projects one resolved Table behavior snapshot into the current DevTools identity session.
    pub fn snapshot(&mut self, snapshot: &TableBehaviorSnapshot) -> SnapshotProbeSnapshot {
        self.projection_generation = next_ordinal(
            &mut self.projection_generation,
            "Table identity projection generation",
        );
        let generation = self.projection_generation;
        let projection = {
            let scope = self.table_scope(snapshot.table_id(), generation);
            table_probe_snapshot(scope, snapshot, generation)
        };
        self.prune_identities();
        projection
    }

    fn table_scope(&mut self, source_id: &str, generation: u64) -> &mut TableIdentityScope {
        if !self.tables.contains_key(source_id) {
            let ordinal = next_ordinal(&mut self.next_table_ordinal, "table");
            self.tables.insert(
                source_id.to_owned(),
                TableIdentityScope::new(ordinal, generation),
            );
        }
        let scope = self
            .tables
            .get_mut(source_id)
            .expect("inserted Table identity scope must exist");
        scope.last_seen_generation = generation;
        scope
    }

    fn prune_identities(&mut self) {
        let generation = self.projection_generation;
        let retention = self.identity_retention;
        self.tables.retain(|_, scope| {
            scope.prune(generation, retention);
            retained(scope.last_seen_generation, generation, retention)
        });
    }
}

struct TableIdentityScope {
    table_ordinal: u64,
    last_seen_generation: u64,
    next_column_ordinal: u64,
    next_row_ordinal: u64,
    next_diagnostic_ordinal: u64,
    columns: HashMap<TableColumnId, RetainedOrdinal>,
    rows: HashMap<TableRowIdentity, RetainedOrdinal>,
    diagnostics: HashMap<TableDiagnosticIdentity, RetainedOrdinal>,
}

impl TableIdentityScope {
    fn new(table_ordinal: u64, generation: u64) -> Self {
        Self {
            table_ordinal,
            last_seen_generation: generation,
            next_column_ordinal: 0,
            next_row_ordinal: 0,
            next_diagnostic_ordinal: 0,
            columns: HashMap::new(),
            rows: HashMap::new(),
            diagnostics: HashMap::new(),
        }
    }

    fn table_id(&self) -> String {
        opaque_label("table", self.table_ordinal)
    }

    fn column_id(&mut self, source_id: &TableColumnId, generation: u64) -> String {
        let ordinal = retained_ordinal(
            &mut self.columns,
            &mut self.next_column_ordinal,
            source_id,
            generation,
            "Table column",
        );
        opaque_label("column", ordinal)
    }

    fn row_id(&mut self, source_id: &TableRowIdentity, generation: u64) -> String {
        let ordinal = retained_ordinal(
            &mut self.rows,
            &mut self.next_row_ordinal,
            source_id,
            generation,
            "Table row",
        );
        opaque_label("row", ordinal)
    }

    fn diagnostic_id(
        &mut self,
        diagnostic: &TableRowIdentityDiagnostic,
        generation: u64,
    ) -> String {
        let identity = TableDiagnosticIdentity::from(diagnostic);
        let ordinal = retained_ordinal(
            &mut self.diagnostics,
            &mut self.next_diagnostic_ordinal,
            &identity,
            generation,
            "Table identity diagnostic",
        );
        opaque_label("diagnostic", ordinal)
    }

    fn prune(&mut self, generation: u64, retention: u64) {
        self.columns
            .retain(|_, entry| retained(entry.last_seen_generation, generation, retention));
        self.rows
            .retain(|_, entry| retained(entry.last_seen_generation, generation, retention));
        self.diagnostics
            .retain(|_, entry| retained(entry.last_seen_generation, generation, retention));
    }
}

#[derive(Clone, Copy)]
struct RetainedOrdinal {
    ordinal: u64,
    last_seen_generation: u64,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TableDiagnosticIdentity {
    DuplicateRowId(TableRowId),
    DuplicateSourceInstance(TableRowId, TableRowInstanceId),
}

impl From<&TableRowIdentityDiagnostic> for TableDiagnosticIdentity {
    fn from(diagnostic: &TableRowIdentityDiagnostic) -> Self {
        match diagnostic {
            TableRowIdentityDiagnostic::DuplicateRowId { row_id, .. } => {
                Self::DuplicateRowId(row_id.clone())
            }
            TableRowIdentityDiagnostic::DuplicateSourceInstance {
                row_id,
                instance_id,
                ..
            } => Self::DuplicateSourceInstance(row_id.clone(), instance_id.clone()),
        }
    }
}

fn retained_ordinal<K>(
    identities: &mut HashMap<K, RetainedOrdinal>,
    next: &mut u64,
    source_id: &K,
    generation: u64,
    kind: &str,
) -> u64
where
    K: Clone + Eq + Hash,
{
    match identities.get_mut(source_id) {
        Some(entry) => {
            entry.last_seen_generation = generation;
            entry.ordinal
        }
        None => {
            let ordinal = next_ordinal(next, kind);
            identities.insert(
                source_id.clone(),
                RetainedOrdinal {
                    ordinal,
                    last_seen_generation: generation,
                },
            );
            ordinal
        }
    }
}

fn retained(last_seen_generation: u64, generation: u64, retention: u64) -> bool {
    generation.saturating_sub(last_seen_generation) < retention
}

fn table_probe_snapshot(
    scope: &mut TableIdentityScope,
    snapshot: &TableBehaviorSnapshot,
    generation: u64,
) -> SnapshotProbeSnapshot {
    let table_id = scope.table_id();
    let rows = snapshot.row_counts();
    let visible = snapshot.visible_rows();
    let column_regions = snapshot.column_regions();
    let header = snapshot.header_summary();
    let tree = snapshot.tree_summary();
    let mut redaction = SnapshotRedactionSummary::default();
    redaction.record_redacted("Table caller-owned id redacted");
    redaction.record_redacted("Table accessible label redacted");
    for _ in snapshot.grouping_columns() {
        redaction.record_redacted("Table grouping column id redacted");
    }
    for _ in snapshot.column_facets() {
        redaction.record_redacted("Table facet identity and values redacted");
    }

    let mut root = snapshot_node_with_payload(
        ["table", table_id.as_str()],
        "Table behavior",
        serde_json::json!({
            "table_id": table_id,
            "label": SensitiveTableMarker::Redacted,
            "roles": {
                "root": "table",
                "row": "row",
                "column_header": "column-header",
                "cell": "cell",
            },
            "stages": {
                "filtering": table_stage_mode_label(snapshot.filtering_mode()),
                "sorting": table_stage_mode_label(snapshot.sorting_mode()),
                "pagination": table_stage_mode_label(snapshot.pagination_mode()),
                "faceting": table_stage_mode_label(snapshot.faceting_mode()),
            },
            "pagination": {
                "page_index": snapshot.pagination_page_index(),
                "page_size": snapshot.pagination_page_size(),
                "row_count": snapshot.pagination_row_count(),
                "page_count": snapshot.pagination_page_count(),
            },
            "rows": {
                "core": rows.core_rows(),
                "filtered": rows.filtered_rows(),
                "grouped": rows.grouped_rows(),
                "sorted": rows.sorted_rows(),
                "expanded": rows.expanded_rows(),
                "paginated": rows.paginated_rows(),
                "final": rows.final_rows(),
                "pinned_top": rows.pinned_top_rows(),
                "pinned_center": rows.pinned_center_rows(),
                "pinned_bottom": rows.pinned_bottom_rows(),
                "rendered": rows.rendered_rows(),
                "visible": rows.visible_rows(),
                "selected": rows.selected_rows(),
                "groups": rows.group_rows(),
                "leaves": rows.leaf_rows(),
                "aria": rows.aria_rows(),
            },
            "visible_window": {
                "visible_start": visible.visible_start(),
                "visible_end": visible.visible_end(),
                "overscan_start": visible.overscan_start(),
                "overscan_end": visible.overscan_end(),
                "center_overscan_count": visible.center_overscan_count(),
            },
            "columns": {
                "left": column_regions.left_columns(),
                "center": column_regions.center_columns(),
                "right": column_regions.right_columns(),
                "aria": column_regions.aria_columns(),
                "resizable": column_regions.resizable_columns(),
                "split_pinned_lanes": column_regions.uses_split_pinned_columns(),
                "row_pinning_page_only": column_regions.row_pinning_page_only(),
            },
            "header": {
                "rows": header.header_rows(),
                "visible_groups": header.visible_group_headers(),
            },
            "tree": {
                "rows": tree.tree_rows(),
                "branches": tree.tree_branch_rows(),
                "unloaded_branches": tree.unloaded_tree_branches(),
                "loading_rows": tree.loading_tree_rows(),
                "failed_rows": tree.failed_tree_rows(),
                "max_depth": tree.tree_depth(),
            },
            "configuration": {
                "row_measurement": if snapshot.row_measure_mode().measured() { "measured" } else { "fixed" },
                "grouping_columns": snapshot.grouping_columns().len(),
                "aggregations": snapshot.aggregation_count(),
                "custom_aggregation_functions": snapshot.aggregation_fn_count(),
                "manual_expansion": snapshot.manual_expansion(),
                "all_rows_expanded": snapshot.all_rows_expanded(),
                "expanded_group_inputs": snapshot.expanded_group_inputs(),
                "expanded_tree_inputs": snapshot.expanded_tree_inputs(),
                "facet_columns": snapshot.column_facets().len(),
            },
            "identity_diagnostic_count": snapshot.row_identity_diagnostics().len(),
        }),
    );

    for diagnostic in snapshot.row_identity_diagnostics() {
        root = root.with_child(identity_diagnostic_node(
            scope,
            diagnostic,
            generation,
            &mut redaction,
        ));
    }
    for column in snapshot.columns() {
        root = root.with_child(column_node(scope, column, generation, &mut redaction));
    }
    for row in snapshot.rows() {
        root = root.with_child(row_node(scope, row, generation, &mut redaction));
    }

    SnapshotProbeSnapshot::new(SnapshotTree::new([root])).with_redaction(redaction)
}

fn identity_diagnostic_node(
    scope: &mut TableIdentityScope,
    diagnostic: &TableRowIdentityDiagnostic,
    generation: u64,
    redaction: &mut SnapshotRedactionSummary,
) -> crate::SnapshotNode {
    let diagnostic_id = scope.diagnostic_id(diagnostic, generation);
    let (kind, occurrences, redacted_sources) = match diagnostic {
        TableRowIdentityDiagnostic::DuplicateRowId { occurrences, .. } => {
            ("duplicate-row-id", *occurrences, 1)
        }
        TableRowIdentityDiagnostic::DuplicateSourceInstance { occurrences, .. } => {
            ("duplicate-source-instance", *occurrences, 2)
        }
    };
    for _ in 0..redacted_sources {
        redaction.record_redacted("Table identity diagnostic source id redacted");
    }

    snapshot_node_with_payload(
        ["table", "identity-diagnostic", diagnostic_id.as_str()],
        "Table identity diagnostic",
        serde_json::json!({
            "diagnostic_id": diagnostic_id,
            "kind": kind,
            "occurrences": occurrences,
            "source_identity": SensitiveTableMarker::Redacted,
        }),
    )
}

fn column_node(
    scope: &mut TableIdentityScope,
    column: &TableColumnBehaviorSnapshot,
    generation: u64,
    redaction: &mut SnapshotRedactionSummary,
) -> crate::SnapshotNode {
    let column_id = scope.column_id(column.id(), generation);
    redaction.record_redacted("Table column id redacted");
    redaction.record_redacted("Table column label redacted");
    record_select_option_redactions(column.select_options().len(), redaction);

    snapshot_node_with_payload(
        ["table", "column", column_id.as_str()],
        "Table column",
        serde_json::json!({
            "column_id": column_id,
            "label": SensitiveTableMarker::Redacted,
            "role": "column-header",
            "region": column.region().as_str(),
            "aria_column_index": column.aria_column_index(),
            "sortable": column.sortable(),
            "resizable": column.resizable(),
            "width_policy": column.width_policy().as_str(),
            "width_px": column.width().as_f32(),
            "sort_direction": column.sort_direction().map(|direction| direction.as_str()),
            "editor": column.editor().map(table_cell_editor_summary),
            "select_option_count": column.select_options().len(),
            "actions": if column.sort_action().is_some() { &["sort"][..] } else { &[] },
        }),
    )
}

fn row_node(
    scope: &mut TableIdentityScope,
    row: &TableRowBehaviorSnapshot,
    generation: u64,
    redaction: &mut SnapshotRedactionSummary,
) -> crate::SnapshotNode {
    let row_id = scope.row_id(row.identity(), generation);
    redaction.record_redacted("Table row identity redacted");
    let load_state = row
        .children_load_state()
        .map(|state| table_load_state_summary(state, redaction));
    let mut node = snapshot_node_with_payload(
        ["table", "row", row_id.as_str()],
        "Table row",
        serde_json::json!({
            "row_id": row_id,
            "role": "row",
            "kind": if row.is_group() { "group" } else { "source" },
            "region": row.region().as_str(),
            "model_index": row.model_index(),
            "region_index": row.region_index(),
            "aria_row_index": row.aria_row_index(),
            "selected": row.selected(),
            "depth": row.depth(),
            "tree_branch": row.is_tree_branch(),
            "tree_expanded": row.tree_expanded(),
            "loaded_child_count": row.loaded_child_count(),
            "children_load_state": load_state,
            "cell_count": row.cells().len(),
        }),
    );

    for cell in row.cells() {
        node = node.with_child(cell_node(scope, &row_id, cell, generation, redaction));
    }
    node
}

fn cell_node(
    scope: &mut TableIdentityScope,
    row_id: &str,
    cell: &TableCellBehaviorSnapshot,
    generation: u64,
    redaction: &mut SnapshotRedactionSummary,
) -> crate::SnapshotNode {
    let column_id = scope.column_id(cell.column_id(), generation);
    let cell_id = format!("cell-{row_id}-{column_id}");
    redaction.record_redacted("Table cell column id redacted");
    redaction.record_redacted("Table cell display text redacted");
    if cell.value().is_some() {
        redaction.record_redacted("Table cell value redacted");
    }
    record_select_option_redactions(cell.select_options().len(), redaction);

    snapshot_node_with_payload(
        ["table", "cell", cell_id.as_str()],
        "Table cell",
        serde_json::json!({
            "cell_id": cell_id,
            "column_id": column_id,
            "role": "cell",
            "text": SensitiveTableMarker::Redacted,
            "value": if cell.value().is_some() { SensitiveTableMarker::Redacted } else { SensitiveTableMarker::Absent },
            "region": cell.region().as_str(),
            "aria_column_index": cell.aria_column_index(),
            "width_px": cell.width().as_f32(),
            "editor": cell.editor().map(table_cell_editor_summary),
            "select_option_count": cell.select_options().len(),
        }),
    )
}

fn table_load_state_summary(
    state: &TableRowChildrenLoadState,
    redaction: &mut SnapshotRedactionSummary,
) -> &'static str {
    match state {
        TableRowChildrenLoadState::Idle => "idle",
        TableRowChildrenLoadState::Loading { .. } => {
            redaction.record_redacted("Table child-loading message redacted");
            "loading"
        }
        TableRowChildrenLoadState::Failed { .. } => {
            redaction.record_redacted("Table child-loading failure redacted");
            "failed"
        }
    }
}

fn table_cell_editor_summary(editor: TableCellEditor) -> serde_json::Value {
    match editor {
        TableCellEditor::Text => serde_json::json!({ "kind": "text" }),
        TableCellEditor::MultilineText { rows } => {
            serde_json::json!({ "kind": "multiline-text", "rows": rows })
        }
        TableCellEditor::Checkbox => serde_json::json!({ "kind": "checkbox" }),
        TableCellEditor::Select => serde_json::json!({ "kind": "select" }),
    }
}

fn record_select_option_redactions(count: usize, redaction: &mut SnapshotRedactionSummary) {
    for _ in 0..count {
        redaction.record_redacted("Table select option value redacted");
        redaction.record_redacted("Table select option label redacted");
    }
}

const fn table_stage_mode_label(mode: TableStageMode) -> &'static str {
    match mode {
        TableStageMode::Client => "client",
        TableStageMode::Manual => "manual",
    }
}

fn next_ordinal(next: &mut u64, kind: &str) -> u64 {
    *next = next
        .checked_add(1)
        .unwrap_or_else(|| panic!("{kind} opaque ordinal space exhausted"));
    *next
}

fn opaque_label(kind: &str, ordinal: u64) -> String {
    format!("{kind}-{ordinal}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SensitiveTableMarker {
    Absent,
    Redacted,
}
