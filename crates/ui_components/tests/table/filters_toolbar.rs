use super::*;

#[test]
fn table_behavior_snapshot_exposes_faceting_metadata() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize),
        TableRow::new("row-4")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("status", "Ready"),
        TableFilter::contains("team", "UI"),
    ])
    .with_pagination(TablePagination::new(0, 1))
    .with_manual_facets(
        [TableColumnFacets::manual("status", 64).with_unique_values([
            TableFacetValueCount::new("Blocked", 24),
            TableFacetValueCount::new("Ready", 40),
        ])],
    );

    let snapshot = Table::new("faceted-table", "Faceted table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    assert_eq!(snapshot.faceting_mode(), TableStageMode::Client);
    assert_eq!(snapshot.column_facets().len(), 3);
    assert_eq!(
        snapshot
            .rows()
            .iter()
            .map(|row| row.source_row_id().expect("source row").as_str())
            .collect::<Vec<_>>(),
        ["row-1"],
        "pagination still limits the rendered row window"
    );

    let status = snapshot
        .column_facet(&TableColumnId::new("status"))
        .expect("status facet should resolve");
    assert_eq!(status.mode(), TableStageMode::Manual);
    assert_eq!(status.row_count(), 64);
    assert_eq!(
        text_facet_counts(status),
        [("Blocked".to_string(), 24), ("Ready".to_string(), 40)],
        "manual facet payloads should survive render-plan resolution"
    );

    let team = snapshot
        .column_facet(&TableColumnId::new("team"))
        .expect("team facet should resolve");
    assert_eq!(team.mode(), TableStageMode::Client);
    assert_eq!(team.row_count(), 3);
    assert_eq!(
        text_facet_counts(team),
        [("API".to_string(), 1), ("UI".to_string(), 2)],
        "client facets ignore their own column filter and honor the other filters"
    );

    let score = snapshot
        .column_facet(&TableColumnId::new("score"))
        .expect("score facet should resolve");
    let range = score
        .numeric_range()
        .expect("score facet should expose a numeric range");
    assert_eq!(range.min(), 10.0);
    assert_eq!(range.max(), 40.0);
}

#[test]
fn table_behavior_snapshot_exposes_global_facet_summary() {
    let state = TableState::new([
        TableRow::new("row-1")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "alpha")
            .with_cell("notes", "ready"),
        TableRow::new("row-2")
            .with_cell("team", "UI")
            .with_cell("status", "Blocked")
            .with_cell("score", 20_usize)
            .with_cell("enabled", false)
            .with_cell("notes", "done"),
        TableRow::new("row-3")
            .with_cell("team", "API")
            .with_cell("status", "Ready")
            .with_cell("score", 30_usize)
            .with_cell("enabled", true)
            .with_cell("tag", "beta")
            .with_cell("notes", "done"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
        TableColumn::new("enabled", "Enabled"),
        TableColumn::new("tag", "Tag"),
        TableColumn::new("notes", "Notes").with_global_filterable(false),
    ])
    .with_filters([TableFilter::contains("team", "UI")])
    .with_global_filter("done")
    .with_pagination(TablePagination::disabled());

    let snapshot = Table::new("global-facet-table", "Global facet table", state)
        .row_height(ui_px(24.0))
        .viewport_extent(ui_px(96.0))
        .behavior_snapshot(UiPx::ZERO, ui_px(96.0));

    let summary: &TableGlobalFacetSummary = snapshot.global_facet_summary();
    assert_eq!(summary.mode(), TableStageMode::Client);
    assert_eq!(summary.row_count(), 2);
    assert!(summary.column_facet(&TableColumnId::new("notes")).is_none());
    assert_eq!(
        summary
            .column_facets()
            .iter()
            .map(|facet| facet.column().as_str())
            .collect::<Vec<_>>(),
        ["team", "status", "score", "enabled", "tag"]
    );
    assert_eq!(
        text_facet_counts(
            summary
                .column_facet(&TableColumnId::new("status"))
                .expect("status global facet should resolve")
        ),
        [("Blocked".to_string(), 1), ("Ready".to_string(), 1)]
    );
}

