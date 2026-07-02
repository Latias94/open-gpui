use super::*;

#[test]
fn table_public_exports_include_core_table_and_virtualizer_contracts() {
    use open_gpui_ui_components::{self as root, prelude};

    let state: root::TableState =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_columns([root::TableColumn::new("name", "Name")]);
    let table: root::Table = root::Table::new("root-table", "Root table", state.clone());
    let _prelude_state: prelude::TableState = state;
    let _prelude_table: prelude::Table = prelude::Table::new(
        "prelude-table",
        "Prelude table",
        root::TableState::new([root::TableRow::new("row-b").with_cell("name", "Beta")])
            .with_columns([root::TableColumn::new("name", "Name")]),
    );
    let virtualizer: root::VirtualizerState =
        root::VirtualizerState::new(4, ui_px(24.0)).with_overscan(2);
    let root_state_readout: &root::TableState = table.state();
    let root_resolved_state = root_state_readout.resolve();
    assert_eq!(root_resolved_state.final_model().rows().len(), 1);
    let root_snapshot: root::TableBehaviorSnapshot =
        table.behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let _prelude_snapshot: prelude::TableBehaviorSnapshot = root_snapshot.clone();
    let _root_region_snapshot: root::TableColumnRegionSnapshot = root_snapshot.column_regions();
    let _root_header_groups: &root::TableResolvedHeaderGroupRegions =
        root_resolved_state.header_groups();
    let _root_header_kind: root::TableResolvedHeaderKind =
        root_resolved_state.center_header_groups()[0].headers()[0].kind();
    let _root_header_cell: &root::TableResolvedHeaderCell =
        &root_resolved_state.center_header_groups()[0].headers()[0];
    let _root_header_group: &root::TableResolvedHeaderGroup =
        &root_resolved_state.center_header_groups()[0];
    let _root_header_summary: root::TableHeaderSummarySnapshot = root_snapshot.header_summary();
    let root_group_id = root::TableColumnGroupId::new("identity");
    assert_eq!(root_group_id.as_str(), "identity");
    let root_column_group = root::TableColumnGroup::new(
        root_group_id.clone(),
        "Identity",
        [root::TableColumn::new("name", "Name")],
    )
    .with_child(root::TableColumn::new("team", "Team"));
    let root_column_tree_state =
        root::TableState::new([root::TableRow::new("row-a").with_cell("name", "Alpha")])
            .with_column_tree([root_column_group.clone()]);
    let _root_column_node: &root::TableColumnNode = &root_column_tree_state.column_tree()[0];
    let _root_column_group: root::TableColumnGroup = root_column_group;
    let prelude_group = prelude::TableColumnGroup::new(
        prelude::TableColumnGroupId::new("status-group"),
        "Status",
        [prelude::TableColumn::new("status", "Status")],
    );
    let prelude_state =
        prelude::TableState::new([prelude::TableRow::new("row-c").with_cell("status", "Ready")])
            .with_column_tree([prelude::TableColumnNode::from(prelude_group)]);
    assert_eq!(prelude_state.columns()[0].id().as_str(), "status");
    let root_pinned_state = root::TableState::new([root::TableRow::new("row-a")
        .with_cell("name", "Alpha")
        .with_cell("team", "UI")
        .with_cell("status", "Ready")])
    .with_columns([
        root::TableColumn::new("name", "Name"),
        root::TableColumn::new("team", "Team"),
        root::TableColumn::new("status", "Status"),
    ])
    .with_column_pinning(
        root::TableColumnPinning::new()
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
    let root_row_pinning: root::TableRowPinning = root::TableRowPinning::new()
        .pinned_top(["row-a"])
        .pinned_bottom(["row-b"]);
    let _prelude_row_pinning: prelude::TableRowPinning = root_row_pinning.clone();
    let _root_row_measure_mode: root::TableRowMeasureMode = root::TableRowMeasureMode::Measured;
    let _prelude_row_measure_mode: prelude::TableRowMeasureMode =
        prelude::TableRowMeasureMode::Fixed;
    let _root_row_pinning_policy: root::TableRowPinningPolicy =
        root::TableRowPinningPolicy::PageOnly;
    let _prelude_row_pinning_policy: prelude::TableRowPinningPolicy =
        prelude::TableRowPinningPolicy::KeepPinnedRows;
    let _root_row_region: root::TableRowRegion = root::TableRowRegion::Top;
    let _prelude_row_region: prelude::TableRowRegion = prelude::TableRowRegion::Bottom;
    let root_row_counts: root::TableRowCountSnapshot = root::Table::new(
        "root-row-pinning-table",
        "Root row pinning table",
        root::TableState::new([
            root::TableRow::new("row-a").with_cell("name", "Alpha"),
            root::TableRow::new("row-b").with_cell("name", "Beta"),
        ])
        .with_columns([root::TableColumn::new("name", "Name")])
        .with_row_pinning(root_row_pinning.clone()),
    )
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0))
    .row_counts();
    let _prelude_row_counts: prelude::TableRowCountSnapshot = root_row_counts;
    assert_eq!(root_pinned_regions.center_columns(), 1);
    let root_grid_viewport: root::GridViewport2D = root::resolve_grid_viewport_2d(
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
        &root::VirtualizerState::new(2, ui_px(24.0))
            .with_viewport_extent(ui_px(24.0))
            .with_scroll_offset(ui_px(12.0)),
    );
    let _prelude_grid_viewport: prelude::GridViewport2D = root_grid_viewport.clone();
    let _prelude_grid_viewport_via_prelude: prelude::GridViewport2D =
        prelude::resolve_grid_viewport_2d(
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
            &prelude::VirtualizerState::new(2, ui_px(24.0))
                .with_viewport_extent(ui_px(24.0))
                .with_scroll_offset(ui_px(12.0)),
        );
    assert_eq!(root_grid_viewport.row_overscan_range().start(), 0);
    let header_action: root::TableHeaderAction = root_snapshot.columns()[0]
        .sort_action()
        .expect("sortable exported table column should expose a header action")
        .clone();
    let _root_cache_key: root::TableStateCacheKey = table.state().cache_key();
    let _prelude_header_action: prelude::TableHeaderAction = header_action;
    let _prelude_cache_key: prelude::TableStateCacheKey = table.state().cache_key();
    let _root_aggregation: root::TableAggregation =
        root::TableAggregation::new("score", root::TableAggregateKind::Sum);
    let _prelude_aggregation: prelude::TableAggregation =
        prelude::TableAggregation::average("score");
    let _root_expansion: root::TableExpansionState = root::TableExpansionState::all();
    let _prelude_expansion: prelude::TableExpansionState =
        prelude::TableExpansionState::rows([prelude::TableRowId::new("group:team=ui")]);
    let _root_expansion_mode: root::TableExpansionMode = root::TableExpansionMode::Manual;
    let _prelude_expansion_mode: prelude::TableExpansionMode = prelude::TableExpansionMode::Client;
    let _root_stage_mode: root::TableStageMode = root::TableStageMode::Manual;
    let _prelude_stage_mode: prelude::TableStageMode = prelude::TableStageMode::Client;
    let root_filter = root::TableFilter::one_of("status", ["Ready", "Blocked"]);
    let _prelude_filter: prelude::TableFilter = prelude::TableFilter::contains("team", "UI");
    let _root_filter_kind: root::TableFilterKind = root_filter.kind().clone();
    let _prelude_filter_kind: prelude::TableFilterKind =
        prelude::TableFilterKind::Contains { query: "UI".into() };
    let _root_text_filter_operator: root::TableTextFilterOperator =
        root::TableTextFilterOperator::StartsWith;
    let _prelude_text_filter_operator: prelude::TableTextFilterOperator =
        prelude::TableTextFilterOperator::NotContains;
    let _root_numeric_bound: root::TableNumericFilterBound =
        root::TableNumericFilterBound::new(10.0)
            .expect("finite numeric bounds should be constructible");
    let _prelude_numeric_bound: prelude::TableNumericFilterBound =
        prelude::TableNumericFilterBound::new(20.0)
            .expect("finite numeric bounds should be constructible");
    let _root_numeric_filter_operator: root::TableNumericFilterOperator =
        root::TableNumericFilterOperator::GreaterThanOrEqual;
    let _prelude_numeric_filter_operator: prelude::TableNumericFilterOperator =
        prelude::TableNumericFilterOperator::LessThan;
    let root_range_filter = root::TableFilter::number_range("score", Some(10.0), Some(20.0))
        .expect("exported numeric range filter should construct");
    assert_eq!(
        root_range_filter.number_range_bounds(),
        Some((Some(10.0), Some(20.0)))
    );
    let root_facet_value = root::TableFacetValueCount::new("Ready", 2);
    let root_facets: root::TableColumnFacets =
        root::TableColumnFacets::manual("status", 2).with_unique_values([root_facet_value]);
    let _prelude_facets: prelude::TableColumnFacets = root_facets.clone();
    let root_global_facets: root::TableGlobalFacetSummary =
        root::TableGlobalFacetSummary::default();
    let _prelude_global_facets: prelude::TableGlobalFacetSummary = root_global_facets.clone();
    let root_global_filter: root::TableGlobalFilter =
        root::TableGlobalFilter::new("root-global-filter", "Search").query("ready");
    let _root_global_filter_state: root::TableGlobalFilterState = root_global_filter.state();
    let _root_global_filter_change: root::TableGlobalFilterChange =
        root::TableGlobalFilterChange::new("ready");
    let root_predicate_operator: root::TablePredicateFilterOperator =
        root::TablePredicateFilterOperator::text(root::TableTextFilterOperator::StartsWith);
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
            prelude::TableNumericFilterOperator::GreaterThan,
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
                root::TableColumn::new("name", "Name").with_hideable(false),
                root::TableColumn::new("status", "Status"),
            ])
            .visibility(root::TableColumnVisibilityOverrides::new().hide("status"));
    let root_column_visibility_state: root::TableColumnVisibilityState =
        root_column_visibility.state();
    let _root_column_visibility_item: Option<&root::TableColumnVisibilityItemState> =
        root_column_visibility_state.items().first();
    let root_column_visibility_change: root::TableColumnVisibilityChange =
        root::TableColumnVisibilityChange::new("status", false);
    let _root_column_visibility_action: root::TableColumnVisibilityAction =
        root_column_visibility_change.action();
    let root_column_order_change: root::TableColumnOrderChange =
        root::TableColumnOrderChange::move_before("score", "team", root::TableColumnRegion::Center);
    let _root_column_order_placement: root::TableColumnOrderPlacement =
        root_column_order_change.placement();
    let prelude_column_visibility: prelude::TableColumnVisibility =
        prelude::TableColumnVisibility::new("prelude-columns", "Columns")
            .columns([prelude::TableColumn::new("status", "Status")])
            .default_visibility(prelude::TableColumnVisibilityOverrides::new().hide("status"));
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
    let _root_facet_range: Option<root::TableFacetRange> = root::TableFacetRange::new(1.0, 2.0);
    let root_range_facets =
        root::TableColumnFacets::manual("score", 2).with_numeric_range(1.0, 20.0);
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
    let _prelude_facet_value: prelude::TableFacetValueCount =
        prelude::TableFacetValueCount::new("Blocked", 1);
    let _root_child_load_state: root::TableRowChildrenLoadState =
        root::TableRowChildrenLoadState::loading("Loading children");
    let _prelude_child_load_state: prelude::TableRowChildrenLoadState =
        prelude::TableRowChildrenLoadState::failed("Load failed");
    let _prelude_row_kind: prelude::TableResolvedRowKind = prelude::TableResolvedRowKind::Leaf;
    let root_tree_state = root::TableState::new([root::TableRow::new("root")
        .with_cell("name", "Root")
        .with_child(root::TableRow::new("child").with_cell("name", "Child"))])
    .with_columns([root::TableColumn::new("name", "Name")])
    .with_all_rows_expanded();
    let root_tree_row: root::TableTreeRow = root_tree_state.resolve().final_model().rows()[0]
        .tree()
        .expect("tree source row should expose hierarchy metadata")
        .clone();
    let _prelude_tree_row: prelude::TableTreeRow = root_tree_row;
    let _resolved_kind: Option<&root::TableGroupRow> =
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
    let _root_pinning: root::TableColumnPinning =
        root::TableColumnPinning::new().pinned_left(["name"]);
    let _root_width_policy: root::TableColumnWidthPolicy = root::TableColumnWidthPolicy::ContentFit;
    let _prelude_width_policy: prelude::TableColumnWidthPolicy =
        prelude::TableColumnWidthPolicy::Fixed;
    let content_fit_column = root::TableColumn::new("status", "Status").with_content_fit();
    assert!(content_fit_column.is_content_fit());
    assert_eq!(
        content_fit_column.width_policy(),
        root::TableColumnWidthPolicy::ContentFit
    );
    let root_visibility = root::TableColumnVisibilityOverrides::new()
        .hide("score")
        .show("status")
        .without("missing");
    let _root_visibility: root::TableColumnVisibilityOverrides = root_visibility.clone();
    let _prelude_visibility: prelude::TableColumnVisibilityOverrides =
        prelude::TableColumnVisibilityOverrides::new().show("status");
    assert_eq!(
        root_visibility.override_for(&root::TableColumnId::new("score")),
        Some(false)
    );
    let root_sizing = root::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let _root_sizing: root::TableColumnSizing = root_sizing.clone();
    let _prelude_sizing: prelude::TableColumnSizing =
        prelude::TableColumnSizing::new().with_width("name", ui_px(180.0));
    let root_resize_state = root::TableColumnResizeState::begin(
        "name",
        ui_px(12.0),
        ui_px(180.0),
        [("name", ui_px(180.0))],
    );
    let root_resize_update: root::TableColumnResizeUpdate = root::drag_table_column_resize(
        root::TableColumnResizeMode::OnChange,
        root::TableColumnResizeDirection::Ltr,
        &root_sizing,
        &root_resize_state,
        ui_px(24.0),
    );
    let _prelude_resize_state: prelude::TableColumnResizeState = root_resize_update.state().clone();
    let _prelude_resize_update: prelude::TableColumnResizeUpdate = root::end_table_column_resize(
        prelude::TableColumnResizeMode::OnEnd,
        prelude::TableColumnResizeDirection::Ltr,
        &prelude::TableColumnSizing::new().with_width("name", ui_px(180.0)),
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
    let _root_resolved_sizing: root::TableResolvedColumnSizing = table
        .state()
        .resolve()
        .visible_column_sizing()
        .column(&root::TableColumnId::new("name"))
        .expect("resolved column sizing should be available")
        .clone();
    let _prelude_resolved_sizing: prelude::TableResolvedColumnSizing =
        _root_resolved_sizing.clone();
    let _root_resolved_sizing_regions: root::TableResolvedColumnSizingRegions =
        table.state().resolve().visible_column_sizing().clone();
    let _prelude_resolved_sizing_regions: prelude::TableResolvedColumnSizingRegions =
        _root_resolved_sizing_regions.clone();
    let _root_default_width = root::TABLE_DEFAULT_COLUMN_WIDTH;
    let _root_min_width = root::TABLE_MIN_COLUMN_WIDTH;
    let _root_max_width = root::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_default_width = prelude::TABLE_DEFAULT_COLUMN_WIDTH;
    let _prelude_min_width = prelude::TABLE_MIN_COLUMN_WIDTH;
    let _prelude_max_width = prelude::TABLE_MAX_COLUMN_WIDTH;
    let _prelude_region: prelude::TableColumnRegion = prelude::TableColumnRegion::Center;
    let _prelude_regions: prelude::TableColumnRegions =
        table.state().resolve().visible_column_regions().clone();

    assert_eq!(root_snapshot.role(), Role::Table);
    assert!(!root_snapshot.columns().is_empty());
    assert_eq!(
        root::TableRowActivationKind::DoubleClick.as_str(),
        "double-click"
    );
    assert_eq!(virtualizer.resolve().overscan(), 2);
}
