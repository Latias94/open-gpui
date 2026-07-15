use super::*;

#[test]
fn table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("notes", "Line 1\nLine 2")
            .with_cell("enabled", true)
            .with_cell("status", "ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-b").with_cell("score", 20_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("notes", "Notes").with_multiline_text_editor(3),
        TableColumn::new("enabled", "Enabled").with_checkbox_editor(),
        TableColumn::new("status", "Status")
            .with_select_editor([
                TableSelectOption::new("ready", "Ready"),
                TableSelectOption::new("blocked", "Blocked"),
            ])
            .with_width(ui_px(120.0)),
        TableColumn::new("score", "Score"),
    ])
    .with_grouping(["score"])
    .with_all_rows_expanded()
    .with_pagination(TablePagination::disabled());
    let snapshot = Table::new("editable-plan-table", "Editable plan table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(120.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(120.0));

    let name_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "name")
        .expect("name column should resolve");
    let score_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "score")
        .expect("score column should resolve");
    let notes_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "notes")
        .expect("notes column should resolve");
    let enabled_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "enabled")
        .expect("enabled column should resolve");
    let status_column = snapshot
        .columns()
        .iter()
        .find(|column| column.id().as_str() == "status")
        .expect("status column should resolve");
    assert!(name_column.text_editable());
    assert_eq!(name_column.editor(), Some(TableCellEditor::Text));
    assert!(notes_column.text_editable());
    assert_eq!(
        notes_column.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert!(enabled_column.text_editable());
    assert_eq!(enabled_column.editor(), Some(TableCellEditor::Checkbox));
    assert_eq!(status_column.editor(), Some(TableCellEditor::Select));
    assert_eq!(status_column.select_options().len(), 2);
    assert_eq!(status_column.select_options()[0].value(), "ready");
    assert_eq!(status_column.select_options()[0].label(), "Ready");
    assert!(!score_column.text_editable());
    assert_eq!(score_column.editor(), None);

    let group_row = snapshot
        .rows()
        .iter()
        .find(|row| row.is_group())
        .expect("group row should resolve");
    let group_name_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("group name cell should resolve");
    assert!(
        !group_name_cell.text_editable(),
        "synthetic grouped rows must stay display-only"
    );
    let group_notes_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("group notes cell should resolve");
    assert_eq!(group_notes_cell.editor(), None);
    let group_enabled_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("group enabled cell should resolve");
    assert_eq!(group_enabled_cell.editor(), None);
    let group_status_cell = group_row
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("group status cell should resolve");
    assert_eq!(group_status_cell.editor(), None);

    let editable_leaf = snapshot
        .rows()
        .iter()
        .find(|row| row.source_row_id().is_some_and(|id| id.as_str() == "row-a"))
        .expect("row-a should resolve");
    let editable_name = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-a name cell should resolve");
    assert!(editable_name.text_editable());
    let editable_notes = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "notes")
        .expect("row-a notes cell should resolve");
    assert_eq!(
        editable_notes.editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    let editable_enabled = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("row-a enabled cell should resolve");
    assert_eq!(editable_enabled.editor(), Some(TableCellEditor::Checkbox));
    assert_eq!(editable_enabled.value(), Some(&TableCellValue::Bool(true)));
    let editable_status = editable_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("row-a status cell should resolve");
    assert_eq!(editable_status.editor(), Some(TableCellEditor::Select));
    assert_eq!(editable_status.text(), "Ready");
    assert_eq!(editable_status.select_options().len(), 2);
    assert_eq!(editable_status.select_options()[1].value(), "blocked");

    let missing_leaf = snapshot
        .rows()
        .iter()
        .find(|row| row.source_row_id().is_some_and(|id| id.as_str() == "row-b"))
        .expect("row-b should resolve");
    let missing_name = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "name")
        .expect("row-b missing name cell should resolve");
    assert!(!missing_name.text_editable());
    let missing_enabled = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "enabled")
        .expect("row-b missing enabled cell should resolve");
    assert_eq!(missing_enabled.editor(), None);
    let missing_status = missing_leaf
        .cells()
        .iter()
        .find(|cell| cell.column_id().as_str() == "status")
        .expect("row-b missing status cell should resolve");
    assert_eq!(missing_status.editor(), None);
}