#[test]
fn table_faceted_filter_state_resolves_query_selection_and_popover_contract() {
    let facets = TableColumnFacets::manual("status", 4).with_unique_values([
        TableFacetValueCount::new("Ready", 2),
        TableFacetValueCount::new("Blocked", 1),
        TableFacetValueCount::new("Review", 1),
    ]);

    let state: TableFacetedFilterState =
        TableFacetedFilter::new("status-filter", "Status", "status")
            .facets(facets)
            .selected_values(["Ready", "Blocked"])
            .query("rea")
            .open(true)
            .placeholder("Find status")
            .empty_label("No statuses")
            .clear_label("Reset")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "status-filter");
    assert_eq!(state.label(), "Status");
    assert_eq!(state.column_id().as_str(), "status");
    assert_eq!(state.query(), "rea");
    assert_eq!(state.trigger_label(), "Status: Ready, Blocked");
    assert_eq!(
        state.selected_values(),
        &["Blocked".to_string(), "Ready".to_string()]
    );
    assert_eq!(
        state.selected_labels(),
        &["Ready".to_string(), "Blocked".to_string()]
    );
    assert_eq!(state.total_option_count(), 3);
    assert!(state.clear_enabled());
    assert_eq!(state.empty_label(), "No statuses");
    assert_eq!(state.clear_label(), "Reset");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.search_input().value(), "rea");
    assert_eq!(state.search_input().placeholder(), Some("Find status"));
    assert_eq!(state.search_input().size(), Size::Small);
    assert!(state.search_input().controller_driven());

    let options = state
        .options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.count(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![("Ready".to_string(), "Ready".to_string(), 2, true)]
    );
}

#[test]
fn table_faceted_filter_state_reports_empty_query_result() {
    let state = TableFacetedFilter::new("status-filter", "Status", "status")
        .facets(TableColumnFacets::manual("status", 2).with_unique_values([
            TableFacetValueCount::new("Ready", 1),
            TableFacetValueCount::new("Blocked", 1),
        ]))
        .query("missing")
        .empty_label("No matching statuses")
        .state();

    assert!(state.empty());
    assert!(!state.clear_enabled());
    assert_eq!(state.total_option_count(), 2);
    assert_eq!(state.empty_label(), "No matching statuses");
    assert!(state.options().is_empty());
}

#[test]
fn table_faceted_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready"),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked"),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change =
        TableFacetedFilterChange::new("status", ["Blocked", "Ready"], Some("Blocked"), true);
    assert_eq!(change.column_id().as_str(), "status");
    assert_eq!(change.toggled_value(), Some("Blocked"));
    assert!(change.selected());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 2);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let status_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "status")
        .expect("status filter should be replaced by the faceted selection");
    assert_eq!(
        status_filter
            .selected_values()
            .expect("status filter should be categorical")
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["Blocked".to_string(), "Ready".to_string()]
    );

    let cleared = TableFacetedFilterChange::clear("status");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 1);
    assert_eq!(cleared_state.filters()[0].column().as_str(), "team");
}

#[test]
fn table_range_filter_state_resolves_bounds_and_popover_contract() {
    let facets = TableColumnFacets::manual("score", 64).with_numeric_range(1.0, 64.0);

    let state: TableRangeFilterState = TableRangeFilter::new("score-range", "Score", "score")
        .facets(facets)
        .range(Some(40.0), Some(10.0))
        .open(true)
        .clear_label("Reset score")
        .small()
        .placement_side(OverlayPlacementSide::Top)
        .placement_alignment(OverlayPlacementAlignment::End)
        .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
        .state();

    assert_eq!(state.id(), "score-range");
    assert_eq!(state.label(), "Score");
    assert_eq!(state.column_id().as_str(), "score");
    assert_eq!(state.min_text(), "10");
    assert_eq!(state.max_text(), "40");
    assert_eq!(state.min_value(), Some(10.0));
    assert_eq!(state.max_value(), Some(40.0));
    assert_eq!(state.trigger_label(), "Score: 10-40");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.clear_label(), "Reset score");
    let facet_range = state
        .facet_range()
        .expect("manual score facets should expose a numeric range");
    assert_eq!(facet_range.min(), 1.0);
    assert_eq!(facet_range.max(), 64.0);
    assert_eq!(state.min_placeholder(), "Min (1)");
    assert_eq!(state.max_placeholder(), "Max (64)");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(state.min_input().value(), "10");
    assert_eq!(state.max_input().value(), "40");
    assert_eq!(state.min_input().placeholder(), Some("Min (1)"));
    assert_eq!(state.max_input().placeholder(), Some("Max (64)"));
    assert!(state.min_input().controller_driven());
    assert!(state.max_input().controller_driven());
}

