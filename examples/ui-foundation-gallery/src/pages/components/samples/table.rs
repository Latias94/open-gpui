use super::*;

/// One table sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Resolved renderer-neutral table state.
    pub state: TableState,
    /// Visual size applied to the concrete table.
    pub size: Size,
    /// Fixed table body viewport used by the sample.
    pub viewport_extent: UiPx,
    /// Fixed row height used by the virtualizer.
    pub row_height: UiPx,
    /// Overscan row budget.
    pub overscan: usize,
    /// Precomputed state summary used by the gallery page.
    state_summary: TableSampleStateSummary,
}

/// Precomputed state summary for a table sample.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableSampleStateSummary {
    /// Core row count after source resolution.
    pub core_rows: usize,
    /// Filtered row count after filters apply.
    pub filtered_rows: usize,
    /// Final row count after pagination.
    pub final_rows: usize,
    /// Top-pinned row count in the final visual model.
    pub pinned_top_rows: usize,
    /// Center row count used by the row virtualizer.
    pub pinned_center_rows: usize,
    /// Bottom-pinned row count in the final visual model.
    pub pinned_bottom_rows: usize,
    /// Whether row pinning is limited to the current page.
    pub row_pinning_page_only: bool,
    /// Rendered body row count after overscan.
    pub rendered_rows: usize,
    /// Visible body row count before overscan.
    pub visible_rows: usize,
    /// Visible row range start.
    pub visible_start: usize,
    /// Visible row range end.
    pub visible_end: usize,
    /// Overscan row range start.
    pub overscan_start: usize,
    /// Overscan row range end.
    pub overscan_end: usize,
    /// Visible column count.
    pub aria_columns: usize,
    /// Accessible row count including the header row.
    pub aria_rows: usize,
    /// Selected row count in the final model.
    pub selected_rows: usize,
    /// Visible header row count across all rendered regions.
    pub header_rows: usize,
    /// Unique visible group header count across all rendered regions.
    pub header_groups: usize,
    /// Visible leaf column count across all rendered regions.
    pub visible_leaf_columns: usize,
    /// Row count before expansion flattens the grouped tree.
    pub grouped_rows: usize,
    /// Row count after expansion applies.
    pub expanded_rows: usize,
    /// Visible group row count in the final model.
    pub group_rows: usize,
    /// Visible leaf row count in the final model.
    pub leaf_rows: usize,
    /// Visible tree row count in the final model.
    pub tree_rows: usize,
    /// Visible tree branch row count in the final model.
    pub tree_branch_rows: usize,
    /// Visible expandable tree rows without loaded children.
    pub unloaded_tree_branches: usize,
    /// Visible tree rows currently marked as loading children.
    pub loading_tree_rows: usize,
    /// Visible tree rows currently marked as failed child loads.
    pub failed_tree_rows: usize,
    /// Deepest visible tree depth in the final model.
    pub tree_depth: usize,
    /// Whether the sample keeps expansion pruning app-owned.
    pub manual_expansion: bool,
    /// Whether filtering is app-owned.
    pub manual_filtering: bool,
    /// Whether sorting is app-owned.
    pub manual_sorting: bool,
    /// Whether pagination is app-owned.
    pub manual_pagination: bool,
    /// Zero-based page index in the current snapshot.
    pub pagination_page_index: usize,
    /// Page size in the current snapshot.
    pub pagination_page_size: usize,
    /// Server-known total row count, if any.
    pub pagination_row_count: Option<usize>,
    /// Total page count, if any.
    pub pagination_page_count: Option<usize>,
    /// Resolved facet summary count.
    pub facet_columns: usize,
    /// Resolved caller-owned facet summary count.
    pub manual_facet_columns: usize,
    /// Unique status facet value count.
    pub status_facet_values: usize,
    /// Sum of status facet value counts.
    pub status_facet_total_count: usize,
    /// Rounded score facet minimum, if present.
    pub score_facet_min: Option<usize>,
    /// Rounded score facet maximum, if present.
    pub score_facet_max: Option<usize>,
    /// Configured grouping column count.
    pub grouping_columns: usize,
    /// Configured aggregate column count.
    pub aggregation_count: usize,
    /// Named custom aggregate callback count.
    pub custom_aggregation_count: usize,
    /// Explicit expanded group row ids, or all group rows when expansion is global.
    pub expanded_group_inputs: usize,
    /// Explicit expanded tree row ids, or all tree branch rows when expansion is global.
    pub expanded_tree_inputs: usize,
    /// Whether every group row is expanded.
    pub all_rows_expanded: bool,
    /// Visible left-pinned columns.
    pub pinned_left_columns: usize,
    /// Visible unpinned center columns.
    pub pinned_center_columns: usize,
    /// Visible right-pinned columns.
    pub pinned_right_columns: usize,
    /// Rounded visible left-pinned lane width.
    pub pinned_left_width_px: usize,
    /// Rounded visible center lane width.
    pub pinned_center_width_px: usize,
    /// Rounded visible right-pinned lane width.
    pub pinned_right_width_px: usize,
    /// Rounded total visible column width.
    pub total_column_width_px: usize,
    /// Visible resizable columns.
    pub resizable_columns: usize,
}