#[test]
fn table_cell_edit_change_updates_source_row_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("name", "Root")
            .with_cell("team", "Platform")
            .with_child(
                TableRow::new("child")
                    .with_cell("name", "Child")
                    .with_cell("team", "UI"),
            ),
        TableRow::new("other")
            .with_cell("name", "Other")
            .with_cell("team", "Ops"),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("team", "Team"),
    ])
    .with_column_order(["team", "name"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_filters([TableFilter::contains("team", "UI")])
    .with_sorting([TableSort::ascending("name")])
    .with_expanded_rows([TableRowIdentity::source("root")])
    .with_selected_rows(["child"])
    .with_pagination(TablePagination::new(2, 25));

    let change = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("child"),
        "name",
        "Child",
        "Child Prime",
    );

    let (next, outcome) = change.apply_to(state.clone());
    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(next.column_order()[0].as_str(), "team");
    assert_eq!(next.column_pinning().left()[0].as_str(), "name");
    assert_eq!(next.filters()[0].query(), "UI");
    assert_eq!(next.sorting()[0].column().as_str(), "name");
    assert_eq!(next.expansion(), state.expansion());
    assert!(next.selected_rows().contains(&TableRowId::new("child")));
    assert_eq!(next.pagination().page_index(), 2);

    let updated = next
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "root")
        .and_then(|row| row.children().first())
        .expect("nested child should remain nested");
    assert_eq!(
        updated
            .cell(&TableColumnId::new("name"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Child Prime")
    );

    let missing_column = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("child"),
        "missing",
        "old",
        "new",
    );
    let (missing_column_state, missing_outcome) = missing_column.apply_to(next.clone());
    assert_eq!(missing_outcome, TableCellEditApplyOutcome::CellNotFound);
    assert_eq!(missing_column_state, next);
    assert_eq!(
        missing_column_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing cell edits should be inspectable no-ops"
    );

    let missing_row = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("missing-row"),
        "name",
        "old",
        "new",
    );
    let (missing_row_state, missing_row_outcome) = missing_row.apply_to(next.clone());
    assert_eq!(missing_row_outcome, TableCellEditApplyOutcome::RowNotFound);
    assert_eq!(missing_row_state, next);
    assert_eq!(
        missing_row_state.cache_key().rows_identity(),
        next.cache_key().rows_identity(),
        "missing row edits should be inspectable no-ops"
    );
}

#[test]
fn table_cell_edit_change_targets_grouped_duplicate_by_resolved_identity() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("team", "Root")
            .with_cell("name", "Root")
            .with_child(
                TableRow::new("duplicate")
                    .with_instance_id("nested")
                    .with_cell("team", "Nested")
                    .with_cell("name", "Nested duplicate"),
            ),
        TableRow::new("duplicate")
            .with_instance_id("top")
            .with_cell("team", "Top")
            .with_cell("name", "Top duplicate"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("name", "Name").with_text_editable(true),
    ])
    .with_grouping(["team"])
    .with_all_rows_expanded();
    let target = TableSourceRowIdentity::explicit("duplicate", "top");
    let change =
        TableCellEditRequest::new(target, "name", "Top duplicate", "Updated top duplicate");

    let (next, outcome) = change.apply_to(state);

    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(
        next.rows()[0].children()[0]
            .cell(&TableColumnId::new("name"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Nested duplicate")
    );
    assert_eq!(
        next.rows()[1]
            .cell(&TableColumnId::new("name"))
            .map(TableCellValue::filter_text)
            .as_deref(),
        Some("Updated top duplicate")
    );
}

#[test]
fn ambiguous_business_id_edit_is_an_inspectable_no_op() {
    let state = TableState::new([
        TableRow::new("duplicate")
            .with_instance_id("first")
            .with_cell("name", "First"),
        TableRow::new("duplicate")
            .with_instance_id("second")
            .with_cell("name", "Second"),
    ]);
    let change = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("duplicate"),
        "name",
        "First",
        "Updated",
    );

    let (next, outcome) = change.apply_to(state.clone());

    assert_eq!(outcome, TableCellEditApplyOutcome::AmbiguousRowId);
    assert_eq!(next, state);
    assert_eq!(
        next.cache_key().rows_identity(),
        state.cache_key().rows_identity(),
        "ambiguous edits must not invalidate row or adapter caches"
    );
}

