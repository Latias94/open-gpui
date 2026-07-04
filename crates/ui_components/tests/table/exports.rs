use super::*;

#[test]
fn table_public_exports_use_explicit_core_table_and_virtualizer_contracts() {
    use open_gpui_ui_components::{self as root, prelude};
    use open_gpui_ui_core as ui_core;

    let state: ui_core::TableState =
        ui_core::TableState::new([ui_core::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_columns([ui_core::TableColumn::new("name", "Name")]);
    let table: root::Table = root::Table::new("root-table", "Root table", state.clone());
    let _prelude_state: ui_core::TableState = state;
    let _prelude_table: prelude::Table = prelude::Table::new(
        "prelude-table",
        "Prelude table",
        ui_core::TableState::new([ui_core::TableRow::new("row-b").with_cell("name", "Beta")])
            .with_columns([ui_core::TableColumn::new("name", "Name")]),
    );
    let virtualizer: ui_core::VirtualizerState =
        ui_core::VirtualizerState::new(4, ui_px(24.0)).with_overscan(2);
    let root_state_readout: &ui_core::TableState = table.state();
    let root_resolved_state = root_state_readout.resolve();
    assert_eq!(root_resolved_state.final_model().rows().len(), 1);
    let root_snapshot: root::TableBehaviorSnapshot =
        table.behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let _prelude_snapshot: prelude::TableBehaviorSnapshot = root_snapshot.clone();
    let _root_region_snapshot: root::TableColumnRegionSnapshot = root_snapshot.column_regions();
    let _root_header_groups: &ui_core::TableResolvedHeaderGroupRegions =
        root_resolved_state.header_groups();
    let _root_header_kind: ui_core::TableResolvedHeaderKind =
        root_resolved_state.center_header_groups()[0].headers()[0].kind();
    let _root_header_cell: &ui_core::TableResolvedHeaderCell =
        &root_resolved_state.center_header_groups()[0].headers()[0];
    let _root_header_group: &ui_core::TableResolvedHeaderGroup =
        &root_resolved_state.center_header_groups()[0];
    let _root_header_summary: root::TableHeaderSummarySnapshot = root_snapshot.header_summary();
    let root_group_id = ui_core::TableColumnGroupId::new("identity");
    assert_eq!(root_group_id.as_str(), "identity");
    let root_column_group = ui_core::TableColumnGroup::new(
        root_group_id.clone(),
        "Identity",
        [ui_core::TableColumn::new("name", "Name")],
    )
    .with_child(ui_core::TableColumn::new("team", "Team"));
    let root_column_tree_state =
        ui_core::TableState::new([ui_core::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_column_tree([root_column_group.clone()]);
    let _root_column_node: &ui_core::TableColumnNode = &root_column_tree_state.column_tree()[0];
    let _root_column_group: ui_core::TableColumnGroup = root_column_group;
    let prelude_group = ui_core::TableColumnGroup::new(
        ui_core::TableColumnGroupId::new("status-group"),
        "Status",
        [ui_core::TableColumn::new("status", "Status")],
    );
    let prelude_state =
        ui_core::TableState::new([ui_core::TableRow::new("row-c").with_cell("status", "Ready")])
            .with_column_tree([ui_core::TableColumnNode::from(prelude_group)]);
    assert_eq!(prelude_state.columns()[0].id().as_str(), "status");
    let root_pinned_state = ui_core::TableState::new([ui_core::TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("status", "Ready")])
    .with_columns([
        ui_core::TableColumn::new("name", "Name"),
        ui_core::TableColumn::new("team", "Team"),
        ui_core::TableColumn::new("status", "Status"),
    ])
    .with_column_pinning(
        ui_core::TableColumnPinning::new()
            .pinned_left(["name"])
            .pinned_right(["status"]),
    );
    let root_pinned_snapshot =
        root::Table::new("root-pinned-table", "Root pinned table", root_pinned_state)
            .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let root_pinned_regions: root::TableColumnRegionSnapshot =
        root_pinned_snapshot.column_regions();
    let _prelude_pinned_regions: prelude::TableColumnRegionSnapshot = root_pinned_regions;
    let _prelude_header_summary: prelude::TableHeaderSummarySnapshot = _root_header_summary;
    assert_eq!(root_pinned_snapshot.table_id(), "root-pinned-table");
    let root_row_pinning: ui_core::TableRowPinning = ui_core::TableRowPinning::new()
        .pinned_top(["row-a"])
        .pinned_bottom(["row-b"]);
    let _prelude_row_pinning: ui_core::TableRowPinning = root_row_pinning.clone();
    let _root_row_measure_mode: root::TableRowMeasureMode = root::TableRowMeasureMode::Measured;
    let _prelude_row_measure_mode: prelude::TableRowMeasureMode =
        prelude::TableRowMeasureMode::Fixed;
    let _root_row_pinning_policy: ui_core::TableRowPinningPolicy =
        ui_core::TableRowPinningPolicy::PageOnly;
    let _prelude_row_pinning_policy: ui_core::TableRowPinningPolicy =
        ui_core::TableRowPinningPolicy::KeepPinnedRows;
    let _root_row_region: ui_core::TableRowRegion = ui_core::TableRowRegion::Top;
    let _prelude_row_region: ui_core::TableRowRegion = ui_core::TableRowRegion::Bottom;
    let root_row_counts: root::TableRowCountSnapshot = root::Table::new(
        "root-row-pinning-table",
        "Root row pinning table",
        ui_core::TableState::new([
            ui_core::TableRow::new("row-a").with_cell("name", "Alpha"),
            ui_core::TableRow::new("row-b").with_cell("name", "Beta"),
        ])
        .with_columns([ui_core::TableColumn::new("name", "Name")])
        .with_row_pinning(root_row_pinning.clone()),
    )
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0))
    .row_counts();
    let _prelude_row_counts: prelude::TableRowCountSnapshot = root_row_counts;
    assert_eq!(root_pinned_regions.center_columns(), 1);
    let root_grid_viewport: ui_core::GridViewport2D = ui_core::resolve_grid_viewport_2d(
        &ui_core::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
        &ui_core::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
    );
    let _prelude_grid_viewport: ui_core::GridViewport2D = root_grid_viewport.clone();
    let _prelude_grid_viewport_via_prelude: ui_core::GridViewport2D =
        ui_core::resolve_grid_viewport_2d(
            &ui_core::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
            &ui_core::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
        );
    assert_eq!(root_grid_viewport.row_overscan_range().start(), 0);
    let header_action: root::TableHeaderAction = root_snapshot.columns()[0]
        .sort_action()
        .expect("sortable exported table column should expose a header action")
        .clone();
    let _root_cache_key: ui_core::TableStateCacheKey = table.state().cache_key();
    let _prelude_header_action: prelude::TableHeaderAction = header_action;
    let _prelude_cache_key: ui_core::TableStateCacheKey = table.state().cache_key();
    let _root_aggregation: ui_core::TableAggregation =
        ui_core::TableAggregation::new("score", ui_core::TableAggregateKind::Sum);
    let _prelude_aggregation: ui_core::TableAggregation =
        ui_core::TableAggregation::average("score");
    let _root_expansion: ui_core::TableExpansionState = ui_core::TableExpansionState::all();
    let _prelude_expansion: ui_core::TableExpansionState =
        ui_core::TableExpansionState::rows([ui_core::TableRowId::new("group:team=ui")]);
    let _root_expansion_mode: ui_core::TableExpansionMode = ui_core::TableExpansionMode::Manual;
    let _prelude_expansion_mode: ui_core::TableExpansionMode = ui_core::TableExpansionMode::Client;
    let _root_stage_mode: ui_core::TableStageMode = ui_core::TableStageMode::Manual;
    let _prelude_stage_mode: ui_core::TableStageMode = ui_core::TableStageMode::Client;
    let root_filter = ui_core::TableFilter::one_of("status", ["Ready", "Blocked"]);
    let _prelude_filter: ui_core::TableFilter = ui_core::TableFilter::contains("team", "UI");
    let _root_filter_kind: ui_core::TableFilterKind = root_filter.kind().clone();
    let _prelude_filter_kind: ui_core::TableFilterKind =
        ui_core::TableFilterKind::Contains { query: "UI".into() };
    let _root_text_filter_operator: ui_core::TableTextFilterOperator =
        ui_core::TableTextFilterOperator::StartsWith;
    let _prelude_text_filter_operator: ui_core::TableTextFilterOperator =
        ui_core::TableTextFilterOperator::NotContains;
    let _root_numeric_bound: ui_core::TableNumericFilterBound =
        ui_core::TableNumericFilterBound::new(10.0)
            .expect("finite numeric bounds should be constructible");
    let _prelude_numeric_bound: ui_core::TableNumericFilterBound =
        ui_core::TableNumericFilterBound::new(20.0)
            .expect("finite numeric bounds should be constructible");
    let _root_numeric_filter_operator: ui_core::TableNumericFilterOperator =
        ui_core::TableNumericFilterOperator::GreaterThanOrEqual;
    let _prelude_numeric_filter_operator: ui_core::TableNumericFilterOperator =
        ui_core::TableNumericFilterOperator::LessThan;
    let root_range_filter = ui_core::TableFilter::number_range("score", Some(10.0), Some(20.0))
        .expect("exported numeric range filter should construct");
    assert_eq!(
        root_range_filter.number_range_bounds(),
        Some((Some(10.0), Some(20.0)))
    );
    let root_facet_value = ui_core::TableFacetValueCount::new("Ready", 2);
    let root_facets: ui_core::TableColumnFacets =
        ui_core::TableColumnFacets::manual("status", 2).with_unique_values([root_facet_value]);
    let _prelude_facets: ui_core::TableColumnFacets = root_facets.clone();
    let root_global_facets: ui_core::TableGlobalFacetSummary =
        ui_core::TableGlobalFacetSummary::default();
    let _prelude_global_facets: ui_core::TableGlobalFacetSummary = root_global_facets.clone();
    let root_global_filter: root::TableGlobalFilter =
        root::TableGlobalFilter::new("root-global-filter", "Search").query("ready");
    let _root_global_filter_state: root::TableGlobalFilterState = root_global_filter.state();
    let _root_global_filter_change: root::TableGlobalFilterChange =
        root::TableGlobalFilterChange::new("ready");
    let root_predicate_operator: root::TablePredicateFilterOperator =
        root::TablePredicateFilterOperator::text(ui_core::TableTextFilterOperator::StartsWith);
    let root_predicate_filter: root::TablePredicateFilter =
        root::TablePredicateFilter::new("root-name-predicate", "Name", "name")
            .operator(root_predicate_operator)
            .value("Al");
    let root_predicate_filter_state: root::TablePredicateFilterState =
        root_predicate_filter.state();
    let _root_predicate_option: Option<&root::TablePredicateFilterOperatorOptionState> =
        root_predicate_filter_state.operator_options().first();
    let _root_predicate_change: root::TablePredicateFilterChange =
        root::TablePredicateFilterChange::new("name", root_predicate_operator, "Al");
    let root_table_toolbar: root::TableToolbar =
        root::TableToolbar::new("root-table-toolbar", "Filters").summary("2 visible controls");
    let root_table_toolbar_state: root::TableToolbarState = root_table_toolbar.state();
    let _root_table_toolbar_colors: root::TableToolbarColors = root_table_toolbar_state.colors();
    let prelude_global_filter: prelude::TableGlobalFilter =
        prelude::TableGlobalFilter::new("prelude-global-filter", "Search").default_query("ready");
    let _prelude_global_filter_state: prelude::TableGlobalFilterState =
        prelude_global_filter.state();
    let _prelude_global_filter_change: prelude::TableGlobalFilterChange =
        prelude::TableGlobalFilterChange::clear();
    let prelude_predicate_operator: prelude::TablePredicateFilterOperator =
        prelude::TablePredicateFilterOperator::number(
            ui_core::TableNumericFilterOperator::GreaterThan,
        );
    let prelude_predicate_filter: prelude::TablePredicateFilter =
        prelude::TablePredicateFilter::new("prelude-score-predicate", "Score", "score")
            .operator(prelude_predicate_operator)
            .default_value("10");
    let prelude_predicate_filter_state: prelude::TablePredicateFilterState =
        prelude_predicate_filter.state();
    let _prelude_predicate_option: Option<&prelude::TablePredicateFilterOperatorOptionState> =
        prelude_predicate_filter_state.operator_options().first();
    let _prelude_predicate_change: prelude::TablePredicateFilterChange =
        prelude::TablePredicateFilterChange::clear("score");
    let prelude_table_toolbar: prelude::TableToolbar =
        prelude::TableToolbar::new("prelude-table-toolbar", "Filters")
            .summary("2 visible controls");
    let prelude_table_toolbar_state: prelude::TableToolbarState = prelude_table_toolbar.state();
    let _prelude_table_toolbar_colors: prelude::TableToolbarColors =
        prelude_table_toolbar_state.colors();
    let root_faceted_filter: root::TableFacetedFilter =
        root::TableFacetedFilter::new("root-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let root_faceted_filter_state: root::TableFacetedFilterState = root_faceted_filter.state();
    let _root_faceted_option: Option<&root::TableFacetedFilterOptionState> =
        root_faceted_filter_state.options().first();
    let _root_faceted_change: root::TableFacetedFilterChange =
        root::TableFacetedFilterChange::new("status", ["Ready"], Some("Ready"), true);
    let prelude_faceted_filter: prelude::TableFacetedFilter =
        prelude::TableFacetedFilter::new("prelude-status-filter", "Status", "status")
            .facets(root_facets.clone())
            .selected_values(["Ready"]);
    let prelude_faceted_filter_state: prelude::TableFacetedFilterState =
        prelude_faceted_filter.state();
    let _prelude_faceted_option: Option<&prelude::TableFacetedFilterOptionState> =
        prelude_faceted_filter_state.options().first();
    let _prelude_faceted_change: prelude::TableFacetedFilterChange =
        prelude::TableFacetedFilterChange::clear("status");
    let root_column_visibility: root::TableColumnVisibility =
        root::TableColumnVisibility::new("root-columns", "Columns")
            .columns([
                ui_core::TableColumn::new("name", "Name").with_hideable(false),
                ui_core::TableColumn::new("status", "Status"),
            ])
            .visibility(ui_core::TableColumnVisibilityOverrides::new().hide("status"));
    let root_column_visibility_state: root::TableColumnVisibilityState =
        root_column_visibility.state();
    let _root_column_visibility_item: Option<&root::TableColumnVisibilityItemState> =
        root_column_visibility_state.items().first();
    let root_column_visibility_change: root::TableColumnVisibilityChange =
        root::TableColumnVisibilityChange::new("status", false);
    let _root_column_visibility_action: root::TableColumnVisibilityAction =
        root_column_visibility_change.action();
    let root_column_order_change: root::TableColumnOrderChange =
        root::TableColumnOrderChange::move_before(
            "score",
            "team",
            ui_core::TableColumnRegion::Center,
        );
    let _root_column_order_placement: root::TableColumnOrderPlacement =
        root_column_order_change.placement();
    let prelude_column_visibility: prelude::TableColumnVisibility =
        prelude::TableColumnVisibility::new("prelude-columns", "Columns")
            .columns([ui_core::TableColumn::new("status", "Status")])
            .default_visibility(ui_core::TableColumnVisibilityOverrides::new().hide("status"));
    let prelude_column_visibility_state: prelude::TableColumnVisibilityState =
        prelude_column_visibility.state();
    let _prelude_column_visibility_item: Option<&prelude::TableColumnVisibilityItemState> =
        prelude_column_visibility_state.items().first();
    let prelude_column_visibility_change: prelude::TableColumnVisibilityChange =
        prelude::TableColumnVisibilityChange::reset();
    let _prelude_column_visibility_action: prelude::TableColumnVisibilityAction =
        prelude_column_visibility_change.action();
    let _prelude_column_order_change: prelude::TableColumnOrderChange = root_column_order_change;
    let _prelude_column_order_placement: prelude::TableColumnOrderPlacement =
        prelude::TableColumnOrderPlacement::After;
    let _root_facet_range: Option<ui_core::TableFacetRange> =
        ui_core::TableFacetRange::new(1.0, 2.0);
    let root_range_facets =
        ui_core::TableColumnFacets::manual("score", 2).with_numeric_range(1.0, 20.0);
    let root_range_filter: root::TableRangeFilter =
        root::TableRangeFilter::new("root-score-range", "Score", "score")
            .facets(root_range_facets.clone())
            .range(Some(1.0), Some(20.0));
    let _root_range_filter_state: root::TableRangeFilterState = root_range_filter.state();
    let _root_range_change: root::TableRangeFilterChange =
        root::TableRangeFilterChange::new("score", "1", "20");
    let prelude_range_filter: prelude::TableRangeFilter =
        prelude::TableRangeFilter::new("prelude-score-range", "Score", "score")
            .facets(root_range_facets)
            .range(Some(1.0), Some(20.0));
    let _prelude_range_filter_state: prelude::TableRangeFilterState = prelude_range_filter.state();
    let _prelude_range_change: prelude::TableRangeFilterChange =
        prelude::TableRangeFilterChange::clear("score");
    let _prelude_facet_value: ui_core::TableFacetValueCount =
        ui_core::TableFacetValueCount::new("Blocked", 1);
    let _root_child_load_state: ui_core::TableRowChildrenLoadState =
        ui_core::TableRowChildrenLoadState::loading("Loading children");
    let _prelude_child_load_state: ui_core::TableRowChildrenLoadState =
        ui_core::TableRowChildrenLoadState::failed("Load failed");
    let _prelude_row_kind: ui_core::TableResolvedRowKind = ui_core::TableResolvedRowKind::Leaf;
    let root_tree_state = ui_core::TableState::new([ui_core::TableRow::new("root")
        .with_cell("name", "Root")
        .with_child(ui_core::TableRow::new("child").with_cell("name", "Child"))])
    .with_columns([ui_core::TableColumn::new("name", "Name")])
    .with_all_rows_expanded();
    let root_tree_row: ui_core::TableTreeRow = root_tree_state.resolve().final_model().rows()[0]
        .tree()
        .expect("tree source row should expose hierarchy metadata")
        .clone();
    let _prelude_tree_row: ui_core::TableTreeRow = root_tree_row;
    let _resolved_kind: Option<&ui_core::TableGroupRow> =
        table.state().resolve().final_model().rows()[0].group();
    let _root_table_modifiers: root::TableInputModifiers = root::TableInputModifiers::default();
    let _prelude_table_modifiers: prelude::TableInputModifiers =
        prelude::TableInputModifiers::default();
    let _root_row_action: Option<root::TableRowAction> = None;
    let _prelude_row_action: Option<prelude::TableRowAction> = None;
    let _root_row_activation: Option<root::TableRowActivation> = None;
    let _prelude_row_activation: Option<prelude::TableRowActivation> = None;
    let _root_row_expansion: Option<root::TableRowExpansionToggle> = None;
    let _prelude_row_expansion: Option<prelude::TableRowExpansionToggle> = None;
    let _root_activation_kind: root::TableRowActivationKind =
        root::TableRowActivationKind::DoubleClick;
    let _prelude_activation_kind: prelude::TableRowActivationKind =
        prelude::TableRowActivationKind::Keyboard;
    let _root_pinning: ui_core::TableColumnPinning =
        ui_core::TableColumnPinning::new().pinned_left(["name"]);
    let _root_width_policy: ui_core::TableColumnWidthPolicy =
        ui_core::TableColumnWidthPolicy::ContentFit;
    let _prelude_width_policy: ui_core::TableColumnWidthPolicy =
        ui_core::TableColumnWidthPolicy::Fixed;
    let content_fit_column = ui_core::TableColumn::new("status", "Status").with_content_fit();
    assert!(content_fit_column.is_content_fit());
    assert_eq!(
        content_fit_column.width_policy(),
        ui_core::TableColumnWidthPolicy::ContentFit
    );
    let root_visibility = ui_core::TableColumnVisibilityOverrides::new()
        .hide("score")
        .show("status")
        .without("missing");
    let _root_visibility: ui_core::TableColumnVisibilityOverrides = root_visibility.clone();
    let _prelude_visibility: ui_core::TableColumnVisibilityOverrides =
        ui_core::TableColumnVisibilityOverrides::new().show("status");
    assert_eq!(
        root_visibility.override_for(&ui_core::TableColumnId::new("score")),
        Some(false)
    );
    let root_sizing = ui_core::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let _root_sizing: ui_core::TableColumnSizing = root_sizing.clone();
    let _prelude_sizing: ui_core::TableColumnSizing =
        ui_core::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let root_resize_state = ui_core::TableColumnResizeState::begin(
        "name",
        ui_px(12.0),
        ui_px(180.0),
        [("name", ui_px(180.0))],
    );
    let root_resize_update: ui_core::TableColumnResizeUpdate = ui_core::drag_table_column_resize(
        ui_core::TableColumnResizeMode::OnChange,
        ui_core::TableColumnResizeDirection::Ltr,
        &root_sizing,
        &root_resize_state,
        ui_px(24.0),
    );
    let _prelude_resize_state: ui_core::TableColumnResizeState = root_resize_update.state().clone();
    let _prelude_resize_update: ui_core::TableColumnResizeUpdate = ui_core::end_table_column_resize(
        ui_core::TableColumnResizeMode::OnEnd,
        ui_core::TableColumnResizeDirection::Ltr,
        &ui_core::TableColumnSizing::new().with_width("name", ui_px(180.0)),
        &root_resize_state,
        Some(ui_px(24.0)),
    );
    let root_resize_change = root::TableColumnSizingChange::new(
        "name",
        ui_px(204.0),
        root_resize_update
            .committed_sizing()
            .cloned()
            .expect("resize update should commit in on-change mode"),
    );
    let _prelude_resize_change: prelude::TableColumnSizingChange = root_resize_change;
    let _root_resolved_sizing: ui_core::TableResolvedColumnSizing = table
        .state()
        .resolve()
        .visible_column_sizing()
        .column(&ui_core::TableColumnId::new("name"))
        .expect("resolved column sizing should be available")
        .clone();
    let _prelude_resolved_sizing: ui_core::TableResolvedColumnSizing =
        _root_resolved_sizing.clone();
    let _root_resolved_sizing_regions: ui_core::TableResolvedColumnSizingRegions =
        table.state().resolve().visible_column_sizing().clone();
    let _prelude_resolved_sizing_regions: ui_core::TableResolvedColumnSizingRegions =
        _root_resolved_sizing_regions.clone();
    let _root_default_width = ui_core::TABLE_DEFAULT_COLUMN_WIDTH;
    let _root_min_width = ui_core::TABLE_MIN_COLUMN_WIDTH;
    let _root_max_width = ui_core::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_default_width = ui_core::TABLE_DEFAULT_COLUMN_WIDTH;
    let _prelude_min_width = ui_core::TABLE_MIN_COLUMN_WIDTH;
    let _prelude_max_width = ui_core::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_region: ui_core::TableColumnRegion = ui_core::TableColumnRegion::Center;
    let _prelude_regions: ui_core::TableColumnRegions =
        table.state().resolve().visible_column_regions().clone();

    assert_eq!(root_snapshot.role(), Role::Table);
    assert!(!root_snapshot.columns().is_empty());
    assert_eq!(
        root::TableRowActivationKind::DoubleClick.as_str(),
        "double-click"
    );
    assert_eq!(virtualizer.resolve().overscan(), 2);
}