impl TableSampleStateSummary {
    fn from_snapshot(snapshot: &TableBehaviorSnapshot) -> Self {
        let visible = snapshot.visible_rows();
        let rows = snapshot.row_counts();
        let columns = snapshot.column_regions();
        let header = snapshot.header_summary();
        let tree = snapshot.tree_summary();
        let status_column = TableColumnId::new("status");
        let score_column = TableColumnId::new("score");
        let status_facet = snapshot.column_facet(&status_column);
        let score_range = snapshot
            .column_facet(&score_column)
            .and_then(|facet| facet.numeric_range());
        let score_facet_min = score_range.map(|range| range.min().round() as usize);
        let score_facet_max = score_range.map(|range| range.max().round() as usize);

        Self {
            core_rows: rows.core_rows(),
            filtered_rows: rows.filtered_rows(),
            final_rows: rows.final_rows(),
            pinned_top_rows: rows.pinned_top_rows(),
            pinned_center_rows: rows.pinned_center_rows(),
            pinned_bottom_rows: rows.pinned_bottom_rows(),
            row_pinning_page_only: columns.row_pinning_page_only(),
            rendered_rows: rows.rendered_rows(),
            visible_rows: rows.visible_rows(),
            visible_start: visible.visible_start(),
            visible_end: visible.visible_end(),
            overscan_start: visible.overscan_start(),
            overscan_end: visible.overscan_end(),
            aria_columns: snapshot.aria_column_count(),
            aria_rows: snapshot.aria_row_count(),
            selected_rows: rows.selected_rows(),
            header_rows: header.header_rows(),
            header_groups: header.visible_group_headers(),
            visible_leaf_columns: snapshot.columns().len(),
            grouped_rows: rows.grouped_rows(),
            expanded_rows: rows.expanded_rows(),
            group_rows: rows.group_rows(),
            leaf_rows: rows.leaf_rows(),
            tree_rows: tree.tree_rows(),
            tree_branch_rows: tree.tree_branch_rows(),
            unloaded_tree_branches: tree.unloaded_tree_branches(),
            loading_tree_rows: tree.loading_tree_rows(),
            failed_tree_rows: tree.failed_tree_rows(),
            tree_depth: tree.tree_depth(),
            manual_expansion: snapshot.manual_expansion(),
            manual_filtering: snapshot.filtering_mode() == TableStageMode::Manual,
            manual_sorting: snapshot.sorting_mode() == TableStageMode::Manual,
            manual_pagination: snapshot.pagination_mode() == TableStageMode::Manual,
            pagination_page_index: snapshot.pagination_page_index(),
            pagination_page_size: snapshot.pagination_page_size(),
            pagination_row_count: snapshot.pagination_row_count(),
            pagination_page_count: snapshot.pagination_page_count(),
            facet_columns: snapshot.column_facets().len(),
            manual_facet_columns: snapshot
                .column_facets()
                .iter()
                .filter(|facet| facet.mode() == TableStageMode::Manual)
                .count(),
            status_facet_values: status_facet
                .map(|facet| facet.unique_values().len())
                .unwrap_or(0),
            status_facet_total_count: status_facet
                .map(|facet| {
                    facet
                        .unique_values()
                        .iter()
                        .map(|entry| entry.count())
                        .sum()
                })
                .unwrap_or(0),
            score_facet_min,
            score_facet_max,
            grouping_columns: snapshot.grouping_columns().len(),
            aggregation_count: snapshot.aggregation_count(),
            custom_aggregation_count: snapshot.aggregation_fn_count(),
            expanded_group_inputs: snapshot.expanded_group_inputs(),
            expanded_tree_inputs: snapshot.expanded_tree_inputs(),
            all_rows_expanded: snapshot.all_rows_expanded(),
            pinned_left_columns: columns.left_columns(),
            pinned_center_columns: columns.center_columns(),
            pinned_right_columns: columns.right_columns(),
            pinned_left_width_px: columns.left_width().as_f32().round() as usize,
            pinned_center_width_px: columns.center_width().as_f32().round() as usize,
            pinned_right_width_px: columns.right_width().as_f32().round() as usize,
            total_column_width_px: columns.total_width().as_f32().round() as usize,
            resizable_columns: columns.resizable_columns(),
        }
    }
}

impl TableSample {
    /// Builds the concrete GPUI table for this sample.
    pub fn build_table(&self) -> Table {
        self.build_table_with_sizing(self.state.column_sizing().clone())
    }