#[test]
fn table_range_filter_change_updates_filters_and_resets_pagination() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        TableFilter::number_range("score", Some(5.0), Some(20.0))
            .expect("initial score range should be valid"),
    ])
    .with_pagination(TablePagination::new(3, 25));

    let change = TableRangeFilterChange::new("score", "30", "10");
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.min_text(), "30");
    assert_eq!(change.max_text(), "10");
    assert_eq!(change.min_value(), Some(10.0));
    assert_eq!(change.max_value(), Some(30.0));
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state);
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters().len(), 3);
    let team_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "team")
        .expect("unrelated team filter should be preserved");
    assert_eq!(team_filter.query(), "UI");
    let score_text_filter = next
        .filters()
        .iter()
        .find(|filter| filter.column().as_str() == "score" && filter.query() == "1")
        .expect("same-column non-range filter should be preserved");
    assert_eq!(score_text_filter.number_range_bounds(), None);
    let score_filter = next
        .filters()
        .iter()
        .find(|filter| {
            filter.column().as_str() == "score" && filter.number_range_bounds().is_some()
        })
        .expect("score filter should be replaced by the range selection");
    assert_eq!(
        score_filter.number_range_bounds(),
        Some((Some(10.0), Some(30.0)))
    );

    let cleared = TableRangeFilterChange::clear("score");
    assert!(cleared.cleared());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 2);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_range_bounds().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team")
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.column().as_str() == "score" && filter.query() == "1")
    );
}

