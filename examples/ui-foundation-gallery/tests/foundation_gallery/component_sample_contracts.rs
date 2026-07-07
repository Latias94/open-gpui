use super::*;

#[test]
fn components_page_tabs_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let tabs = pages::components::tabs_samples(tokens);

    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].id, "overview-tabs");
    assert_eq!(tabs[0].state.selected_value(), Some("overview"));
    assert_eq!(tabs[0].state.focused_value(), Some("overview"));
    assert!(tabs[0].items.iter().any(|item| item.disabled));

    assert_eq!(tabs[1].id, "workspace-tabs");
    assert!(tabs[1].items.len() >= 12);
    assert_eq!(tabs[1].state.selected_value(), Some("profile"));
    assert_eq!(tabs[1].state.focused_value(), Some("profile"));
    assert!(tabs[1].items[3].disabled);
}

#[test]
fn components_page_table_samples_expose_virtualized_row_model_contract() {
    let samples = pages::components::table_samples(ThemeTokens::default());
    let release_queue = table_sample(samples, "release-queue");
    let release_plan = release_queue.behavior_snapshot();
    let release_summary = release_queue.state_summary();

    assert_eq!(release_queue.id, "release-queue");
    assert_eq!(release_queue.state.rows().len(), 10_000);
    assert_eq!(release_plan.row_counts().final_rows(), 10_000);
    assert_eq!(release_summary.core_rows, 10_000);
    assert_eq!(release_summary.final_rows, 10_000);
    assert_eq!(
        release_summary.rendered_rows,
        release_plan.rendered_row_count()
    );
    assert_eq!(
        release_summary.visible_rows,
        release_plan.visible_row_count()
    );
    assert_eq!(
        release_plan.rows()[0].id().as_str(),
        "release-queue-row-0000"
    );
    assert_eq!(release_plan.row_counts().pinned_center_rows(), 10_000);
    assert!(!release_plan.visible_rows().visible_range().is_empty());
    assert!(release_plan.rendered_row_count() <= release_plan.visible_row_count() + 5);
    assert_eq!(release_plan.row_role(), Role::Row);
    assert_eq!(release_plan.column_header_role(), Role::ColumnHeader);
    assert_eq!(release_plan.cell_role(), Role::Cell);

    let filter_board = table_sample(samples, "filter-board");
    let filter_plan = filter_board.behavior_snapshot();
    let filter_summary = filter_board.state_summary();

    assert_eq!(filter_board.id, "filter-board");
    assert_eq!(filter_board.state.rows().len(), 180);
    assert_eq!(filter_plan.row_counts().filtered_rows(), 60);
    assert_eq!(filter_plan.row_counts().final_rows(), 24);
    assert_eq!(filter_summary.filtered_rows, 60);
    assert_eq!(filter_summary.final_rows, 24);
    assert_eq!(filter_summary.selected_rows, 1);
    assert_eq!(filter_summary.facet_columns, 4);
    assert_eq!(filter_summary.manual_facet_columns, 0);
    assert_eq!(filter_summary.status_facet_values, 4);
    assert_eq!(filter_summary.status_facet_total_count, 60);
    assert_eq!(filter_summary.score_facet_min, Some(0));
    assert_eq!(filter_summary.score_facet_max, Some(177));
    assert_eq!(filter_plan.rows()[0].id().as_str(), "filter-board-row-177");
    assert_eq!(filter_plan.row_counts().selected_rows(), 1);
    assert_eq!(filter_plan.aria_column_count(), 4);
    let filter_status_facet = filter_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("filter-board status facet should resolve");
    assert_eq!(filter_status_facet.mode(), TableStageMode::Client);
    assert_eq!(filter_status_facet.row_count(), 60);
    assert_eq!(facet_total_count(filter_status_facet), 60);

    let server_paged = table_sample(samples, "server-paged");
    let server_page_plan = server_paged.behavior_snapshot();
    let server_page_summary = server_paged.state_summary();

    assert_eq!(server_paged.id, "server-paged");
    assert_eq!(server_paged.state.rows().len(), 8);
    assert_eq!(server_page_plan.filtering_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.sorting_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_mode(), TableStageMode::Manual);
    assert_eq!(server_page_plan.pagination_row_count(), Some(64));
    assert_eq!(server_page_plan.pagination_page_count(), Some(8));
    assert_eq!(server_page_summary.core_rows, 8);
    assert_eq!(server_page_summary.filtered_rows, 8);
    assert_eq!(server_page_summary.final_rows, 8);
    assert_eq!(server_page_summary.selected_rows, 1);
    assert!(server_page_summary.manual_filtering);
    assert!(server_page_summary.manual_sorting);
    assert!(server_page_summary.manual_pagination);
    assert_eq!(server_page_summary.pagination_page_index, 2);
    assert_eq!(server_page_summary.pagination_page_size, 8);
    assert_eq!(server_page_summary.pagination_row_count, Some(64));
    assert_eq!(server_page_summary.pagination_page_count, Some(8));
    assert_eq!(server_page_summary.facet_columns, 4);
    assert_eq!(server_page_summary.manual_facet_columns, 2);
    assert_eq!(server_page_summary.status_facet_values, 4);
    assert_eq!(server_page_summary.status_facet_total_count, 64);
    assert_eq!(server_page_summary.score_facet_min, Some(1));
    assert_eq!(server_page_summary.score_facet_max, Some(64));
    let server_status_facet = server_page_plan
        .column_facet(&TableColumnId::new("status"))
        .expect("server-paged status facet should resolve");
    assert_eq!(server_status_facet.mode(), TableStageMode::Manual);
    assert_eq!(server_status_facet.row_count(), 64);
    assert_eq!(facet_total_count(server_status_facet), 64);
    let server_score_range = server_page_plan
        .column_facet(&TableColumnId::new("score"))
        .and_then(|facet| facet.numeric_range())
        .expect("server-paged score facet should resolve");
    assert_eq!(server_score_range.min(), 1.0);
    assert_eq!(server_score_range.max(), 64.0);
    assert_eq!(
        server_page_plan
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        [
            "server-paged-row-0016",
            "server-paged-row-0017",
            "server-paged-row-0018",
            "server-paged-row-0019",
            "server-paged-row-0020",
            "server-paged-row-0021",
            "server-paged-row-0022",
            "server-paged-row-0023",
        ],
        "manual modes should preserve the supplied server page snapshot"
    );
    assert_eq!(server_page_plan.row_counts().selected_rows(), 1);
    assert!(
        server_page_plan
            .rows()
            .iter()
            .any(|row| row.id().as_str() == "server-paged-row-0018" && row.selected())
    );

    let release_resize = table_sample(samples, "release-resize");
    let resize_plan = release_resize.behavior_snapshot();
    let resize_summary = release_resize.state_summary();

    assert_eq!(release_resize.id, "release-resize");
    assert_eq!(release_resize.state.rows().len(), 160);
    assert_eq!(resize_plan.row_counts().final_rows(), 160);
    assert_eq!(resize_summary.core_rows, 160);
    assert_eq!(resize_summary.total_column_width_px, 520);
    assert_eq!(resize_summary.resizable_columns, 3);
    assert_eq!(resize_plan.columns()[0].width(), ui_px(188.0));
    assert_eq!(resize_plan.columns()[1].width(), ui_px(116.0));
    assert_eq!(resize_plan.columns()[2].width(), ui_px(132.0));
    assert_eq!(resize_plan.columns()[3].width(), ui_px(84.0));
    assert!(resize_plan.columns()[0].resizable());
    assert!(resize_plan.columns()[1].resizable());
    assert!(resize_plan.columns()[2].resizable());
    assert!(!resize_plan.columns()[3].resizable());

    let content_fit_release = table_sample(samples, "content-fit-release");
    let content_fit_plan = content_fit_release.behavior_snapshot();
    let content_fit_summary = content_fit_release.state_summary();

    assert_eq!(content_fit_release.id, "content-fit-release");
    assert_eq!(content_fit_release.state.rows().len(), 32);
    assert_eq!(content_fit_summary.core_rows, 32);
    assert_eq!(content_fit_summary.selected_rows, 1);
    assert_eq!(
        content_fit_plan.columns()[0].width_policy(),
        TableColumnWidthPolicy::ContentFit
    );
    assert_eq!(content_fit_plan.columns()[3].width(), ui_px(84.0));

    let toggle_release = table_sample(samples, "toggle-release");
    let toggle_plan = toggle_release.behavior_snapshot();
    let toggle_summary = toggle_release.state_summary();

    assert_eq!(toggle_release.id, "toggle-release");
    assert_eq!(toggle_release.state.rows().len(), 28);
    assert_eq!(toggle_summary.core_rows, 28);
    assert_eq!(toggle_summary.selected_rows, 1);
    assert_eq!(
        toggle_plan.columns()[1].editor(),
        Some(TableCellEditor::Checkbox)
    );
    assert_eq!(
        toggle_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Checkbox)
    );
    assert_eq!(
        toggle_plan.rows()[0]
            .cell(&TableColumnId::new("enabled"))
            .map(|cell| cell.text())
            .as_deref(),
        Some("true")
    );

    let select_release = table_sample(samples, "select-release");
    let select_plan = select_release.behavior_snapshot();
    let select_summary = select_release.state_summary();

    assert_eq!(select_release.id, "select-release");
    assert_eq!(select_release.state.rows().len(), 28);
    assert_eq!(select_summary.core_rows, 28);
    assert_eq!(select_summary.selected_rows, 1);
    assert_eq!(
        select_plan.columns()[1].editor(),
        Some(TableCellEditor::Select)
    );
    assert_eq!(
        select_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::Select)
    );
    assert_eq!(select_plan.rows()[0].cells()[1].text(), "Ready");
    assert_eq!(select_plan.rows()[0].cells()[1].select_options().len(), 2);

    let multiline_release = table_sample(samples, "multiline-release");
    let multiline_plan = multiline_release.behavior_snapshot();
    let multiline_summary = multiline_release.state_summary();

    assert_eq!(multiline_release.id, "multiline-release");
    assert_eq!(multiline_release.state.rows().len(), 24);
    assert_eq!(multiline_summary.core_rows, 24);
    assert_eq!(multiline_summary.selected_rows, 1);
    assert_eq!(multiline_release.row_height, ui_px(82.0));
    assert_eq!(
        multiline_plan.columns()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert_eq!(
        multiline_plan.rows()[0].cells()[1].editor(),
        Some(TableCellEditor::MultilineText { rows: 3 })
    );
    assert_eq!(
        multiline_plan.rows()[0]
            .cell(&TableColumnId::new("notes"))
            .map(|cell| cell.text())
            .as_deref(),
        Some("User-visible summary 000\nRollback: pending")
    );

    let grouped_release = table_sample(samples, "release-rollup");
    let grouped_plan = grouped_release.behavior_snapshot();
    let grouped_summary = grouped_release.state_summary();

    assert_eq!(grouped_release.id, "release-rollup");
    assert_eq!(grouped_release.state.rows().len(), 320);
    assert_eq!(grouped_release.state.grouping()[0].as_str(), "team");
    assert_eq!(grouped_release.state.aggregations().len(), 2);
    assert!(matches!(
        grouped_release.state.expansion(),
        TableExpansionState::Rows(rows) if rows.len() == 2
    ));
    assert_eq!(
        grouped_release.state.column_pinning().left()[0].as_str(),
        "name"
    );
    assert_eq!(
        grouped_release.state.column_pinning().right()[0].as_str(),
        "status"
    );
    assert_eq!(grouped_summary.core_rows, 320);
    assert_eq!(grouped_summary.grouping_columns, 1);
    assert_eq!(grouped_summary.aggregation_count, 2);
    assert_eq!(grouped_summary.expanded_group_inputs, 2);
    assert!(!grouped_summary.all_rows_expanded);
    assert_eq!(grouped_summary.pinned_left_columns, 1);
    assert_eq!(grouped_summary.pinned_center_columns, 2);
    assert_eq!(grouped_summary.pinned_right_columns, 1);
    assert_eq!(grouped_summary.pinned_left_width_px, 188);
    assert_eq!(grouped_summary.pinned_center_width_px, 400);
    assert_eq!(grouped_summary.pinned_right_width_px, 164);
    assert_eq!(grouped_summary.total_column_width_px, 752);
    assert!(grouped_plan.uses_split_pinned_columns());
    assert!(grouped_summary.group_rows >= 5);
    assert!(grouped_summary.leaf_rows > 0);
    assert!(grouped_summary.expanded_rows < grouped_summary.grouped_rows);

    let ui_group = grouped_plan
        .row(&TableRowId::new("group:team=UI"))
        .expect("expanded UI group should be visible and addressable");
    assert!(ui_group.is_group());
    assert_eq!(
        ui_group
            .cell(&TableColumnId::new("name"))
            .expect("group count aggregate should be present")
            .text(),
        "64"
    );
    assert!(
        !ui_group
            .cell(&TableColumnId::new("score"))
            .expect("group score aggregate should be present")
            .text()
            .is_empty()
    );
    assert!(
        grouped_plan
            .rows()
            .iter()
            .any(|row| row.id().as_str() == "grouped-release-row-000" && row.is_leaf())
    );
    assert!(
        grouped_plan
            .rows()
            .iter()
            .all(|row| row.id().as_str() != "grouped-release-row-001"),
        "Runtime leaf row should stay hidden because that group starts collapsed"
    );
    assert_eq!(
        grouped_plan
            .columns()
            .iter()
            .map(|column| (column.region(), column.id().as_str()))
            .collect::<Vec<_>>(),
        [
            (TableColumnRegion::Left, "name"),
            (TableColumnRegion::Center, "team"),
            (TableColumnRegion::Center, "score"),
            (TableColumnRegion::Right, "status"),
        ]
    );

    let custom_grouped = table_sample(samples, "grouped-custom-aggregation");
    let custom_plan = custom_grouped.behavior_snapshot();
    let custom_summary = custom_grouped.state_summary();

    assert_eq!(custom_grouped.id, "grouped-custom-aggregation");
    assert_eq!(custom_grouped.state.rows().len(), 8);
    assert_eq!(custom_grouped.state.grouping()[0].as_str(), "team");
    assert_eq!(custom_grouped.state.aggregations().len(), 2);
    assert_eq!(custom_grouped.state.aggregation_fn_count(), 1);
    assert!(custom_grouped.state.has_aggregation_fn("score_plus_one"));
    assert_eq!(custom_summary.custom_aggregation_count, 1);
    assert_eq!(custom_plan.aggregation_fn_count(), 1);
    assert_eq!(custom_summary.grouping_columns, 1);
    assert_eq!(custom_summary.aggregation_count, 2);
    assert_eq!(custom_summary.group_rows, 2);
    assert_eq!(custom_summary.leaf_rows, 8);
    assert_eq!(custom_summary.expanded_group_inputs, 2);
    assert_eq!(custom_plan.row_counts().final_rows(), 10);
    let custom_ui_group = custom_plan
        .row(&TableRowId::new("group:team=UI"))
        .expect("expanded UI custom group should be visible and addressable");
    assert_eq!(
        custom_ui_group
            .cell(&TableColumnId::new("name"))
            .expect("custom group count aggregate should be present")
            .text(),
        "4"
    );
    assert_eq!(
        custom_ui_group
            .cell(&TableColumnId::new("score"))
            .expect("custom score aggregate should be present")
            .text(),
        "11"
    );
    assert_eq!(
        custom_plan
            .row(&TableRowId::new("group:team=Platform"))
            .expect("expanded Platform custom group should be visible and addressable")
            .cell(&TableColumnId::new("score"))
            .expect("platform custom score aggregate should be present")
            .text(),
        "101"
    );

    let release_matrix = table_sample(samples, "release-matrix");
    let matrix_plan = release_matrix.behavior_snapshot();
    let matrix_summary = release_matrix.state_summary();

    assert_eq!(release_matrix.id, "release-matrix");
    assert_eq!(release_matrix.state.rows().len(), 480);
    assert_eq!(
        release_matrix.state.sorting()[0].column().as_str(),
        "metric_13"
    );
    assert_eq!(matrix_summary.header_rows, 3);
    assert_eq!(matrix_summary.header_groups, 4);
    assert_eq!(matrix_summary.visible_leaf_columns, 16);
    assert_eq!(matrix_summary.core_rows, 480);
    assert_eq!(matrix_summary.final_rows, 480);
    assert_eq!(matrix_summary.selected_rows, 1);
    assert_eq!(matrix_summary.pinned_left_columns, 1);
    assert_eq!(matrix_summary.pinned_center_columns, 14);
    assert_eq!(matrix_summary.pinned_right_columns, 1);
    assert_eq!(matrix_summary.pinned_left_width_px, 172);
    assert_eq!(matrix_summary.pinned_center_width_px, 1516);
    assert_eq!(matrix_summary.pinned_right_width_px, 148);
    assert_eq!(matrix_summary.total_column_width_px, 1836);
    assert!(matrix_plan.uses_split_pinned_columns());
    assert_eq!(matrix_plan.aria_column_count(), 16);
    assert_eq!(matrix_plan.header_summary().header_rows(), 3);
    assert_eq!(matrix_plan.header_summary().visible_group_headers(), 4);
    assert_eq!(
        matrix_plan
            .columns()
            .iter()
            .filter(|column| column.region() == TableColumnRegion::Center)
            .map(|column| column.id().as_str())
            .collect::<Vec<_>>(),
        [
            "metric_00",
            "metric_01",
            "metric_02",
            "metric_03",
            "metric_04",
            "metric_05",
            "metric_06",
            "metric_07",
            "metric_08",
            "metric_09",
            "metric_10",
            "metric_11",
            "metric_12",
            "metric_13",
        ]
    );
    let row_pinning = table_sample(samples, "row-pinning");
    let row_pinning_plan = row_pinning.behavior_snapshot();
    let row_pinning_summary = row_pinning.state_summary();

    assert_eq!(row_pinning.id, "row-pinning");
    assert_eq!(row_pinning.state.rows().len(), 96);
    assert_eq!(row_pinning.state.pagination().page_index(), 2);
    assert_eq!(row_pinning.state.pagination().page_size(), 12);
    assert_eq!(row_pinning_summary.core_rows, 96);
    assert_eq!(row_pinning_summary.final_rows, 14);
    assert_eq!(row_pinning_summary.pinned_top_rows, 1);
    assert_eq!(row_pinning_summary.pinned_center_rows, 11);
    assert_eq!(row_pinning_summary.pinned_bottom_rows, 2);
    assert!(!row_pinning_summary.row_pinning_page_only);
    assert_eq!(
        row_pinning_summary.visible_rows,
        row_pinning_plan.visible_row_count()
    );
    assert_eq!(
        row_pinning_summary.rendered_rows,
        row_pinning_plan.rendered_row_count()
    );
    assert_eq!(row_pinning_plan.row_counts().pinned_center_rows(), 11);
    assert_eq!(row_pinning_plan.aria_row_count(), 15);
    assert!(row_pinning_plan.uses_split_pinned_columns());
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Top)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [("row-pinning-row-003", TableRowRegion::Top, 0)]
    );
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Center)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-pinning-row-024", TableRowRegion::Center, 0),
            ("row-pinning-row-025", TableRowRegion::Center, 1),
            ("row-pinning-row-026", TableRowRegion::Center, 2),
            ("row-pinning-row-027", TableRowRegion::Center, 3),
            ("row-pinning-row-028", TableRowRegion::Center, 4),
            ("row-pinning-row-029", TableRowRegion::Center, 5),
            ("row-pinning-row-031", TableRowRegion::Center, 6),
            ("row-pinning-row-032", TableRowRegion::Center, 7),
            ("row-pinning-row-033", TableRowRegion::Center, 8),
            ("row-pinning-row-034", TableRowRegion::Center, 9),
            ("row-pinning-row-035", TableRowRegion::Center, 10),
        ]
    );
    assert_eq!(
        row_pinning_plan
            .rows_for_region(TableRowRegion::Bottom)
            .map(|row| (row.id().as_str(), row.region(), row.region_index()))
            .collect::<Vec<_>>(),
        [
            ("row-pinning-row-030", TableRowRegion::Bottom, 0),
            ("row-pinning-row-070", TableRowRegion::Bottom, 1),
        ]
    );

    let dependency_tree = table_sample(samples, "dependency-tree");
    let tree_plan = dependency_tree.behavior_snapshot();
    let tree_summary = dependency_tree.state_summary();

    assert_eq!(dependency_tree.state.rows().len(), 1);
    assert_eq!(tree_summary.core_rows, 7);
    assert_eq!(tree_summary.final_rows, 4);
    assert_eq!(tree_summary.tree_rows, 4);
    assert_eq!(tree_summary.tree_branch_rows, 3);
    assert_eq!(tree_summary.tree_depth, 1);
    assert_eq!(tree_summary.expanded_tree_inputs, 1);
    assert_eq!(tree_summary.pinned_left_columns, 1);
    assert_eq!(tree_summary.pinned_center_columns, 5);
    assert_eq!(tree_summary.pinned_right_columns, 1);
    assert_eq!(tree_summary.pinned_left_width_px, 220);
    assert_eq!(tree_summary.pinned_center_width_px, 604);
    assert_eq!(tree_summary.pinned_right_width_px, 132);
    assert_eq!(tree_summary.total_column_width_px, 956);
    assert!(tree_plan.uses_split_pinned_columns());
    assert_eq!(tree_plan.aria_column_count(), 7);
    assert_eq!(
        tree_plan
            .rows()
            .iter()
            .map(|row| (
                row.id().as_str(),
                row.depth(),
                row.tree_expanded(),
                row.is_tree_branch()
            ))
            .collect::<Vec<_>>(),
        [
            ("dependency-workspace", 0, Some(true), true),
            ("dependency-ui", 1, Some(false), true),
            ("dependency-core", 1, Some(false), true),
            ("dependency-docs", 1, None, false),
        ]
    );

    let server_tree = table_sample(samples, "server-tree");
    let server_plan = server_tree.behavior_snapshot();
    let server_summary = server_tree.state_summary();

    assert_eq!(server_tree.state.rows().len(), 3);
    assert_eq!(
        server_tree.state.expansion_mode(),
        TableExpansionMode::Manual
    );
    assert_eq!(server_summary.core_rows, 3);
    assert_eq!(server_summary.final_rows, 3);
    assert_eq!(server_summary.tree_rows, 3);
    assert_eq!(server_summary.tree_branch_rows, 3);
    assert_eq!(server_summary.tree_depth, 0);
    assert_eq!(server_summary.unloaded_tree_branches, 1);
    assert_eq!(server_summary.loading_tree_rows, 1);
    assert_eq!(server_summary.failed_tree_rows, 1);
    assert!(server_summary.manual_expansion);
    assert_eq!(server_summary.expanded_tree_inputs, 0);
    assert_eq!(server_summary.pinned_left_columns, 1);
    assert_eq!(server_summary.pinned_center_columns, 5);
    assert_eq!(server_summary.pinned_right_columns, 1);
    assert_eq!(server_summary.total_column_width_px, 956);
    assert!(server_plan.uses_split_pinned_columns());
    assert_eq!(server_plan.aria_column_count(), 7);
    assert_eq!(server_plan.aria_row_count(), 4);
    assert_eq!(
        server_plan
            .rows()
            .iter()
            .map(|row| row.id().as_str())
            .collect::<Vec<_>>(),
        ["server-workspace", "server-cache", "server-failed"]
    );

    let server_workspace = server_plan
        .row(&TableRowId::new("server-workspace"))
        .expect("server workspace row should resolve");
    let server_cache = server_plan
        .row(&TableRowId::new("server-cache"))
        .expect("server cache row should resolve");
    let server_failed = server_plan
        .row(&TableRowId::new("server-failed"))
        .expect("server failed row should resolve");

    assert!(server_workspace.is_tree_branch());
    assert_eq!(server_workspace.loaded_child_count(), 0);
    assert_eq!(
        server_workspace.children_load_state(),
        Some(&TableRowChildrenLoadState::Idle)
    );
    assert_eq!(server_workspace.tree_expanded(), Some(false));
    assert!(server_cache.is_tree_branch());
    assert_eq!(server_cache.loaded_child_count(), 0);
    assert_eq!(
        server_cache
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Loading cached modules")
    );
    assert!(
        server_cache
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_loading)
    );
    assert_eq!(server_cache.tree_expanded(), Some(false));
    assert!(server_failed.is_tree_branch());
    assert_eq!(server_failed.loaded_child_count(), 0);
    assert_eq!(
        server_failed
            .children_load_state()
            .and_then(TableRowChildrenLoadState::message),
        Some("Gateway timeout")
    );
    assert!(
        server_failed
            .children_load_state()
            .is_some_and(TableRowChildrenLoadState::is_failed)
    );
    assert_eq!(server_failed.tree_expanded(), Some(false));
}