    /// Builds the concrete GPUI table with caller-owned column sizing.
    pub fn build_table_with_sizing(&self, column_sizing: TableColumnSizing) -> Table {
        self.build_table_with_state(self.state.clone().with_column_sizing(column_sizing))
    }

    /// Builds the concrete GPUI table from a fully resolved sample state.
    pub fn build_table_with_state(&self, state: TableState) -> Table {
        Table::new(format!("component-table:{}", self.id), self.title, state)
            .with_size(self.size)
            .viewport_extent(self.viewport_extent)
            .row_height(self.row_height)
            .overscan(self.overscan)
    }

    /// Resolves the public table behavior used by gallery tests and state rows.
    pub fn behavior_snapshot(&self) -> TableBehaviorSnapshot {
        self.build_table()
            .behavior_snapshot(UiPx::ZERO, self.viewport_extent)
    }

    /// Returns the precomputed state summary used by the gallery page.
    pub const fn state_summary(&self) -> TableSampleStateSummary {
        self.state_summary
    }

    /// Resolves the summary for a caller-supplied table state using this sample's layout settings.
    pub fn state_summary_for_state(&self, state: &TableState) -> TableSampleStateSummary {
        let snapshot = self
            .build_table_with_state(state.clone())
            .behavior_snapshot(UiPx::ZERO, self.viewport_extent);
        TableSampleStateSummary::from_snapshot(&snapshot)
    }
}

const RELEASE_MATRIX_METRIC_COUNT: usize = 14;

static TABLE_SAMPLES: LazyLock<Vec<TableSample>> = LazyLock::new(build_table_samples);

/// Returns table samples backed by real table and virtualizer contracts.
pub fn table_samples(_tokens: ThemeTokens) -> &'static [TableSample] {
    TABLE_SAMPLES.as_slice()
}