#[test]
fn table_column_visibility_state_resolves_items_counts_and_popover_contract() {
    let visibility = TableColumnVisibilityOverrides::new()
        .hide("name")
        .show("team")
        .hide("score");

    let state: TableColumnVisibilityState =
        TableColumnVisibility::new("column-visibility", "Columns")
            .columns([
                TableColumn::new("name", "Name").with_hideable(false),
                TableColumn::new("team", "Team").with_visible(false),
                TableColumn::new("score", "Score"),
            ])
            .visibility(visibility)
            .open(true)
            .empty_label("No columns configured")
            .show_all_label("Show every column")
            .reset_label("Reset columns")
            .small()
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::End)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .state();

    assert_eq!(state.id(), "column-visibility");
    assert_eq!(state.label(), "Columns");
    assert_eq!(state.trigger_label(), "Columns: 1 hidden");
    assert_eq!(state.item_count(), 3);
    assert_eq!(state.visible_count(), 2);
    assert_eq!(state.hidden_count(), 1);
    assert_eq!(state.hideable_count(), 2);
    assert!(!state.all_visible());
    assert!(state.some_visible());
    assert!(state.show_all_enabled());
    assert!(state.reset_enabled());
    assert_eq!(state.empty_label(), "No columns configured");
    assert_eq!(state.show_all_label(), "Show every column");
    assert_eq!(state.reset_label(), "Reset columns");
    assert_eq!(state.popover().open_mode(), PopoverOpenMode::Controlled);
    assert!(state.popover().open());
    assert_eq!(state.popover().placement_side(), OverlayPlacementSide::Top);
    assert_eq!(
        state.popover().placement_alignment(),
        OverlayPlacementAlignment::End
    );
    assert_eq!(
        state.popover().outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );

    let items = state
        .items()
        .iter()
        .map(|item| {
            (
                item.column_id().as_str().to_owned(),
                item.label().to_owned(),
                item.checked(),
                item.hideable(),
                item.disabled(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        items,
        vec![
            ("name".to_string(), "Name".to_string(), true, false, true),
            ("team".to_string(), "Team".to_string(), true, true, false),
            ("score".to_string(), "Score".to_string(), false, true, false),
        ]
    );
}

#[test]
fn table_column_visibility_change_updates_visibility_and_preserves_table_state() {
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("name", "Beta")
            .with_cell("team", "Platform")
            .with_cell("score", 40_usize),
    ])
    .with_columns([
        TableColumn::new("name", "Name").with_hideable(false),
        TableColumn::new("team", "Team").with_visible(false),
        TableColumn::new("score", "Score"),
    ])
    .with_column_visibility(
        TableColumnVisibilityOverrides::new()
            .hide("name")
            .show("team")
            .hide("score"),
    )
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::number_range("score", Some(5.0), Some(50.0))
            .expect("finite score range should be valid"),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]))
    .with_row_pinning(TableRowPinning::new().pinned_top([table_source_row_identity("row-a")]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_pagination(TablePagination::new(2, 25));

    let change = TableColumnVisibilityChange::new("team", false);
    assert_eq!(change.action(), TableColumnVisibilityAction::ToggleColumn);
    assert_eq!(change.action().as_str(), "toggle_column");
    assert_eq!(change.column_id().map(TableColumnId::as_str), Some("team"));
    assert_eq!(change.column_ids(), &[TableColumnId::new("team")]);
    assert_eq!(change.next_visible(), Some(false));

    let next = change.apply_to(state.clone());
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(false)
    );
    assert_eq!(
        next.column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.pagination(), state.pagination());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());

    let show_all = TableColumnVisibilityChange::show_all(["team", "score"]);
    assert_eq!(show_all.action(), TableColumnVisibilityAction::ShowAll);
    assert_eq!(show_all.action().as_str(), "show_all");
    assert_eq!(show_all.next_visible(), Some(true));
    assert_eq!(show_all.column_ids().len(), 2);
    let shown = show_all.apply_to(state.clone());
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("team")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("score")),
        Some(true)
    );
    assert_eq!(
        shown
            .column_visibility()
            .override_for(&TableColumnId::new("name")),
        Some(false)
    );

    let reset = TableColumnVisibilityChange::reset();
    assert_eq!(reset.action(), TableColumnVisibilityAction::Reset);
    assert_eq!(reset.action().as_str(), "reset");
    assert!(reset.column_ids().is_empty());
    assert_eq!(reset.column_id(), None);
    assert_eq!(reset.next_visible(), None);
    let reset_state = reset.apply_to(state);
    assert!(reset_state.column_visibility().is_empty());
}

#[test]
fn table_global_filter_state_resolves_input_contract() {
    let state: TableGlobalFilterState = TableGlobalFilter::new("global-filter", "Search rows")
        .default_query("stale")
        .query("  done  ")
        .placeholder("Search every row")
        .clear_label("Reset search")
        .small()
        .state();

    assert_eq!(state.id(), "global-filter");
    assert_eq!(state.label(), "Search rows");
    assert_eq!(state.query(), "  done  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Search every row");
    assert_eq!(state.clear_label(), "Reset search");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  done  ");
    assert_eq!(state.input().placeholder(), Some("Search every row"));
    assert_eq!(state.input().size(), Size::Small);
    assert!(state.input().controller_driven());

    let empty = TableGlobalFilter::new("empty-global-filter", "Search")
        .default_query("   ")
        .disabled(true)
        .state();
    assert!(!empty.active());
    assert!(empty.clear_enabled());
    assert!(empty.disabled());
    assert!(empty.input().disabled());
}

#[test]
fn table_toolbar_state_resolves_slot_counts_and_summary() {
    let tokens = custom_tokens();
    let state: TableToolbarState = TableToolbar::new("table-toolbar", "Filters")
        .small()
        .tokens(tokens)
        .control(div())
        .controls(vec![div(), div()])
        .secondary_control(div())
        .secondary_controls(vec![div(), div()])
        .summary("3 filtered / 8 total")
        .state();

    assert_eq!(state.id(), "table-toolbar");
    assert_eq!(state.label(), "Filters");
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.primary_control_count(), 3);
    assert_eq!(state.secondary_control_count(), 3);
    assert_eq!(state.control_count(), 6);
    assert!(state.has_controls());
    assert_eq!(state.summary(), Some("3 filtered / 8 total"));
    assert!(state.has_summary());
    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.tokens(), tokens);
    assert_eq!(state.foreground().token(), TEST_TEXT);
    assert_eq!(state.muted_foreground().token(), TEST_TEXT_MUTED);
    assert_eq!(state.colors().foreground().token(), TEST_TEXT);
    assert_eq!(state.colors().muted_foreground().token(), TEST_TEXT_MUTED);

    let empty = TableToolbar::new("empty-table-toolbar", "Filters").state();
    assert_eq!(empty.primary_control_count(), 0);
    assert_eq!(empty.secondary_control_count(), 0);
    assert_eq!(empty.control_count(), 0);
    assert!(!empty.has_controls());
    assert_eq!(empty.summary(), None);
    assert!(!empty.has_summary());
}

