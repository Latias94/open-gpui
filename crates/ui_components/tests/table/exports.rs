use super::*;

#[test]
fn table_public_exports_keep_component_surface_and_core_owner_paths_explicit() {
    use open_gpui_ui_components::{self as root, prelude};
    use open_gpui_ui_core as ui_core;

    let state = ui_core::TableState::new([
        ui_core::TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("status", "Ready"),
        ui_core::TableRow::new("row-b")
            .with_cell("name", "Beta")
            .with_cell("status", "Queued"),
    ])
    .with_columns([
        ui_core::TableColumn::new("name", "Name"),
        ui_core::TableColumn::new("status", "Status"),
    ])
    .with_column_pinning(ui_core::TableColumnPinning::new().pinned_left(["name"]));

    let table: root::Table = root::Table::new("root-table", "Root table", state.clone());
    let _prelude_table: prelude::Table =
        prelude::Table::new("prelude-table", "Prelude table", state.clone());
    let state_readout: &ui_core::TableState = table.state();
    let resolved_state: ui_core::TableResolvedState = state_readout.resolve();
    assert_eq!(resolved_state.final_model().rows().len(), 2);

    let snapshot: root::TableBehaviorSnapshot = table.behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let _prelude_snapshot: prelude::TableBehaviorSnapshot = snapshot.clone();
    let _region_snapshot: root::TableColumnRegionSnapshot = snapshot.column_regions();
    let _prelude_region_snapshot: prelude::TableColumnRegionSnapshot = snapshot.column_regions();
    let _header_summary: root::TableHeaderSummarySnapshot = snapshot.header_summary();
    let _row_counts: root::TableRowCountSnapshot = snapshot.row_counts();
    assert_eq!(snapshot.role(), ui_core::Role::Table);
    assert_eq!(snapshot.row_role(), ui_core::Role::Row);
    assert_eq!(snapshot.column_header_role(), ui_core::Role::ColumnHeader);
    assert_eq!(snapshot.cell_role(), ui_core::Role::Cell);
    let row_snapshot: &root::TableRowBehaviorSnapshot = &snapshot.rows()[0];
    assert_eq!(row_snapshot.role(), ui_core::Role::Row);
    let cell_snapshot: &root::TableCellBehaviorSnapshot = &row_snapshot.cells()[0];
    assert_eq!(cell_snapshot.role(), ui_core::Role::Cell);
    let _header_action: root::TableHeaderAction = snapshot.columns()[0]
        .sort_action()
        .expect("sortable exported table column should expose a header action")
        .clone();
    let _state_cache_key: ui_core::TableStateCacheKey = state_readout.cache_key();

    let group = ui_core::TableColumnGroup::new(
        ui_core::TableColumnGroupId::new("identity"),
        "Identity",
        [ui_core::TableColumn::new("name", "Name")],
    )
    .with_child(ui_core::TableColumn::new("status", "Status"));
    let column_tree_state =
        ui_core::TableState::new([ui_core::TableRow::new("row-c").with_cell("name", "Gamma")])
            .with_column_tree([group.clone()]);
    let _column_node: &ui_core::TableColumnNode = &column_tree_state.column_tree()[0];
    let _column_group: ui_core::TableColumnGroup = group;

    let virtualizer = ui_core::VirtualizerState::new(4, ui_px(24.0)).with_overscan(2);
    let viewport: ui_core::GridViewport2D = ui_core::resolve_grid_viewport_2d(
        &virtualizer
            .clone()
            .with_viewport_extent(ui_px(48.0))
            .with_scroll_offset(ui_px(12.0)),
        &ui_core::VirtualizerState::new(2, ui_px(80.0))
            .with_viewport_extent(ui_px(80.0))
            .with_scroll_offset(ui_px(40.0)),
    );
    let _virtualizer_state: ui_core::VirtualizerResolvedState = virtualizer.resolve();
    assert_eq!(viewport.row_overscan_range().start(), 0);

    let _row_pin_target: ui_core::TableRowPinTarget =
        ui_core::TableRowPinTarget::exact(ui_core::TableRowIdentity::source("row-a"));
    let _row_pinning: ui_core::TableRowPinning = ui_core::TableRowPinning::new()
        .pinned_top([ui_core::TableRowIdentity::source("row-a")])
        .pinned_bottom([ui_core::TableRowIdentity::source("row-b")]);
    let _row_pinning_policy: ui_core::TableRowPinningPolicy =
        ui_core::TableRowPinningPolicy::KeepPinnedRows;
    let _row_region: ui_core::TableRowRegion = ui_core::TableRowRegion::Top;
    let _expansion_mode: ui_core::TableExpansionMode = ui_core::TableExpansionMode::Client;
    let _stage_mode: ui_core::TableStageMode = ui_core::TableStageMode::Manual;
    let _filter: ui_core::TableFilter = ui_core::TableFilter::contains("status", "Ready");
    let _facet_range: Option<ui_core::TableFacetRange> = ui_core::TableFacetRange::new(1.0, 2.0);
    let _child_load_state: ui_core::TableRowChildrenLoadState =
        ui_core::TableRowChildrenLoadState::loading("Loading children");

    assert!(!snapshot.columns().is_empty());
}