#[test]
fn components_page_sidebar_samples_expose_navigation_contract() {
    let samples = pages::components::sidebar_samples(ThemeTokens::default());
    let workspace = &samples[0].state;
    let icon = &samples[1].state;
    let long = &samples[2].state;

    assert_eq!(workspace.role(), Role::Navigation);
    assert_eq!(workspace.selected_value(), Some("projects"));
    assert_eq!(workspace.focused_value(), Some("projects"));
    assert_eq!(
        workspace.navigation_target("down").map(|item| item.value()),
        Some("inbox")
    );
    assert_eq!(
        workspace
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
    assert!(workspace.items().iter().any(|item| item.disabled()));

    assert_eq!(
        icon.metrics().resolved_width(),
        icon.metrics().collapsed_width()
    );
    assert!(icon.icon_collapsed());
    assert!(icon.items().iter().all(|item| !item.label().is_empty()));

    assert_eq!(long.side().as_str(), "right");
    assert_eq!(long.focused_value(), Some("quality"));
    assert_eq!(
        long.navigation_target("down").map(|item| item.value()),
        Some("alerts")
    );
    assert!(long.scrollable());
}

#[test]
fn components_page_toolbar_samples_expose_roving_focus_contract() {
    let tokens = ThemeTokens::default();
    let samples = pages::components::toolbar_samples(tokens);
    let editor = &samples[0].state;
    let inspector = &samples[1].state;

    assert_eq!(editor.role(), Role::Toolbar);
    assert_eq!(editor.focused_value(), Some("bold"));
    assert_eq!(
        editor.navigation_target("right").map(|item| item.value()),
        Some("italic")
    );
    assert_eq!(
        editor
            .activation_for_key("space")
            .map(|selection| selection.value().to_owned()),
        Some("bold".to_string())
    );
    assert_eq!(editor.items()[2].role(), None);
    assert_eq!(editor.items()[3].toggled(), Some(Toggled::True));
    assert_eq!(editor.items()[1].icon_label(), Some("R"));
    assert_eq!(editor.items()[1].shortcut(), Some("Ctrl+Shift+Z"));
    assert_eq!(
        editor.items()[1].disabled_reason_ref(),
        Some("Nothing to redo")
    );
    assert_eq!(editor.items()[1].tooltip(), Some("Redo last edit"));
    assert_eq!(
        editor.items()[1].accessibility_description(),
        Some("Reapplies the most recently undone edit")
    );
    assert_eq!(inspector.orientation(), Orientation::Vertical);
    assert_eq!(
        inspector.navigation_target("down").map(|item| item.value()),
        Some("refresh")
    );
}

#[test]
fn components_page_choice_samples_expose_listbox_and_select_contracts() {
    let tokens = ThemeTokens::default();
    let listboxes = pages::components::listbox_samples(tokens);
    let selects = pages::components::select_samples(tokens);
    let assignee = &listboxes[0].state;
    let empty = &listboxes[1].state;
    let priority = &selects[0].state;
    let status = &selects[1].state;
    let disabled = &selects[2].state;

    assert_eq!(assignee.role(), Role::ListBox);
    assert_eq!(
        assignee
            .navigation_target("down")
            .map(|option| option.value()),
        Some("owen")
    );
    assert_eq!(
        assignee
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("maya".to_string())
    );
    assert_eq!(
        assignee.typeahead_target("no").map(|option| option.value()),
        Some("nora")
    );
    assert!(assignee.options().iter().any(|option| !option.focusable()));

    assert!(empty.empty());
    assert_eq!(empty.active_value(), None);

    assert_eq!(priority.open_mode(), SelectOpenMode::Controlled);
    assert!(priority.open());
    assert_eq!(priority.selected_value(), Some("critical"));
    assert_eq!(priority.active_value(), Some("normal"));
    assert_ne!(priority.selected_value(), priority.active_value());
    assert_eq!(priority.trigger_label(), "Critical");
    assert_eq!(
        priority.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );
    assert_eq!(
        priority.outside_press_policy(),
        OutsidePressPolicy::DismissAndConsume
    );
    assert_eq!(
        priority.initial_focus_intent(),
        &InitialFocusIntent::FirstFocusable
    );
    assert_eq!(
        priority.focus_restore_intent(),
        &FocusRestoreIntent::Trigger
    );
    assert_eq!(priority.listbox().role(), Role::ListBox);
    assert!(priority.scrollable_content());
    assert!(priority.scroll_area().scrolls_y());

    assert_eq!(status.open_mode(), SelectOpenMode::Uncontrolled);
    assert!(!status.open());
    assert_eq!(status.trigger_label(), "Doing");
    assert_eq!(disabled.trigger_label(), "Unavailable");
    assert!(disabled.disabled());
    assert!(!disabled.overlay().should_render_deferred_layer());
}

#[test]
fn components_page_search_samples_expose_combobox_and_command_contracts() {
    let tokens = ThemeTokens::default();
    let comboboxes = pages::components::combobox_samples(tokens);
    let commands = pages::components::command_samples(tokens);

    let framework = &comboboxes[0].state;
    let empty_combo = &comboboxes[1].state;
    let disabled_combo = &comboboxes[2].state;
    let ranked = &commands[0].state;
    let multi = &commands[1].state;
    let virtualized = &commands[2].state;
    let indexed = &commands[3].state;
    let registry = &commands[4].state;
    let provider = &commands[5].state;
    let diagnostics = &commands[6].state;
    let context = &commands[7].state;
    let keymap = &commands[8].state;

    assert_eq!(framework.open_mode(), ComboboxOpenMode::Controlled);
    assert!(framework.open());
    assert_eq!(framework.input_role(), Role::EditableComboBox);
    assert_eq!(framework.content_role(), Role::ListBox);
    assert_eq!(framework.total_option_count(), 5);
    assert_eq!(framework.filtered_option_count(), 3);
    assert_eq!(framework.selected_value(), Some("solid"));
    assert_eq!(framework.listbox().selected_value(), None);
    assert_eq!(framework.active_value(), Some("react"));
    assert_ne!(framework.selected_value(), framework.active_value());
    assert_eq!(framework.listbox().typeahead_query(), Some("re"));
    assert_eq!(
        framework.overlay().policy().kind(),
        OverlayLayerKind::NonModalDismissible
    );

    assert!(empty_combo.empty());
    assert_eq!(empty_combo.selected_value(), None);
    assert!(empty_combo.listbox().empty());
    assert!(disabled_combo.disabled());
    assert!(!disabled_combo.open());
    assert!(!disabled_combo.input().editable());

    assert_eq!(ranked.open_mode(), CommandOpenMode::Controlled);
    assert!(ranked.open());
    assert_eq!(ranked.input_role(), Role::TextInput);
    assert_eq!(ranked.list_role(), Role::ListBox);
    assert_eq!(ranked.selected_value(), Some("open-file"));
    assert_eq!(ranked.active_value(), Some("open-file"));
    assert_eq!(ranked.filtered_item_count(), 3);
    assert_eq!(ranked.groups().len(), 1);
    assert!(ranked.groups()[0].standalone());
    assert!(ranked.items().iter().any(|item| item.shortcut().is_some()));
    let dialog = ranked.dialog().expect("ranked command is dialog-backed");
    assert!(dialog.open());
    assert_eq!(dialog.overlay().policy().kind(), OverlayLayerKind::Modal);
    assert_eq!(dialog.description(), Some("Run a workspace command"));

    assert_eq!(multi.selection_mode(), CommandSelectionMode::Multiple);
    assert_eq!(multi.selected_values().len(), 2);
    assert_eq!(multi.selected_chips().len(), 2);
    assert_eq!(multi.filtered_item_count(), 1);
    assert_eq!(virtualized.total_item_count(), 10_000);
    assert_eq!(virtualized.filtered_item_count(), 10_000);
    assert_eq!(virtualized.active_value(), Some("command-0000"));
    assert!(indexed.loading().is_some());
    assert_eq!(indexed.loading().unwrap().role(), Role::ProgressIndicator);
    assert_eq!(indexed.index_revision(), Some("workspace-index-v3"));
    assert_eq!(
        indexed.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(
        indexed
            .items()
            .iter()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "recent-open".to_string(),
            "open-file".to_string(),
            "archive".to_string(),
        ]
    );
    assert_eq!(
        commands[4].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert!(commands[4].shortcut_diagnostics.is_empty());
    assert_eq!(registry.index_revision(), Some("gallery-command-center-v1"));
    assert_eq!(
        registry.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(registry.selected_value(), Some("workspace.open"));
    assert_eq!(registry.active_value(), Some("workspace.open"));
    assert_eq!(registry.groups()[0].label(), "Workspace");
    assert_eq!(
        registry.items()[0].shortcut(),
        Some(display_shortcut("ctrl-shift-p").as_str())
    );
    assert_eq!(
        registry.items()[1].shortcut(),
        Some(display_shortcut("ctrl-s").as_str())
    );
    assert!(registry.loop_navigation());
    assert!(registry.group_navigation());

    let provider_status = commands[5]
        .provider_status
        .as_ref()
        .expect("provider sample records provider status");
    assert_eq!(provider_status.provider_id().as_str(), "recent-provider");
    assert_eq!(
        provider_status
            .request_id()
            .map(|request_id| request_id.get()),
        Some(1)
    );
    assert_eq!(provider_status.query(), Some("alpha"));
    assert_eq!(provider_status.state(), CommandProviderState::Ready);
    assert_eq!(provider_status.source_count(), 1);
    assert_eq!(provider_status.command_count(), 2);
    assert!(commands[5].shortcut_diagnostics.is_empty());
    assert!(provider.status_items().is_empty());
    assert_eq!(
        provider.index_revision(),
        Some("gallery-provider-center-v1")
    );
    assert_eq!(provider.index_mode(), CommandIndexSnapshotMode::PreFiltered);
    assert_eq!(provider.query(), "alpha");
    assert_eq!(provider.selected_value(), Some("provider.open.alpha"));
    assert_eq!(provider.active_value(), Some("provider.open.alpha"));
    assert_eq!(provider.filtered_item_count(), 2);
    assert_eq!(provider.groups()[0].label(), "Provider");
    assert_eq!(
        provider
            .group_items(0)
            .map(|item| (item.value().to_owned(), item.label().to_owned()))
            .collect::<Vec<_>>(),
        vec![
            (
                "provider.open.alpha".to_string(),
                "Open alpha from provider".to_string()
            ),
            (
                "provider.reveal.alpha".to_string(),
                "Reveal alpha provider result".to_string()
            ),
        ]
    );
    assert_eq!(
        provider
            .group_items(0)
            .next()
            .and_then(|item| item.shortcut()),
        Some(display_shortcut("ctrl-alt-o").as_str())
    );

    assert_eq!(commands[6].id, "diagnostics-empty");
    assert_eq!(commands[6].dispatched_command_id.as_deref(), None);
    assert_eq!(
        diagnostics.index_revision(),
        Some("gallery-diagnostics-center-v1")
    );
    assert_eq!(diagnostics.query(), "offline");
    assert_eq!(diagnostics.filtered_item_count(), 0);
    assert!(diagnostics.empty());
    assert_eq!(diagnostics.status_error_count(), 1);
    assert_eq!(diagnostics.status_warning_count(), 2);
    assert_eq!(
        diagnostics.status_items()[0].intent(),
        CommandStatusIntent::Error
    );
    assert!(
        diagnostics
            .status_items()
            .iter()
            .any(|item| item.message().contains("diagnostics-provider"))
    );
    assert_eq!(commands[6].shortcut_diagnostics.len(), 2);
    let diagnostics_provider_status = commands[6]
        .provider_status
        .as_ref()
        .expect("diagnostics sample records failed provider status");
    assert_eq!(
        diagnostics_provider_status.provider_id().as_str(),
        "diagnostics-provider"
    );
    assert_eq!(
        diagnostics_provider_status.state(),
        CommandProviderState::Failed
    );

    assert_eq!(commands[7].id, "context-stack");
    assert_eq!(
        commands[7].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert!(commands[7].shortcut_diagnostics.is_empty());
    assert_eq!(context.index_revision(), Some("gallery-context-center-v1"));
    assert_eq!(
        context.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(context.query(), "focused");
    assert_eq!(context.selected_value(), Some("workspace.open"));
    assert_eq!(context.active_value(), Some("workspace.open"));
    assert_eq!(context.filtered_item_count(), 2);
    assert_eq!(
        context
            .items()
            .iter()
            .find(|item| item.value() == "workspace.open")
            .map(|item| (item.label().to_owned(), item.shortcut().map(str::to_owned))),
        Some((
            "Open Focused Editor".to_string(),
            Some(display_shortcut("ctrl-e"))
        ))
    );
    assert_eq!(
        context
            .items()
            .iter()
            .find(|item| item.value() == "editor.format")
            .map(|item| (item.label().to_owned(), item.shortcut().map(str::to_owned))),
        Some((
            "Format Focused Editor".to_string(),
            Some(display_shortcut("ctrl-shift-f"))
        ))
    );

    assert_eq!(commands[8].id, "keymap-resolution");
    assert_eq!(
        commands[8].dispatched_command_id.as_deref(),
        Some("workspace.open")
    );
    assert_eq!(
        keymap.index_revision(),
        Some("gallery-keymap-resolution-center-v1")
    );
    assert_eq!(
        keymap.index_mode(),
        CommandIndexSnapshotMode::PreRankedFilter
    );
    assert_eq!(keymap.query(), "keymap");
    assert_eq!(keymap.filtered_item_count(), 2);
    assert_eq!(
        keymap
            .items()
            .iter()
            .map(|item| (item.value().to_owned(), item.shortcut().map(str::to_owned)))
            .collect::<Vec<_>>(),
        vec![
            (
                "workspace.open".to_string(),
                Some(display_shortcut("ctrl-k ctrl-o"))
            ),
            (
                "workspace.save".to_string(),
                Some(display_shortcut("ctrl-s"))
            ),
        ]
    );
    assert_eq!(commands[8].shortcut_diagnostics.len(), 1);
    assert_eq!(keymap.status_warning_count(), 1);
    assert_eq!(commands[8].keymap_resolutions.len(), 5);
    assert!(commands[8].keymap_resolutions[0].is_pending());
    assert_eq!(
        commands[8].keymap_resolutions[1]
            .primary_dispatchable_command()
            .map(|command| command.command_id()),
        Some("workspace.open")
    );
    assert_eq!(
        commands[8].keymap_resolutions[2]
            .primary_command()
            .and_then(|command| command.state().reason_ref()),
        Some("Workspace is read-only")
    );
    assert!(
        commands[8].keymap_resolutions[3]
            .primary_command()
            .is_some_and(|command| command.state().is_hidden())
    );
    assert!(
        commands[8].keymap_resolutions[4]
            .primary_command()
            .is_some_and(|command| command.state().is_missing_command())
    );

    let shortcut_inspector = commands[8]
        .shortcut_inspector
        .as_ref()
        .expect("keymap sample should expose shortcut inspector state");
    assert_eq!(shortcut_inspector.query(), "keymap");
    assert_eq!(shortcut_inspector.input_label(), "ctrl-k ctrl-o");
    assert_eq!(
        shortcut_inspector.primary_dispatchable_command_id(),
        Some("workspace.open")
    );
    assert_eq!(shortcut_inspector.matched_commands().len(), 1);

    let keybinding_editor = commands[8]
        .keybinding_editor
        .as_ref()
        .expect("keymap sample should expose keybinding editor state");
    assert_eq!(
        keybinding_editor.mode(),
        CommandKeyBindingEditorFilterMode::ConflictsOnly
    );
    assert_eq!(keybinding_editor.total_binding_count(), 2);
    assert_eq!(keybinding_editor.filtered_binding_count(), 2);
    assert_eq!(keybinding_editor.conflicts().len(), 1);
    assert_eq!(keybinding_editor.diagnostics().len(), 2);

    let keybinding_capture = commands[8]
        .keybinding_capture
        .as_ref()
        .expect("keymap sample should expose captured keybinding input");
    assert_eq!(keybinding_capture.input_label(), Some("ctrl-k ctrl-s"));

    let keybinding_edit_preview = commands[8]
        .keybinding_edit_preview
        .as_ref()
        .expect("keymap sample should expose keybinding edit preview");
    assert_eq!(
        keybinding_edit_preview.operation(),
        CommandKeyBindingPatchOperation::Replace
    );
    assert_eq!(
        keybinding_edit_preview.outcome(),
        CommandKeyBindingPatchOutcome::Replaced
    );
    assert!(keybinding_edit_preview.editor().conflicts().is_empty());
}

#[test]
fn components_page_samples_keep_explicit_a11y_metadata() {
    use std::collections::BTreeSet;

    let tokens = ThemeTokens::default();
    let icon_buttons = pages::components::icon_button_samples(tokens);
    let labels = pages::components::label_samples(tokens);

    assert!(
        icon_buttons
            .iter()
            .all(|sample| !sample.state.accessible_label().trim().is_empty())
    );
    assert!(
        icon_buttons
            .iter()
            .all(|sample| sample.state.role() == Role::Button)
    );

    let control_ids = labels
        .iter()
        .filter_map(|sample| sample.state.control_id())
        .collect::<Vec<_>>();
    let unique_control_ids = control_ids.iter().copied().collect::<BTreeSet<_>>();

    assert_eq!(
        control_ids,
        vec!["email-input", "terms-checkbox", "disabled-control"]
    );
    assert_eq!(unique_control_ids.len(), control_ids.len());
    assert!(
        labels
            .iter()
            .filter(|sample| sample.state.control_id().is_some())
            .all(|sample| sample.state.associated())
    );
    assert!(
        labels
            .iter()
            .filter(|sample| sample.state.control_id().is_none())
            .all(|sample| !sample.state.associated())
    );
}

#[test]
fn components_page_conformance_gates_reference_core_and_gallery_contracts() {
    let gates = pages::components::COMPONENT_CONFORMANCE_GATES;
    let signals = pages::components::SIGNALS;

    assert!(gates.iter().all(|gate| !gate.title.trim().is_empty()));
    assert!(gates.iter().all(|gate| !gate.summary.trim().is_empty()));
    assert!(
        gates
            .iter()
            .all(|gate| gate.evidence.iter().all(|item| !item.trim().is_empty()))
    );
    assert!(gates.iter().any(|gate| gate.id == "scroll-redraw"));
    assert!(gates.iter().any(|gate| gate.id == "tabs-overflow"));
    assert!(gates.iter().any(|gate| gate.id == "table-virtualization"));
    assert!(gates.iter().any(|gate| gate.id == "tree-renderer"));
    assert!(
        gates
            .iter()
            .any(|gate| gate.id == "virtualized-list-renderer")
    );
    assert!(
        gates
            .iter()
            .any(|gate| gate.id == "state-contract-readouts")
    );
    assert!(signals.contains(&"open_gpui_ui_components::StatusCue"));
    assert!(signals.contains(&"open_gpui_ui_components::StatusCueState"));
    assert!(signals.contains(&"open_gpui_ui_components::EmptyState"));
    assert!(signals.contains(&"open_gpui_ui_components::EmptyStateState"));
    assert!(signals.contains(&"open_gpui_ui_components::Listbox"));
    assert!(signals.contains(&"open_gpui_ui_components::ListboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Select"));
    assert!(signals.contains(&"open_gpui_ui_components::SelectState"));
    assert!(signals.contains(&"open_gpui_ui_components::Combobox"));
    assert!(signals.contains(&"open_gpui_ui_components::ComboboxState"));
    assert!(signals.contains(&"open_gpui_ui_components::Command"));
    assert!(signals.contains(&"open_gpui_ui_components::CommandState"));
    assert!(signals.contains(&"open_gpui_ui_components::Table"));
    assert!(signals.contains(&"open_gpui_ui_core::TableState"));
    assert!(signals.contains(&"open_gpui_ui_core::TableAggregation"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableFacetedFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableGlobalFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterOperator"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterOperatorOptionState"));
    assert!(signals.contains(&"open_gpui_ui_components::TablePredicateFilterState"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilter"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilterChange"));
    assert!(signals.contains(&"open_gpui_ui_components::TableRangeFilterState"));
    assert!(signals.contains(&"open_gpui_ui_core::TableColumnPinning"));
    assert!(signals.contains(&"open_gpui_ui_core::TableColumnRegion"));
    assert!(signals.contains(&"open_gpui_ui_core::TableExpansionState"));
    assert!(signals.contains(&"open_gpui_ui_core::TableRowPinning"));
    assert!(signals.contains(&"open_gpui_ui_core::TableRowPinningPolicy"));
    assert!(signals.contains(&"open_gpui_ui_core::TableRowRegion"));
    assert!(signals.contains(&"open_gpui_ui_core::TableRowRegions"));
    assert!(signals.contains(&"open_gpui_ui_components::Tree"));
    assert!(signals.contains(&"open_gpui_ui_components::TreeState"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedList"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListItemDescriptor"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListBehaviorSnapshot"));
    assert!(signals.contains(&"open_gpui_ui_core::VirtualizerState"));
    assert!(signals.contains(&"open_gpui_ui_components::VirtualizedListState"));
    assert!(signals.contains(&"Role::ListBox"));
    assert!(signals.contains(&"Role::ListBoxOption"));
    assert!(signals.contains(&"Role::EditableComboBox"));
    assert!(signals.contains(&"Role::ProgressIndicator"));
    assert!(signals.contains(&"Role::Image"));
    assert!(signals.contains(&"Role::Label"));
    assert!(signals.contains(&"Role::Table"));
    assert!(signals.contains(&"Role::Row"));
    assert!(signals.contains(&"Role::ColumnHeader"));
    assert!(signals.contains(&"Role::Cell"));
    assert!(signals.contains(&"Role::Tree"));
    assert!(signals.contains(&"Role::TreeItem"));

    let table_gate = gates
        .iter()
        .find(|gate| gate.id == "table-virtualization")
        .unwrap_or_else(|| panic!("expected table conformance gate"));
    assert!(table_gate.evidence.contains(&"TableFacetedFilter"));
    assert!(table_gate.evidence.contains(&"TableGlobalFilter"));
    assert!(table_gate.evidence.contains(&"TablePredicateFilter"));
    assert!(table_gate.evidence.contains(&"TableRangeFilter"));
    assert!(table_gate.evidence.contains(&"TableColumnWidthPolicy"));
    assert!(table_gate.evidence.contains(&"content-fit-release"));
    assert!(table_gate.evidence.contains(&"toggle-release"));
    assert!(table_gate.evidence.contains(&"select-release"));
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_select_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_global_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_predicate_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_faceted_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_range_filter_updates_table_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_content_fit_table_cell_edit_widens_name_column")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_checkbox_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_select_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_multiline_table_cell_updates_sample_rows")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_column_visibility_updates_release_matrix")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_focused_table_scroll_stays_inside_sample")
    );
    assert!(table_gate.evidence.contains(
        &"components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample"
    ));
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_grouped_table_column_reorder_updates_sample")
    );
    assert!(table_gate.evidence.contains(
        &"components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample"
    ));
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample")
    );
    assert!(
        table_gate
            .evidence
            .contains(&"components_gallery_smoke_resizable_table_resize_updates_sample")
    );
    assert!(table_gate.evidence.contains(&"select-release"));
}