fn build_table_samples() -> Vec<TableSample> {
    let release_queue_rows = (0..10_000).map(release_queue_row).collect::<Vec<_>>();
    let filter_board_rows = (0..180).map(filter_board_row).collect::<Vec<_>>();
    let server_paged_rows = server_paged_rows();
    let release_resize_rows = (0..160).map(release_resize_row).collect::<Vec<_>>();
    let editable_release_rows = (0..32).map(editable_release_row).collect::<Vec<_>>();
    let toggle_release_rows = (0..28).map(toggle_release_row).collect::<Vec<_>>();
    let select_release_rows = (0..28).map(select_release_row).collect::<Vec<_>>();
    let multiline_release_rows = (0..24).map(multiline_release_row).collect::<Vec<_>>();
    let grouped_release_rows = (0..320).map(grouped_release_row).collect::<Vec<_>>();
    let grouped_custom_aggregation_rows = (0..8)
        .map(grouped_custom_aggregation_row)
        .collect::<Vec<_>>();
    let release_matrix_rows = (0..480).map(release_matrix_row).collect::<Vec<_>>();
    let row_pinning_rows = (0..96).map(row_pinning_row).collect::<Vec<_>>();
    let dependency_tree_rows = dependency_tree_rows();

    let release_queue = TableSample {
        id: "release-queue",
        title: "Release queue",
        summary: "Ten thousand exact row identities with a local virtualized viewport and root-proxy keyboard continuity.",
        badge: "10k rows",
        state: TableState::new(release_queue_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows([TableSourceRowIdentity::unique("release-queue-row-0005")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 5,
        state_summary: TableSampleStateSummary::default(),
    };
    let filter_board = TableSample {
        id: "filter-board",
        title: "Filtered board",
        summary: "Filtered, sorted, and paginated rows keep selection tied to row ids.",
        badge: "filtered",
        state: TableState::new(filter_board_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_filters([TableFilter::contains("team", "UI")])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows([TableSourceRowIdentity::unique("filter-board-row-177")])
            .with_pagination(TablePagination::new(0, 24)),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let server_paged = TableSample {
        id: "server-paged",
        title: "Server paged board",
        summary: "Manual filtering, sorting, and pagination render a server-owned page snapshot with total counts.",
        badge: "manual rows",
        state: TableState::new(server_paged_rows)
            .with_columns(table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_filters([TableFilter::contains("team", "missing")])
            .with_manual_filtering()
            .with_sorting([TableSort::ascending("score")])
            .with_manual_sorting()
            .with_selected_rows([TableSourceRowIdentity::unique("server-paged-row-0018")])
            .with_pagination(TablePagination::manual(2, 8, 64))
            .with_manual_facets([
                TableColumnFacets::manual("score", 64).with_numeric_range(1.0, 64.0),
                TableColumnFacets::manual("status", 64).with_unique_values([
                    TableFacetValueCount::new("Blocked", 16),
                    TableFacetValueCount::new("Queued", 16),
                    TableFacetValueCount::new("Ready", 16),
                    TableFacetValueCount::new("Review", 16),
                ]),
            ]),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let release_resize = TableSample {
        id: "release-resize",
        title: "Resizable release table",
        summary: "Controlled column widths with live resize handles and a fixed score column.",
        badge: "resizable",
        state: TableState::new(release_resize_rows)
            .with_columns(resizable_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(188.0))
                    .with_width("team", ui_px(116.0))
                    .with_width("status", ui_px(132.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_sorting([TableSort::descending("score")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let content_fit_release = TableSample {
        id: "content-fit-release",
        title: "Content-fit release table",
        summary: "A fit-content identity column widens from visible edits while a fixed score column stays anchored.",
        badge: "content fit",
        state: TableState::new(editable_release_rows.clone())
            .with_columns(content_fit_release_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_selected_rows([TableSourceRowIdentity::unique("editable-release-row-002")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let editable_release = TableSample {
        id: "editable-release",
        title: "Editable release cells",
        summary: "Text-cell editors emit exact logical-row and column identity payloads while app-owned rows feed updates back into Table.",
        badge: "cell edit",
        state: TableState::new(editable_release_rows)
            .with_columns(editable_table_columns())
            .with_column_order(["name", "team", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(204.0))
                    .with_width("team", ui_px(132.0))
                    .with_width("status", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows([TableSourceRowIdentity::unique("editable-release-row-002")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let toggle_release = TableSample {
        id: "toggle-release",
        title: "Toggle release cells",
        summary: "Checkbox cell editors preserve exact row and column identity while app-owned rows feed bool updates back into Table.",
        badge: "checkbox cells",
        state: TableState::new(toggle_release_rows)
            .with_columns(toggle_release_table_columns())
            .with_column_order(["name", "enabled", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(196.0))
                    .with_width("enabled", ui_px(104.0))
                    .with_width("status", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows([TableSourceRowIdentity::unique("toggle-release-row-002")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let select_release = TableSample {
        id: "select-release",
        title: "Select release cells",
        summary: "Fixed-option select editors preserve exact row and column identity while app-owned rows feed choices back into Table.",
        badge: "select cells",
        state: TableState::new(select_release_rows)
            .with_columns(select_release_table_columns())
            .with_column_order(["name", "status", "team", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(196.0))
                    .with_width("status", ui_px(132.0))
                    .with_width("team", ui_px(128.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows([TableSourceRowIdentity::unique("select-release-row-002")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(34.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let multiline_release = TableSample {
        id: "multiline-release",
        title: "Multiline release notes",
        summary: "Textarea cell editors preserve exact row and column identity plus newlines while app-owned rows feed updates back into Table.",
        badge: "textarea cells",
        state: TableState::new(multiline_release_rows)
            .with_columns(multiline_edit_table_columns())
            .with_column_order(["name", "notes", "status", "score"])
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(164.0))
                    .with_width("notes", ui_px(264.0))
                    .with_width("status", ui_px(112.0))
                    .with_width("score", ui_px(84.0)),
            )
            .with_selected_rows([TableSourceRowIdentity::unique("multiline-release-row-002")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(220.0),
        row_height: ui_px(82.0),
        overscan: 3,
        state_summary: TableSampleStateSummary::default(),
    };
    let grouped_release = TableSample {
        id: "release-rollup",
        title: "Release rollup",
        summary: "Normalized caller-owned column reorders keep every source column while fixed lanes frame the scrolling center.",
        badge: "sticky pinned",
        state: TableState::new(grouped_release_rows)
            .with_columns(sticky_pinned_table_columns())
            .with_column_order(["name", "team", "score", "status"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_grouping(["team"])
            .with_expanded_rows([
                TableRowIdentity::group(TableGroupRowIdentity::new("team", "UI")),
                TableRowIdentity::group(TableGroupRowIdentity::new("team", "Platform")),
            ])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::sum("score"),
            ])
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows([TableSourceRowIdentity::unique("grouped-release-row-000")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let grouped_custom_aggregation = TableSample {
        id: "grouped-custom-aggregation",
        title: "Custom aggregation",
        summary: "Grouped rows combine a built-in count with a named custom score aggregate.",
        badge: "custom aggregate",
        state: TableState::new(grouped_custom_aggregation_rows)
            .with_columns(sticky_pinned_table_columns())
            .with_column_order(["name", "team", "score", "status"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_grouping(["team"])
            .with_expanded_rows([
                TableRowIdentity::group(TableGroupRowIdentity::new("team", "UI")),
                TableRowIdentity::group(TableGroupRowIdentity::new("team", "Platform")),
            ])
            .with_aggregations([
                TableAggregation::count("name"),
                TableAggregation::named("score", "score_plus_one"),
            ])
            .with_aggregation_fn("score_plus_one", |column, rows| {
                let score = rows.iter().fold(0.0, |sum, row| match row.cell(column) {
                    Some(TableCellValue::Number(value)) => sum + *value,
                    _ => sum,
                });
                TableCellValue::Number(score + 1.0)
            })
            .with_sorting([TableSort::descending("score")])
            .with_selected_rows([TableSourceRowIdentity::unique(
                "grouped-custom-aggregation-row-000",
            )])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let release_matrix = TableSample {
        id: "release-matrix",
        title: "Release matrix",
        summary: "Nested release groups keep pinned identity and status lanes fixed around a wide virtualized center window.",
        badge: "column window",
        state: TableState::new(release_matrix_rows)
            .with_column_tree(release_matrix_column_tree())
            .with_column_order(release_matrix_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_sorting([TableSort::descending("metric_13")])
            .with_selected_rows([TableSourceRowIdentity::unique("release-matrix-row-005")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let row_pinning = TableSample {
        id: "row-pinning",
        title: "Pinned row review",
        summary: "Top and bottom review rows stay visible while the paged center body scrolls.",
        badge: "row pins",
        state: TableState::new(row_pinning_rows)
            .with_columns(release_matrix_table_columns())
            .with_column_order(release_matrix_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_row_pinning(
                TableRowPinning::new()
                    .pinned_top([TableRowIdentity::source("row-pinning-row-003")])
                    .pinned_bottom([
                        TableRowIdentity::source("row-pinning-row-030"),
                        TableRowIdentity::source("row-pinning-row-070"),
                    ]),
            )
            .with_selected_rows([TableSourceRowIdentity::unique("row-pinning-row-030")])
            .with_pagination(TablePagination::new(2, 12)),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let dependency_tree = TableSample {
        id: "dependency-tree",
        title: "Dependency tree",
        summary: "Nested source rows expose controlled expansion, row focus, and activation payloads.",
        badge: "tree rows",
        state: TableState::new(dependency_tree_rows)
            .with_columns(dependency_tree_table_columns())
            .with_column_order(dependency_tree_column_order())
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name"])
                    .pinned_right(["status"]),
            )
            .with_column_sizing(
                TableColumnSizing::new()
                    .with_width("name", ui_px(220.0))
                    .with_width("kind", ui_px(120.0))
                    .with_width("owner", ui_px(132.0))
                    .with_width("risk", ui_px(112.0))
                    .with_width("change", ui_px(148.0))
                    .with_width("score", ui_px(92.0))
                    .with_width("status", ui_px(132.0)),
            )
            .with_expanded_rows([TableRowIdentity::source("dependency-workspace")])
            .with_selected_rows([TableSourceRowIdentity::unique("dependency-ui")])
            .with_pagination(TablePagination::disabled()),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };
    let server_tree = TableSample {
        id: "server-tree",
        title: "Server tree",
        summary: "Manual expansion keeps async child loading app-owned while Table renders branch metadata.",
        badge: "manual expansion",
        state: server_tree_table_state(false),
        size: Size::Small,
        viewport_extent: ui_px(196.0),
        row_height: ui_px(30.0),
        overscan: 4,
        state_summary: TableSampleStateSummary::default(),
    };

    vec![
        release_queue.with_state_summary(),
        filter_board.with_state_summary(),
        server_paged.with_state_summary(),
        release_resize.with_state_summary(),
        content_fit_release.with_state_summary(),
        editable_release.with_state_summary(),
        toggle_release.with_state_summary(),
        select_release.with_state_summary(),
        multiline_release.with_state_summary(),
        grouped_release.with_state_summary(),
        grouped_custom_aggregation.with_state_summary(),
        release_matrix.with_state_summary(),
        row_pinning.with_state_summary(),
        dependency_tree.with_state_summary(),
        server_tree.with_state_summary(),
    ]
}

impl TableSample {
    fn with_state_summary(self) -> Self {
        let snapshot = self
            .build_table()
            .behavior_snapshot(UiPx::ZERO, self.viewport_extent);
        Self {
            state_summary: TableSampleStateSummary::from_snapshot(&snapshot),
            ..self
        }
    }
}

fn editable_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_text_editable(true)
            .with_width(ui_px(204.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("team", "Team")
            .with_text_editable(true)
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn toggle_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(196.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(260.0)),
        TableColumn::new("enabled", "Enabled")
            .with_checkbox_editor()
            .with_width(ui_px(104.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(128.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn select_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(196.0))
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(260.0)),
        TableColumn::new("status", "Status")
            .with_select_editor([
                TableSelectOption::new("ready", "Ready"),
                TableSelectOption::new("blocked", "Blocked"),
            ])
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(108.0))
            .with_max_width(ui_px(184.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn multiline_edit_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(164.0))
            .with_min_width(ui_px(132.0))
            .with_max_width(ui_px(240.0)),
        TableColumn::new("notes", "Notes")
            .with_multiline_text_editor(3)
            .with_width(ui_px(264.0))
            .with_min_width(ui_px(220.0))
            .with_max_width(ui_px(360.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(112.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0)),
    ]
}

fn table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name"),
        TableColumn::new("team", "Team"),
        TableColumn::new("status", "Status"),
        TableColumn::new("score", "Score"),
    ]
}

fn sticky_pinned_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(188.0))
            .with_min_width(ui_px(144.0))
            .with_max_width(ui_px(280.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(220.0))
            .with_min_width(ui_px(128.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(164.0))
            .with_min_width(ui_px(120.0))
            .with_max_width(ui_px(240.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(180.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(220.0)),
    ]
}

fn resizable_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_width(ui_px(188.0))
            .with_min_width(ui_px(140.0))
            .with_max_width(ui_px(280.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(116.0))
            .with_min_width(ui_px(92.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0))
            .with_resizable(false),
    ]
}

fn content_fit_release_table_columns() -> [TableColumn; 4] {
    [
        TableColumn::new("name", "Name")
            .with_text_editable(true)
            .with_content_fit()
            .with_min_width(ui_px(160.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("team", "Team")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(128.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(84.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(120.0))
            .with_resizable(false),
    ]
}

fn release_matrix_column_tree() -> Vec<TableColumnGroup> {
    vec![TableColumnGroup::new(
        "release",
        "Release",
        [
            TableColumnGroup::new(
                "identity",
                "Identity",
                [TableColumn::new("name", "Release")
                    .with_hideable(false)
                    .with_width(ui_px(172.0))
                    .with_min_width(ui_px(140.0))
                    .with_max_width(ui_px(260.0))],
            ),
            TableColumnGroup::new(
                "metrics",
                "Metrics",
                (0..RELEASE_MATRIX_METRIC_COUNT).map(|index| {
                    TableColumn::new(format!("metric_{index:02}"), format!("Metric {index:02}"))
                        .with_width(ui_px(92.0 + (index % 4) as f32 * 12.0))
                        .with_min_width(ui_px(72.0))
                        .with_max_width(ui_px(180.0))
                }),
            ),
            TableColumnGroup::new(
                "delivery",
                "Delivery",
                [TableColumn::new("status", "Status")
                    .with_hideable(false)
                    .with_width(ui_px(148.0))
                    .with_min_width(ui_px(112.0))
                    .with_max_width(ui_px(220.0))],
            ),
        ],
    )]
}

fn release_matrix_table_columns() -> Vec<TableColumn> {
    let mut columns = Vec::with_capacity(RELEASE_MATRIX_METRIC_COUNT + 2);
    columns.push(
        TableColumn::new("name", "Release")
            .with_hideable(false)
            .with_width(ui_px(172.0))
            .with_min_width(ui_px(140.0))
            .with_max_width(ui_px(260.0)),
    );
    columns.extend((0..RELEASE_MATRIX_METRIC_COUNT).map(|index| {
        let width = ui_px(92.0 + (index % 4) as f32 * 12.0);
        TableColumn::new(format!("metric_{index:02}"), format!("Metric {index:02}"))
            .with_width(width)
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(180.0))
    }));
    columns.push(
        TableColumn::new("status", "Status")
            .with_hideable(false)
            .with_width(ui_px(148.0))
            .with_min_width(ui_px(112.0))
            .with_max_width(ui_px(220.0)),
    );
    columns
}

fn release_matrix_column_order() -> Vec<String> {
    let mut order = Vec::with_capacity(RELEASE_MATRIX_METRIC_COUNT + 2);
    order.push("name".to_owned());
    order.extend((0..RELEASE_MATRIX_METRIC_COUNT).map(|index| format!("metric_{index:02}")));
    order.push("status".to_owned());
    order
}

fn dependency_tree_table_columns() -> [TableColumn; 7] {
    [
        TableColumn::new("name", "Package")
            .with_width(ui_px(220.0))
            .with_min_width(ui_px(172.0))
            .with_max_width(ui_px(320.0)),
        TableColumn::new("kind", "Kind")
            .with_width(ui_px(120.0))
            .with_min_width(ui_px(96.0))
            .with_max_width(ui_px(180.0)),
        TableColumn::new("owner", "Owner")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(200.0)),
        TableColumn::new("risk", "Risk")
            .with_width(ui_px(112.0))
            .with_min_width(ui_px(88.0))
            .with_max_width(ui_px(160.0)),
        TableColumn::new("change", "Change")
            .with_width(ui_px(148.0))
            .with_min_width(ui_px(112.0))
            .with_max_width(ui_px(220.0)),
        TableColumn::new("score", "Score")
            .with_width(ui_px(92.0))
            .with_min_width(ui_px(72.0))
            .with_max_width(ui_px(132.0)),
        TableColumn::new("status", "Status")
            .with_width(ui_px(132.0))
            .with_min_width(ui_px(104.0))
            .with_max_width(ui_px(188.0)),
    ]
}

fn dependency_tree_column_order() -> [&'static str; 7] {
    ["name", "kind", "owner", "risk", "change", "score", "status"]
}

fn dependency_tree_rows() -> Vec<TableRow> {
    vec![
        dependency_tree_row(
            "dependency-workspace",
            "open-gpui",
            "workspace",
            "Foundation",
            "medium",
            "tree table slice",
            91,
            "active",
        )
        .with_children([
            dependency_tree_row(
                "dependency-ui",
                "crates/ui_components",
                "crate",
                "Components",
                "high",
                "row interactions",
                88,
                "review",
            )
            .with_children([
                dependency_tree_row(
                    "dependency-ui-table",
                    "table/mod.rs",
                    "module",
                    "Components",
                    "high",
                    "tree affordance",
                    94,
                    "active",
                ),
                dependency_tree_row(
                    "dependency-ui-tree",
                    "tree.rs",
                    "module",
                    "Components",
                    "medium",
                    "navigation parity",
                    77,
                    "stable",
                ),
            ]),
            dependency_tree_row(
                "dependency-core",
                "crates/ui_core",
                "crate",
                "Foundation",
                "medium",
                "row model",
                84,
                "active",
            )
            .with_child(dependency_tree_row(
                "dependency-core-table",
                "table/mod.rs",
                "module",
                "Foundation",
                "medium",
                "source hierarchy",
                90,
                "ready",
            )),
            dependency_tree_row(
                "dependency-docs",
                "docs/ui",
                "docs",
                "Product",
                "low",
                "contract update",
                71,
                "queued",
            ),
        ]),
    ]
}

pub(crate) fn server_tree_table_state(loaded: bool) -> TableState {
    TableState::new(server_tree_rows(loaded))
        .with_columns(dependency_tree_table_columns())
        .with_column_order(dependency_tree_column_order())
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_column_sizing(
            TableColumnSizing::new()
                .with_width("name", ui_px(220.0))
                .with_width("kind", ui_px(120.0))
                .with_width("owner", ui_px(132.0))
                .with_width("risk", ui_px(112.0))
                .with_width("change", ui_px(148.0))
                .with_width("score", ui_px(92.0))
                .with_width("status", ui_px(132.0)),
        )
        .with_manual_expansion()
        .with_selected_rows([TableSourceRowIdentity::unique("server-workspace")])
        .with_pagination(TablePagination::disabled())
}

fn server_tree_rows(loaded: bool) -> Vec<TableRow> {
    let workspace_status = if loaded { "loaded" } else { "unloaded" };
    let mut workspace = dependency_tree_row(
        "server-workspace",
        "remote workspace",
        "workspace",
        "Platform",
        "medium",
        "server children",
        86,
        workspace_status,
    )
    .with_expandable(true);

    if loaded {
        workspace = workspace.with_children([
            dependency_tree_row(
                "server-api",
                "api gateway",
                "service",
                "Platform",
                "medium",
                "loaded child",
                82,
                "ready",
            ),
            dependency_tree_row(
                "server-workers",
                "worker queue",
                "service",
                "Runtime",
                "high",
                "manual expansion",
                79,
                "active",
            ),
        ]);
    }

    vec![
        workspace,
        dependency_tree_row(
            "server-cache",
            "cache prefetch",
            "remote",
            "Runtime",
            "medium",
            "async children",
            74,
            "loading",
        )
        .with_children_loading("Loading cached modules"),
        dependency_tree_row(
            "server-failed",
            "failed shard",
            "remote",
            "Platform",
            "high",
            "retry children",
            61,
            "retry",
        )
        .with_children_load_failed("Gateway timeout"),
    ]
}

#[allow(clippy::too_many_arguments)]
fn dependency_tree_row(
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    owner: &'static str,
    risk: &'static str,
    change: &'static str,
    score: usize,
    status: &'static str,
) -> TableRow {
    TableRow::new(id)
        .with_cell("name", name)
        .with_cell("kind", kind)
        .with_cell("owner", owner)
        .with_cell("risk", risk)
        .with_cell("change", change)
        .with_cell("score", score)
        .with_cell("status", status)
}

fn release_queue_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let score = 10_000_usize.saturating_sub(index);

    TableRow::new(format!("release-queue-row-{index:04}"))
        .with_cell("name", format!("Release #{index:04}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 7) % statuses.len()])
        .with_cell("score", score)
}

fn release_resize_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let statuses = ["Queued", "Running", "Ready", "Held"];
    let score = 500_usize.saturating_sub(index % 500);

    TableRow::new(format!("release-resize-row-{index:03}"))
        .with_cell("name", format!("Resize candidate #{index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 5) % statuses.len()])
        .with_cell("score", score)
}

fn editable_release_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 320_usize.saturating_sub(index % 320);

    TableRow::new(format!("editable-release-row-{index:03}"))
        .with_cell("name", format!("Editable release {index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 4) % statuses.len()])
        .with_cell("score", score)
}

fn toggle_release_row(index: usize) -> TableRow {
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 280_usize.saturating_sub(index % 280);

    TableRow::new(format!("toggle-release-row-{index:03}"))
        .with_cell("name", format!("Toggle release {index:03}"))
        .with_cell("enabled", index.is_multiple_of(2))
        .with_cell("status", statuses[(index / 4) % statuses.len()])
        .with_cell("score", score)
}

fn select_release_row(index: usize) -> TableRow {
    let statuses = ["ready", "blocked"];
    let teams = ["UI", "Runtime", "Platform", "QA"];
    let score = 260_usize.saturating_sub(index % 260);

    TableRow::new(format!("select-release-row-{index:03}"))
        .with_cell("name", format!("Select release {index:03}"))
        .with_cell("status", statuses[index % statuses.len()])
        .with_cell("team", teams[(index / 3) % teams.len()])
        .with_cell("score", score)
}

fn multiline_release_row(index: usize) -> TableRow {
    let statuses = ["Draft", "Review", "Ready", "Held"];
    let score = 240_usize.saturating_sub(index % 240);

    TableRow::new(format!("multiline-release-row-{index:03}"))
        .with_cell("name", format!("Release note {index:03}"))
        .with_cell(
            "notes",
            format!("User-visible summary {index:03}\nRollback: pending"),
        )
        .with_cell("status", statuses[(index / 3) % statuses.len()])
        .with_cell("score", score)
}

fn filter_board_row(index: usize) -> TableRow {
    let team = if index.is_multiple_of(3) {
        "UI"
    } else if index.is_multiple_of(2) {
        "Platform"
    } else {
        "Runtime"
    };
    let statuses = ["Todo", "Doing", "Review", "Done"];

    TableRow::new(format!("filter-board-row-{index:03}"))
        .with_cell("name", format!("Board item {index:03}"))
        .with_cell("team", team)
        .with_cell("status", statuses[index % statuses.len()])
        .with_cell("score", index)
}

fn grouped_release_row(index: usize) -> TableRow {
    let teams = ["UI", "Runtime", "Platform", "Docs", "QA"];
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let score = 500_usize.saturating_sub(index);

    TableRow::new(format!("grouped-release-row-{index:03}"))
        .with_cell("name", format!("Release rollup {index:03}"))
        .with_cell("team", teams[index % teams.len()])
        .with_cell("status", statuses[(index / 9) % statuses.len()])
        .with_cell("score", score)
}

fn grouped_custom_aggregation_row(index: usize) -> TableRow {
    let status = ["Ready", "Review", "Blocked", "Verify"][index % 4];
    let (team, score) = match index {
        0..=3 => ("UI", index + 1),
        _ => ("Platform", (index - 3) * 10),
    };

    TableRow::new(format!("grouped-custom-aggregation-row-{index:03}"))
        .with_cell("name", format!("Custom aggregate {index:03}"))
        .with_cell("team", team)
        .with_cell("status", status)
        .with_cell("score", score)
}

fn release_matrix_row(index: usize) -> TableRow {
    let statuses = ["Ready", "Review", "Build", "Verify", "Blocked"];
    let mut row = TableRow::new(format!("release-matrix-row-{index:03}"))
        .with_cell("name", format!("Train {index:03}"))
        .with_cell("status", statuses[(index / 13) % statuses.len()]);

    for metric in 0..RELEASE_MATRIX_METRIC_COUNT {
        row = row.with_cell(
            format!("metric_{metric:02}"),
            (index + 1) * (metric + 3) % 997,
        );
    }

    row
}

fn row_pinning_row(index: usize) -> TableRow {
    let statuses = ["Queued", "Ready", "Review", "Blocked"];
    let mut row = TableRow::new(format!("row-pinning-row-{index:03}"))
        .with_cell("name", format!("Review lane {index:03}"))
        .with_cell("status", statuses[(index / 4) % statuses.len()]);

    for metric in 0..RELEASE_MATRIX_METRIC_COUNT {
        row = row.with_cell(
            format!("metric_{metric:02}"),
            (index + 11) * (metric + 5) % 991,
        );
    }

    row
}

fn server_paged_rows() -> Vec<TableRow> {
    let teams = ["UI", "Runtime", "Platform", "Docs"];
    let statuses = ["Queued", "Ready", "Review", "Blocked"];
    let mut rows = Vec::with_capacity(8);

    for index in 16..24 {
        rows.push(
            TableRow::new(format!("server-paged-row-{index:04}"))
                .with_cell("name", format!("Page row {index:04}"))
                .with_cell("team", teams[index % teams.len()])
                .with_cell("status", statuses[(index / 2) % statuses.len()])
                .with_cell("score", 64 - index),
        );
    }

    rows
}