#[test]
fn table_public_exports_include_component_table_controls_only() {
    use open_gpui_ui_components::{self as root, prelude};
    use open_gpui_ui_core as ui_core;

    let facets = ui_core::TableColumnFacets::manual("status", 2).with_unique_values([
        ui_core::TableFacetValueCount::new("Ready", 1),
        ui_core::TableFacetValueCount::new("Queued", 1),
    ]);

    let global_filter: root::TableGlobalFilter =
        root::TableGlobalFilter::new("root-global-filter", "Search").query("ready");
    let _global_filter_state: root::TableGlobalFilterState = global_filter.state();
    let _global_filter_change: root::TableGlobalFilterChange =
        root::TableGlobalFilterChange::new("ready");

    let predicate_operator: root::TablePredicateFilterOperator =
        root::TablePredicateFilterOperator::text(ui_core::TableTextFilterOperator::StartsWith);
    let predicate_filter: root::TablePredicateFilter =
        root::TablePredicateFilter::new("root-name-predicate", "Name", "name")
            .operator(predicate_operator)
            .value("Al");
    let predicate_state: root::TablePredicateFilterState = predicate_filter.state();
    let _predicate_option: Option<&root::TablePredicateFilterOperatorOptionState> =
        predicate_state.operator_options().first();
    let _predicate_change: root::TablePredicateFilterChange =
        root::TablePredicateFilterChange::new("name", predicate_operator, "Al");

    let toolbar: root::TableToolbar =
        root::TableToolbar::new("root-table-toolbar", "Filters").summary("2 visible controls");
    let toolbar_state: root::TableToolbarState = toolbar.state();
    assert_eq!(toolbar_state.role(), ui_core::Role::Toolbar);
    let _toolbar_colors: root::TableToolbarColors = toolbar_state.colors();

    let faceted_filter: root::TableFacetedFilter =
        root::TableFacetedFilter::new("root-status-filter", "Status", "status")
            .facets(facets.clone())
            .selected_values(["Ready"]);
    let faceted_state: root::TableFacetedFilterState = faceted_filter.state();
    let _faceted_option: Option<&root::TableFacetedFilterOptionState> =
        faceted_state.options().first();
    let _faceted_change: root::TableFacetedFilterChange =
        root::TableFacetedFilterChange::new("status", ["Ready"], Some("Ready"), true);

    let column_visibility: root::TableColumnVisibility =
        root::TableColumnVisibility::new("root-columns", "Columns")
            .columns([
                ui_core::TableColumn::new("name", "Name").with_hideable(false),
                ui_core::TableColumn::new("status", "Status"),
            ])
            .visibility(ui_core::TableColumnVisibilityOverrides::new().hide("status"));
    let column_visibility_state: root::TableColumnVisibilityState = column_visibility.state();
    let _column_visibility_item: Option<&root::TableColumnVisibilityItemState> =
        column_visibility_state.items().first();
    let column_visibility_change: root::TableColumnVisibilityChange =
        root::TableColumnVisibilityChange::new("status", false);
    let _column_visibility_action: root::TableColumnVisibilityAction =
        column_visibility_change.action();

    let column_order_change: root::TableColumnOrderChange =
        root::TableColumnOrderChange::move_before(
            "score",
            "team",
            ui_core::TableColumnRegion::Center,
        );
    let _column_order_placement: root::TableColumnOrderPlacement = column_order_change.placement();
    let sizing_change = root::TableColumnSizingChange::new(
        "name",
        ui_px(204.0),
        ui_core::TableColumnSizing::new().with_width("name", ui_px(204.0)),
    );
    assert_eq!(sizing_change.width(), ui_px(204.0));

    let range_filter: root::TableRangeFilter =
        root::TableRangeFilter::new("root-score-range", "Score", "score")
            .facets(facets)
            .range(Some(1.0), Some(20.0));
    let _range_filter_state: root::TableRangeFilterState = range_filter.state();
    let _range_change: root::TableRangeFilterChange =
        root::TableRangeFilterChange::new("score", "1", "20");

    let _row_measure_mode: root::TableRowMeasureMode = root::TableRowMeasureMode::Measured;
    let _prelude_row_measure_mode: prelude::TableRowMeasureMode =
        prelude::TableRowMeasureMode::Fixed;
    let _table_modifiers: root::TableInputModifiers = root::TableInputModifiers::default();
    let _row_action: Option<root::TableRowAction> = None;
    let _row_activation: Option<root::TableRowActivation> = None;
    let _row_expansion: Option<root::TableRowExpansionToggle> = None;
    let activation_kind: root::TableRowActivationKind = root::TableRowActivationKind::DoubleClick;
    assert_eq!(activation_kind.as_str(), "double-click");

    let _root_global_filter_for_recipe: root::TableGlobalFilter =
        root::TableGlobalFilter::new("prelude-global-filter", "Search");
    let _root_predicate_filter_for_recipe: root::TablePredicateFilter =
        root::TablePredicateFilter::new("prelude-score-predicate", "Score", "score").operator(
            root::TablePredicateFilterOperator::number(
                ui_core::TableNumericFilterOperator::GreaterThan,
            ),
        );
    let _root_faceted_filter_for_recipe: root::TableFacetedFilter =
        root::TableFacetedFilter::new("prelude-status-filter", "Status", "status");
    let _root_column_visibility_for_recipe: root::TableColumnVisibility =
        root::TableColumnVisibility::new("prelude-columns", "Columns");
    let _root_range_filter_for_recipe: root::TableRangeFilter =
        root::TableRangeFilter::new("prelude-score-range", "Score", "score");
    let _root_column_order_change: root::TableColumnOrderChange = column_order_change;
    let _root_sizing_change: root::TableColumnSizingChange = sizing_change;
}
