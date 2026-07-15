use super::*;

#[test]
fn row_identity_diagnostics_distinguish_business_and_instance_collisions() {
    let state = TableState::new([
        TableRow::new("root").with_child(
            TableRow::new("duplicate")
                .with_instance_id("same-instance")
                .with_cell("name", "Nested"),
        ),
        TableRow::new("duplicate")
            .with_instance_id("same-instance")
            .with_cell("name", "Top level"),
        TableRow::new("other")
            .with_instance_id("same-instance")
            .with_cell("name", "Independent namespace"),
    ]);
    let first_duplicate = TableRowIdentity::Source(
        state
            .source_row_identity_at("duplicate", 0)
            .expect("first duplicate identity should resolve"),
    );
    let second_duplicate = TableRowIdentity::Source(
        state
            .source_row_identity_at("duplicate", 1)
            .expect("second duplicate identity should resolve"),
    );
    let resolved = state.resolve();

    assert_eq!(
        resolved.row_identity_diagnostics(),
        [
            TableRowIdentityDiagnostic::DuplicateRowId {
                row_id: TableRowId::new("duplicate"),
                occurrences: 2,
            },
            TableRowIdentityDiagnostic::DuplicateSourceInstance {
                row_id: TableRowId::new("duplicate"),
                instance_id: TableRowInstanceId::new("same-instance"),
                occurrences: 2,
            },
        ]
    );
    assert!(resolved.core_model().row(&first_duplicate).is_some());
    assert!(resolved.core_model().row(&second_duplicate).is_some());
    assert!(
        resolved
            .core_model()
            .row(&TableRowIdentity::source_instance("other", "same-instance",))
            .is_some(),
        "instance ids are scoped by business row id"
    );
}

#[test]
fn source_row_lookup_distinguishes_exact_missing_and_ambiguous_targets() {
    let state = TableState::new([
        TableRow::new("duplicate").with_instance_id("first"),
        TableRow::new("unique"),
        TableRow::new("duplicate").with_instance_id("second"),
        TableRow::new("collision").with_instance_id("same"),
        TableRow::new("collision").with_instance_id("same"),
    ]);

    assert_eq!(
        state.source_row_lookup(&TableSourceRowIdentity::unique("duplicate")),
        TableSourceRowLookup::Ambiguous
    );
    assert_eq!(
        state.source_row_lookup(&TableSourceRowIdentity::explicit("duplicate", "second")),
        TableSourceRowLookup::Found { source_index: 2 }
    );
    assert_eq!(
        state.source_row_lookup(&TableSourceRowIdentity::explicit("collision", "same")),
        TableSourceRowLookup::Ambiguous
    );
    let collision = state
        .source_row_identity_at("collision", 1)
        .expect("second colliding instance should resolve by snapshot occurrence");
    assert_eq!(
        state.source_row_lookup(&collision),
        TableSourceRowLookup::Found { source_index: 4 }
    );
    assert_eq!(
        state.source_row_lookup(&TableSourceRowIdentity::unique("missing")),
        TableSourceRowLookup::Missing
    );

    let reordered = state.with_rows([
        TableRow::new("duplicate").with_instance_id("second"),
        TableRow::new("duplicate").with_instance_id("first"),
    ]);
    assert_eq!(
        reordered.source_row_lookup(&TableSourceRowIdentity::explicit("duplicate", "second")),
        TableSourceRowLookup::Found { source_index: 0 },
        "explicit source identities survive reorder and report the new preorder index"
    );
}

#[test]
fn occurrence_identity_is_scoped_to_the_source_snapshot() {
    let state = TableState::new([
        TableRow::new("duplicate").with_cell("name", "First"),
        TableRow::new("duplicate").with_cell("name", "Second"),
    ]);
    let identity = state
        .source_row_identity_at("duplicate", 1)
        .expect("second duplicate should resolve in the source snapshot");

    assert_eq!(
        state.clone().source_row_identity_at("duplicate", 1),
        Some(identity.clone()),
        "cloning configuration must retain the same source snapshot"
    );

    let reordered = state.with_rows([
        TableRow::new("duplicate").with_cell("name", "Second"),
        TableRow::new("duplicate").with_cell("name", "First"),
    ]);
    let replacement = reordered
        .source_row_identity_at("duplicate", 1)
        .expect("replacement snapshot should resolve its own occurrence");

    assert_ne!(identity, replacement);
    assert_eq!(
        reordered.source_row_lookup(&replacement),
        TableSourceRowLookup::Found { source_index: 1 }
    );
    assert_eq!(
        reordered.source_row_lookup(&identity),
        TableSourceRowLookup::StaleSnapshot
    );
}

#[test]
fn source_identity_index_is_shared_until_the_source_snapshot_changes() {
    let state = TableState::new([TableRow::new("duplicate"), TableRow::new("duplicate")]);
    let cloned = state.clone();
    assert_eq!(
        state.cache_key().rows_identity(),
        state.source_identities.source_snapshot()
    );
    assert!(Arc::ptr_eq(
        &state.source_identities,
        &cloned.source_identities
    ));

    let rebuilt = state.with_rows([TableRow::new("duplicate"), TableRow::new("duplicate")]);
    assert_ne!(
        cloned.cache_key().rows_identity(),
        rebuilt.cache_key().rows_identity()
    );
    assert!(!Arc::ptr_eq(
        &cloned.source_identities,
        &rebuilt.source_identities
    ));
}

#[test]
fn explicit_duplicate_identity_survives_every_row_model_stage() {
    let target = TableRowIdentity::source_instance("duplicate", "second");
    let resolved = TableState::new([
        TableRow::new("duplicate")
            .with_instance_id("first")
            .with_cell("team", "ops")
            .with_cell("score", 10_usize),
        TableRow::new("duplicate")
            .with_instance_id("second")
            .with_cell("team", "ops")
            .with_cell("score", 20_usize),
        TableRow::new("other")
            .with_cell("team", "design")
            .with_cell("score", 30_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([TableFilter::contains("team", "ops")])
    .with_grouping(["team"])
    .with_sorting([TableSort::descending("score")])
    .with_all_rows_expanded()
    .with_pagination(TablePagination::disabled())
    .resolve();

    for model in [
        resolved.core_model(),
        resolved.filtered_model(),
        resolved.grouped_model(),
        resolved.sorted_model(),
        resolved.expanded_model(),
        resolved.paginated_model(),
        resolved.final_model(),
    ] {
        assert_eq!(
            model.row(&target).map(TableResolvedRow::identity),
            Some(&target),
            "{} stage must preserve the exact explicit identity",
            model.stage().as_str()
        );
    }
}

#[test]
fn row_model_index_excludes_lookup_only_rows() {
    let resolved = TableState::new([TableRow::new("visible"), TableRow::new("off-page")])
        .with_pagination(TablePagination::new(0, 1))
        .resolve();
    let visible = TableRowIdentity::source("visible");
    let off_page = TableRowIdentity::source("off-page");

    assert_eq!(resolved.final_model().row_index(&visible), Some(0));
    assert_eq!(resolved.final_model().row_index(&off_page), None);
    assert!(resolved.final_model().row(&off_page).is_some());
    assert!(std::ptr::eq(
        resolved
            .final_model()
            .row(&visible)
            .expect("materialized row should remain addressable"),
        &resolved.final_model().rows()[0]
    ));
    assert_eq!(
        resolved
            .final_model()
            .lookup_rows()
            .map(TableResolvedRow::debug_label)
            .collect::<Vec<_>>(),
        ["off-page", "visible"],
        "lookup iteration remains in identity order"
    );
}
