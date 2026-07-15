use super::*;

#[test]
fn row_pinning_exact_duplicate_instance_and_business_bulk_targets_are_distinct() {
    let state = TableState::new([
        TableRow::new("duplicate")
            .with_instance_id("first")
            .with_cell("name", "First"),
        TableRow::new("unique").with_cell("name", "Middle"),
        TableRow::new("duplicate")
            .with_instance_id("second")
            .with_cell("name", "Second"),
    ]);

    let exact = state
        .clone()
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top([TableRowIdentity::source_instance("duplicate", "second")]),
        )
        .resolve();
    assert_eq!(
        exact
            .row_regions()
            .top()
            .iter()
            .map(|row| row.cell(&TableColumnId::new("name")).unwrap().filter_text())
            .collect::<Vec<_>>(),
        ["Second"]
    );

    let bulk = state
        .with_row_pinning(
            TableRowPinning::new().pinned_top([TableRowPinTarget::all_source_rows("duplicate")]),
        )
        .resolve();
    assert_eq!(
        bulk.row_regions()
            .top()
            .iter()
            .map(|row| row.cell(&TableColumnId::new("name")).unwrap().filter_text())
            .collect::<Vec<_>>(),
        ["First", "Second"]
    );
}

#[test]
fn row_pinning_exact_target_does_not_expand_an_ambiguous_business_id() {
    let resolved = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
    ])
    .with_row_pinning(TableRowPinning::new().pinned_top([TableRowIdentity::source("duplicate")]))
    .resolve();

    assert!(resolved.row_regions().top().is_empty());
    assert_eq!(
        row_ids(resolved.row_regions().center()),
        ["duplicate", "duplicate"]
    );
}

#[test]
fn row_pinning_bulk_target_preserves_current_model_order() {
    let name = TableColumnId::new("name");
    let resolved = TableState::new([
        TableRow::new("duplicate")
            .with_instance_id("first")
            .with_cell("name", "First")
            .with_cell("score", 10_usize),
        TableRow::new("unique")
            .with_cell("name", "Middle")
            .with_cell("score", 20_usize),
        TableRow::new("duplicate")
            .with_instance_id("second")
            .with_cell("name", "Second")
            .with_cell("score", 30_usize),
    ])
    .with_columns([TableColumn::new("score", "Score")])
    .with_sorting([TableSort::descending("score")])
    .with_row_pinning(
        TableRowPinning::new().pinned_top([TableRowPinTarget::all_source_rows("duplicate")]),
    )
    .resolve();

    assert_eq!(
        resolved
            .row_regions()
            .top()
            .iter()
            .map(|row| row.cell(&name).unwrap().filter_text())
            .collect::<Vec<_>>(),
        ["Second", "First"]
    );
}

#[test]
fn row_pinning_uses_caller_target_order_and_top_wins_overlap() {
    let resolved = TableState::new(sample_rows())
        .with_row_pinning(
            TableRowPinning::new()
                .pinned_top([source_identity("row-c"), source_identity("row-b")])
                .pinned_bottom([source_identity("row-b"), source_identity("row-a")]),
        )
        .resolve();

    assert_eq!(row_ids(resolved.row_regions().top()), ["row-c", "row-b"]);
    assert_eq!(row_ids(resolved.row_regions().bottom()), ["row-a"]);
    assert!(resolved.row_regions().center().is_empty());
}

#[test]
fn row_pinning_mixes_exact_and_bulk_targets_without_losing_order_or_precedence() {
    let resolved = TableState::new([
        TableRow::new("duplicate")
            .with_instance_id("first")
            .with_cell("name", "First duplicate"),
        TableRow::new("center").with_cell("name", "Center"),
        TableRow::new("duplicate")
            .with_instance_id("second")
            .with_cell("name", "Second duplicate"),
        TableRow::new("top-exact").with_cell("name", "Top exact"),
        TableRow::new("duplicate")
            .with_instance_id("third")
            .with_cell("name", "Third duplicate"),
        TableRow::new("bottom-exact").with_cell("name", "Bottom exact"),
        TableRow::new("top-tail").with_cell("name", "Top tail"),
    ])
    .with_row_pinning(
        TableRowPinning::new()
            .pinned_top([
                TableRowPinTarget::exact(TableRowIdentity::source("top-exact")),
                TableRowPinTarget::all_source_rows("duplicate"),
                TableRowPinTarget::exact(TableRowIdentity::source_instance("duplicate", "second")),
                TableRowPinTarget::exact(TableRowIdentity::source("top-tail")),
            ])
            .pinned_bottom([
                TableRowPinTarget::exact(TableRowIdentity::source_instance("duplicate", "first")),
                TableRowPinTarget::exact(TableRowIdentity::source("bottom-exact")),
                TableRowPinTarget::all_source_rows("duplicate"),
                TableRowPinTarget::exact(TableRowIdentity::source("top-tail")),
                TableRowPinTarget::exact(TableRowIdentity::source("missing")),
            ]),
    )
    .resolve();
    let name = TableColumnId::new("name");
    let names = |rows: &[TableResolvedRow]| {
        rows.iter()
            .map(|row| {
                row.cell(&name)
                    .expect("test row should have a name")
                    .filter_text()
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        names(resolved.row_regions().top()),
        [
            "Top exact",
            "First duplicate",
            "Second duplicate",
            "Third duplicate",
            "Top tail",
        ]
    );
    assert_eq!(names(resolved.row_regions().center()), ["Center"]);
    assert_eq!(names(resolved.row_regions().bottom()), ["Bottom exact"]);
}

#[test]
fn row_pinning_can_target_typed_group_rows() {
    let ops = group_identity("team", "ops");
    let resolved = TableState::new(aggregate_rows())
        .with_columns([TableColumn::new("team", "Team")])
        .with_grouping(["team"])
        .with_row_pinning(TableRowPinning::new().pinned_top([ops.clone()]))
        .resolve();

    assert_eq!(
        resolved
            .row_regions()
            .top()
            .iter()
            .map(TableResolvedRow::identity)
            .collect::<Vec<_>>(),
        [&ops]
    );
}

#[test]
fn stale_unique_pin_target_does_not_match_new_duplicate_instances() {
    let resolved = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
    ])
    .with_row_pinning(TableRowPinning::new().pinned_top([source_identity("duplicate")]))
    .resolve();

    assert!(resolved.row_regions().top().is_empty());
    assert_eq!(resolved.row_regions().center().len(), 2);
}

#[test]
fn stale_occurrence_pin_target_does_not_match_an_equal_source_replacement() {
    let rows = || {
        [
            TableRow::new("duplicate").with_cell("name", "First"),
            TableRow::new("duplicate").with_cell("name", "Second"),
        ]
    };
    let state = TableState::new(rows());
    let stale_target = TableRowIdentity::Source(
        state
            .source_row_identity_at("duplicate", 1)
            .expect("second occurrence should resolve in the initial snapshot"),
    );

    let replacement = state
        .with_rows(rows())
        .with_row_pinning(TableRowPinning::new().pinned_top([stale_target]))
        .resolve();

    assert!(replacement.row_regions().top().is_empty());
    assert_eq!(replacement.row_regions().center().len(), 2);
}