#[test]
fn stale_occurrence_edit_cannot_retarget_a_reordered_duplicate() {
    let state = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
    ]);
    let target = state
        .source_row_identity_at("duplicate", 1)
        .expect("second duplicate should resolve in the source snapshot");
    let change = TableCellEditRequest::new(target, "name", "Second", "Updated second");
    let reordered = state.with_rows([
        TableRow::new("duplicate").with_cell("name", "Second"),
        TableRow::new("duplicate").with_cell("name", "First"),
    ]);

    let (next, outcome) = change.apply_to(reordered.clone());

    assert_eq!(outcome, TableCellEditApplyOutcome::StaleRowIdentity);
    assert_eq!(next, reordered);
    assert_eq!(
        next.cache_key().rows_identity(),
        reordered.cache_key().rows_identity(),
        "stale occurrence edits must leave the current snapshot untouched"
    );
}

#[test]
fn table_cell_edit_change_updates_boolean_source_row_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("root")
            .with_cell("name", "Root")
            .with_cell("team", "Platform")
            .with_cell("enabled", true)
            .with_child(
                TableRow::new("child")
                    .with_cell("name", "Child")
                    .with_cell("team", "UI")
                    .with_cell("enabled", true),
            ),
        TableRow::new("other")
            .with_cell("name", "Other")
            .with_cell("team", "Ops")
            .with_cell("enabled", false),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_text_editable(true),
        TableColumn::new("team", "Team"),
        TableColumn::new("enabled", "Enabled").with_checkbox_editor(),
    ])
    .with_column_order(["team", "enabled", "name"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_filters([TableFilter::contains("team", "UI")])
    .with_sorting([TableSort::ascending("name")])
    .with_expanded_rows([TableRowIdentity::source("root")])
    .with_selected_rows(["child"])
    .with_pagination(TablePagination::new(2, 25));

    let change = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("child"),
        "enabled",
        true,
        false,
    );

    let (next, outcome) = change.apply_to(state.clone());
    assert_eq!(outcome, TableCellEditApplyOutcome::Updated);
    assert_eq!(change.previous_value(), &TableCellValue::Bool(true));
    assert_eq!(change.next_value(), &TableCellValue::Bool(false));
    assert_eq!(change.previous_text(), "true");
    assert_eq!(change.next_text(), "false");
    assert_eq!(next.column_order()[0].as_str(), "team");
    assert_eq!(next.column_pinning().left()[0].as_str(), "name");
    assert_eq!(next.filters()[0].query(), "UI");
    assert_eq!(next.sorting()[0].column().as_str(), "name");
    assert_eq!(next.expansion(), state.expansion());
    assert!(next.selected_rows().contains(&TableRowId::new("child")));
    assert_eq!(next.pagination().page_index(), 2);

    let updated = next
        .rows()
        .iter()
        .find(|row| row.id().as_str() == "root")
        .and_then(|row| row.children().first())
        .expect("nested child should remain nested");
    assert_eq!(
        updated.cell(&TableColumnId::new("enabled")),
        Some(&TableCellValue::Bool(false))
    );

    let missing_column = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("child"),
        "missing",
        true,
        false,
    );
    let (missing_column_state, missing_outcome) = missing_column.apply_to(next.clone());
    assert_eq!(missing_outcome, TableCellEditApplyOutcome::CellNotFound);
    assert_eq!(missing_column_state, next);

    let missing_row = TableCellEditRequest::new(
        TableSourceRowIdentity::unique("missing-row"),
        "enabled",
        true,
        false,
    );
    let (missing_row_state, missing_row_outcome) = missing_row.apply_to(next.clone());
    assert_eq!(missing_row_outcome, TableCellEditApplyOutcome::RowNotFound);
    assert_eq!(missing_row_state, next);
}