#[test]
fn table_global_filter_change_updates_state_and_resets_pagination() {
    let state = sample_table_state(4)
        .with_filters([TableFilter::contains("team", "UI")])
        .with_sorting([TableSort::ascending("name")])
        .with_selection_mode(TableSelectionMode::Multiple)
        .with_selected_rows(["row-0001"])
        .with_global_filter("old")
        .with_pagination(TablePagination::new(3, 25));

    let change = TableGlobalFilterChange::new("  done  ");
    assert_eq!(change.query(), "  done  ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.global_filter(), Some("done"));
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.filters(), state.filters());
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());

    let cleared = TableGlobalFilterChange::clear();
    assert_eq!(cleared.query(), "");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.global_filter(), None);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters(), state.filters());
    assert_eq!(cleared_state.sorting(), state.sorting());
    assert_eq!(cleared_state.selected_rows(), state.selected_rows());
}

#[test]
fn table_predicate_filter_state_resolves_operator_and_input_contract() {
    let starts_with = TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith);
    let state: TablePredicateFilterState =
        TablePredicateFilter::new("name-predicate", "Name", "name")
            .default_operator(TablePredicateFilterOperator::text(
                TableTextFilterOperator::Contains,
            ))
            .operator(starts_with)
            .default_value("stale")
            .value("  Al  ")
            .operators([
                TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith),
                TablePredicateFilterOperator::text(TableTextFilterOperator::EndsWith),
                TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThan),
            ])
            .placeholder("Filter name")
            .clear_label("Reset name")
            .small()
            .state();

    assert_eq!(state.id(), "name-predicate");
    assert_eq!(state.label(), "Name");
    assert_eq!(state.column_id().as_str(), "name");
    assert_eq!(state.operator(), starts_with);
    assert_eq!(
        state.operator().text_operator(),
        Some(TableTextFilterOperator::StartsWith)
    );
    assert_eq!(state.value(), "  Al  ");
    assert!(state.active());
    assert!(state.clear_enabled());
    assert_eq!(state.placeholder(), "Filter name");
    assert_eq!(state.clear_label(), "Reset name");
    assert_eq!(state.size(), Size::Small);
    assert!(!state.disabled());
    assert_eq!(state.input().value(), "  Al  ");
    assert_eq!(state.input().placeholder(), Some("Filter name"));
    assert!(state.input().controller_driven());
    assert_eq!(state.select().selected_value(), Some("text:starts_with"));
    assert_eq!(state.select().trigger_label(), "Starts with");
    let first_option: &TablePredicateFilterOperatorOptionState = state
        .operator_options()
        .first()
        .expect("predicate filter should expose operator options");
    assert_eq!(first_option.operator(), starts_with);

    let options = state
        .operator_options()
        .iter()
        .map(|option| {
            (
                option.value().to_owned(),
                option.label().to_owned(),
                option.selected(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        options,
        vec![
            (
                "text:starts_with".to_string(),
                "Starts with".to_string(),
                true,
            ),
            ("text:ends_with".to_string(), "Ends with".to_string(), false),
            (
                "number:greater_than".to_string(),
                "Greater than".to_string(),
                false,
            ),
        ]
    );

    let invalid_number = TablePredicateFilter::new("score-predicate", "Score", "score")
        .operator(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::GreaterThan,
        ))
        .value("not a number")
        .state();
    assert!(!invalid_number.active());
    assert!(invalid_number.clear_enabled());
    assert_eq!(
        TablePredicateFilterOperator::from_str("number:less_than")
            .expect("stable numeric operator should parse")
            .numeric_operator(),
        Some(TableNumericFilterOperator::LessThan)
    );
}

#[test]
fn table_predicate_filter_change_updates_only_target_predicate_filters() {
    let score_range = TableFilter::number_range("score", Some(0.0), Some(100.0))
        .expect("finite score range should be valid");
    let score_comparison = TableFilter::number_greater_than("score", 5.0)
        .expect("finite score comparison should be valid");
    let state = TableState::new([
        TableRow::new("row-a")
            .with_cell("team", "UI")
            .with_cell("status", "Ready")
            .with_cell("score", 10_usize),
        TableRow::new("row-b")
            .with_cell("team", "Platform")
            .with_cell("status", "Blocked")
            .with_cell("score", 50_usize),
    ])
    .with_columns([
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ])
    .with_filters([
        TableFilter::contains("team", "UI"),
        TableFilter::contains("score", "1"),
        score_comparison,
        score_range.clone(),
        TableFilter::one_of("score", ["10", "50"]),
        TableFilter::one_of("status", ["Ready"]),
    ])
    .with_sorting([TableSort::descending("score")])
    .with_selection_mode(TableSelectionMode::Multiple)
    .with_selected_rows(["row-a"])
    .with_column_pinning(TableColumnPinning::new().pinned_left(["team"]))
    .with_row_pinning(TableRowPinning::new().pinned_top([table_source_row_identity("row-a")]))
    .with_column_sizing(TableColumnSizing::new().with_width("score", ui_px(180.0)))
    .with_column_visibility(TableColumnVisibilityOverrides::new().hide("status"))
    .with_global_filter("ready")
    .with_pagination(TablePagination::new(3, 25));

    let change = TablePredicateFilterChange::new(
        "score",
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThanOrEqual),
        " 42 ",
    );
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(
        change.operator(),
        Some(TablePredicateFilterOperator::number(
            TableNumericFilterOperator::LessThanOrEqual
        ))
    );
    assert_eq!(change.value(), " 42 ");
    assert!(change.active());
    assert!(!change.cleared());

    let next = change.apply_to(state.clone());
    assert_eq!(next.pagination().page_index(), 0);
    assert_eq!(next.pagination().page_size(), 25);
    assert_eq!(next.sorting(), state.sorting());
    assert_eq!(next.selected_rows(), state.selected_rows());
    assert_eq!(next.column_pinning(), state.column_pinning());
    assert_eq!(next.row_pinning(), state.row_pinning());
    assert_eq!(next.column_sizing(), state.column_sizing());
    assert_eq!(next.column_visibility(), state.column_visibility());
    assert_eq!(next.global_filter(), state.global_filter());
    assert_eq!(next.filters().len(), 5);
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.column().as_str() == "team" && filter.query() == "UI")
    );
    assert!(
        next.filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(next.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
    let score_predicate = next
        .filters()
        .iter()
        .find(|filter| filter.number_comparison_value().is_some())
        .expect("score numeric comparison should be replaced");
    assert_eq!(score_predicate.column().as_str(), "score");
    assert_eq!(
        score_predicate.number_comparison_value(),
        Some((TableNumericFilterOperator::LessThanOrEqual, 42.0))
    );
    assert!(
        next.filters().iter().all(|filter| {
            filter.column().as_str() != "score" || filter.text_predicate().is_none()
        }),
        "same-column legacy/text predicate should be removed"
    );

    let cleared = TablePredicateFilterChange::clear("score");
    assert!(cleared.cleared());
    assert!(!cleared.active());
    let cleared_state = cleared.apply_to(next);
    assert_eq!(cleared_state.pagination().page_index(), 0);
    assert_eq!(cleared_state.filters().len(), 4);
    assert!(
        cleared_state
            .filters()
            .iter()
            .all(|filter| filter.number_comparison_value().is_none())
    );
    assert!(
        cleared_state
            .filters()
            .iter()
            .any(|filter| filter.number_range_bounds() == score_range.number_range_bounds())
    );
    assert!(cleared_state.filters().iter().any(|filter| {
        filter.column().as_str() == "score"
            && filter
                .selected_values()
                .is_some_and(|values| values.contains("10") && values.contains("50"))
    }));
}
