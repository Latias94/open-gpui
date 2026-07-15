use super::*;

#[test]
fn leaf_header_identity_covers_its_logical_column() {
    let identity = TableResolvedHeaderIdentity::leaf("name");

    assert_eq!(
        identity.logical(),
        &TableHeaderIdentity::Leaf(TableColumnId::new("name"))
    );
    assert_eq!(identity.covered_leaves(), [TableColumnId::new("name")]);
}

// Compile guard: downstream const helpers must remain able to delegate to `col_span`.
const fn resolved_header_col_span(cell: &TableResolvedHeaderCell) -> usize {
    cell.col_span()
}

#[test]
fn resolved_header_col_span_remains_const_compatible() {
    let col_span: fn(&TableResolvedHeaderCell) -> usize = resolved_header_col_span;
    let resolved = TableState::new(sample_rows())
        .with_columns([TableColumn::new("name", "Name")])
        .resolve();

    assert_eq!(
        col_span(&resolved.center_header_groups()[0].headers()[0]),
        1
    );
}

#[test]
fn placeholder_identity_does_not_depend_on_pinning_region() {
    let state = TableState::new(sample_rows()).with_column_tree([
        TableColumnGroup::new(
            "identity",
            "Identity",
            [
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team"),
            ],
        ),
        TableColumnGroup::new(
            "metrics",
            "Metrics",
            [TableColumnGroup::new(
                "scores",
                "Scores",
                [
                    TableColumn::new("score", "Score"),
                    TableColumn::new("status", "Status"),
                ],
            )],
        ),
    ]);
    let unpinned = state.resolve();
    let unpinned_placeholder = unpinned
        .center_header_groups()
        .iter()
        .flat_map(TableResolvedHeaderGroup::headers)
        .find(|header| {
            header.is_placeholder() && header.leaf_column_ids() == [TableColumnId::new("name")]
        })
        .expect("name placeholder should resolve")
        .identity()
        .clone();

    let pinned = state
        .with_column_pinning(TableColumnPinning::new().pinned_left(["name", "score"]))
        .resolve();
    let pinned_placeholder = pinned
        .left_header_groups()
        .iter()
        .flat_map(TableResolvedHeaderGroup::headers)
        .find(|header| {
            header.is_placeholder() && header.leaf_column_ids() == [TableColumnId::new("name")]
        })
        .expect("pinned name placeholder should resolve")
        .identity();

    assert_eq!(pinned_placeholder, &unpinned_placeholder);
}
