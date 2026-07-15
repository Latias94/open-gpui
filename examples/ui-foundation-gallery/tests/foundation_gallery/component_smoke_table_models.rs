use super::*;

fn table_leaf_header_identity(column_id: impl Into<TableColumnId>) -> TableResolvedHeaderIdentity {
    TableResolvedHeaderIdentity::leaf(column_id)
}

fn table_row_center_scroll_selector(table_id: &str, identity: &TableRowIdentity) -> String {
    TableDebugSelector::row_center_scroll(table_id, identity)
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:release-rollup";

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let plan = sample.behavior_snapshot();
    let later_row_index = plan.visible_row_count() + sample.overscan + 5;
    let first_row_identity = plan.rows()[0].identity().clone();
    let first_row_label = first_row_identity.debug_label();
    let resolved = sample.state.resolve();
    let later_row_identity = resolved.final_model().rows()[later_row_index]
        .identity()
        .clone();
    let later_row_label = later_row_identity.debug_label();
    let first_row_selector = table_row_selector(TABLE_ID, &first_row_identity);
    let later_row_selector = table_row_selector(TABLE_ID, &later_row_identity);
    let body_scroll_selector = TableDebugSelector::body_scroll(TABLE_ID);

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &body_scroll_selector);
    let sample_before = bounds(cx, "gallery:component-table-sample:release-rollup");
    let header_before = bounds(cx, "table:component-table:release-rollup:header-row");
    let body_viewport = bounds(cx, &body_scroll_selector);
    let page_viewport = bounds(cx, "scroll-area:gallery-page-scroll-viewport");
    let scroll_target_top = body_viewport.top().max(page_viewport.top());
    let scroll_target_bottom = body_viewport.bottom().min(page_viewport.bottom());
    let scroll_target = point(
        body_viewport.left() + px(12.0),
        scroll_target_top + (scroll_target_bottom - scroll_target_top) * 0.5,
    );

    assert!(
        cx.debug_bounds(&first_row_selector).is_some(),
        "expected grouped Table row `{first_row_label}` to render in the initial window"
    );
    let first_row_before = bounds(cx, &first_row_selector);
    assert!(
        cx.debug_bounds(&later_row_selector).is_none(),
        "expected grouped Table row `{later_row_label}` to start outside the rendered window"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: scroll_target,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-520.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-rollup");
    let header_after = bounds(cx, "table:component-table:release-rollup:header-row");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected grouped Table viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        header_after.top(),
        header_before.top(),
        "expected grouped Table header to stay fixed while the body scrolls; before={header_before:?} after={header_after:?}"
    );
    if let Some(first_row_after) = cx.debug_bounds(&first_row_selector) {
        assert!(
            first_row_after.top() < first_row_before.top(),
            "expected grouped Table row `{first_row_label}` to move up after internal scroll; before={first_row_before:?} after={first_row_after:?}"
        );
    }
    assert!(
        cx.debug_bounds(&later_row_selector).is_some(),
        "expected grouped Table row `{later_row_label}` to enter the rendered window after internal scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:release-rollup";

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let plan = sample.behavior_snapshot();
    assert!(
        plan.uses_split_pinned_columns(),
        "release-rollup should exercise sticky pinned table lanes"
    );
    let first_rendered_row = plan
        .rows()
        .iter()
        .find(|row| row.is_leaf())
        .unwrap_or(&plan.rows()[0]);
    let first_row_identity = first_rendered_row.identity().clone();
    let name_cell_selector = table_cell_selector(TABLE_ID, &first_row_identity, "name");
    let team_cell_selector = table_cell_selector(TABLE_ID, &first_row_identity, "team");
    let status_cell_selector = table_cell_selector(TABLE_ID, &first_row_identity, "status");
    let team_header_selector = table_header_selector(TABLE_ID, &table_leaf_header_identity("team"));
    let row_center_scroll_selector =
        table_row_center_scroll_selector(TABLE_ID, &first_row_identity);
    let header_center_scroll_selector = TableDebugSelector::header_center_scroll(TABLE_ID, 0);

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &header_center_scroll_selector);

    let sample_before = bounds(cx, "gallery:component-table-sample:release-rollup");
    let left_before = bounds(cx, &name_cell_selector);
    let center_header_before = bounds(cx, &team_header_selector);
    let center_cell_before = bounds(cx, &team_cell_selector);
    let right_before = bounds(cx, &status_cell_selector);
    assert!(
        cx.debug_bounds(&row_center_scroll_selector).is_some(),
        "expected release-rollup body center lane to expose the shared horizontal viewport"
    );
    let center_viewport = bounds(cx, &header_center_scroll_selector);

    assert!(
        cx.debug_bounds(&header_center_scroll_selector).is_some(),
        "expected release-rollup header center lane to expose the shared horizontal viewport"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-rollup");
    let left_after = bounds(cx, &name_cell_selector);
    let center_header_after = bounds(cx, &team_header_selector);
    let center_cell_after = bounds(cx, &team_cell_selector);
    let right_after = bounds(cx, &status_cell_selector);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected sticky pinned Table horizontal wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected right pinned lane to keep its screen-space x position"
    );
    assert!(
        center_header_after.left() < center_header_before.left(),
        "expected shared horizontal handle to move center header left; before={center_header_before:?} after={center_header_after:?}"
    );
    assert!(
        center_cell_after.left() < center_cell_before.left(),
        "expected horizontal body center lane to move left; before={center_cell_before:?} after={center_cell_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_grouped_table_column_reorder_updates_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:release-rollup";
    const SAMPLE: &str = "gallery:component-table-sample:release-rollup";

    let score_header_selector =
        table_header_selector(TABLE_ID, &table_leaf_header_identity("score"));
    let team_header_selector = table_header_selector(TABLE_ID, &table_leaf_header_identity("team"));
    let header_center_scroll_selector = TableDebugSelector::header_center_scroll(TABLE_ID, 0);

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-rollup")
        .expect("release-rollup table sample should exist");
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    let center_viewport = bounds(cx, &header_center_scroll_selector);
    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_before = bounds(cx, SAMPLE);
    let score_before = bounds(cx, &score_header_selector);
    let team_before = bounds(cx, &team_header_selector);
    let change = TableColumnOrderChange::move_before("score", "team", TableColumnRegion::Center);
    cx.update(|_, app| {
        pages::components::record_table_column_order_change(
            "release-rollup",
            &sample.state,
            &change,
            app,
        );
    });
    cx.run_until_parked();
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let score_after = bounds(cx, &score_header_selector);
    let team_after = bounds(cx, &team_header_selector);
    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_changes().to_vec()
    });

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected release-rollup reorder update to keep the sample card anchored"
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "release-rollup");
    assert_eq!(changes[0].column_id, "score");
    assert_eq!(changes[0].target_column_id, "team");
    assert_eq!(changes[0].placement, "before");
    assert_eq!(changes[0].region, "center");
    assert_eq!(
        changes[0].column_order,
        ["name", "score", "team", "status"]
            .iter()
            .map(|column| column.to_string())
            .collect::<Vec<_>>()
    );
    assert!(
        score_after.left() < team_after.left(),
        "expected score to render before team after the reorder; before=({score_before:?}, {team_before:?}) after=({score_after:?}, {team_after:?})"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:release-matrix";

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "release-matrix")
        .expect("release-matrix table sample should exist");
    let plan = sample.behavior_snapshot();
    assert!(
        plan.uses_split_pinned_columns(),
        "release-matrix should exercise sticky pinned table lanes"
    );
    let first_row_identity = plan.rows()[0].identity().clone();
    let far_header = table_header_selector(TABLE_ID, &table_leaf_header_identity("metric_13"));
    let first_metric_header =
        table_header_selector(TABLE_ID, &table_leaf_header_identity("metric_00"));
    let far_cell = table_cell_selector(TABLE_ID, &first_row_identity, "metric_13");
    let name_cell_selector = table_cell_selector(TABLE_ID, &first_row_identity, "name");
    let status_cell_selector = table_cell_selector(TABLE_ID, &first_row_identity, "status");
    let row_center_scroll_selector =
        table_row_center_scroll_selector(TABLE_ID, &first_row_identity);
    let resolved = sample.state.resolve();
    let group_header_identity = |group_id: &str| {
        resolved
            .header_groups()
            .all()
            .flat_map(|row| row.headers())
            .find(|header| {
                header
                    .source_group_path()
                    .is_some_and(|path| path.last().is_some_and(|group| group.as_str() == group_id))
            })
            .unwrap_or_else(|| panic!("expected resolved `{group_id}` table header group"))
            .identity()
            .clone()
    };
    let left_group = table_header_selector(TABLE_ID, &group_header_identity("identity"));
    let metrics_group = table_header_selector(TABLE_ID, &group_header_identity("metrics"));
    let right_group = table_header_selector(TABLE_ID, &group_header_identity("delivery"));
    let header_center_scroll_selector = TableDebugSelector::header_center_scroll(TABLE_ID, 0);

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &header_center_scroll_selector);

    let sample_before = bounds(cx, "gallery:component-table-sample:release-matrix");
    let left_before = bounds(cx, &name_cell_selector);
    let right_before = bounds(cx, &status_cell_selector);
    let left_group_before = bounds(cx, &left_group);
    assert!(
        cx.debug_bounds(&metrics_group).is_some(),
        "expected release-matrix metrics group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&right_group).is_some(),
        "expected release-matrix delivery group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&left_group).is_some(),
        "expected release-matrix identity group header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&first_metric_header).is_some(),
        "expected the initial center column window to mount the first metric"
    );
    assert!(
        cx.debug_bounds(&far_header).is_none(),
        "expected the far metric header to stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&far_cell).is_none(),
        "expected the far metric cell to stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&row_center_scroll_selector).is_some(),
        "expected release-matrix body center lane to expose the shared horizontal viewport"
    );
    let center_viewport = bounds(cx, &header_center_scroll_selector);

    for _ in 0..6 {
        if cx.debug_bounds(&far_header).is_some() && cx.debug_bounds(&far_cell).is_some() {
            break;
        }

        cx.simulate_event(ScrollWheelEvent {
            position: center_viewport.center(),
            delta: ScrollDelta::Pixels(point(px(-360.0), px(0.0))),
            ..Default::default()
        });
        redraw(cx);
    }

    let sample_after = bounds(cx, "gallery:component-table-sample:release-matrix");
    let left_after = bounds(cx, &name_cell_selector);
    let left_group_after = bounds(cx, &left_group);
    let right_after = bounds(cx, &status_cell_selector);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected matrix Table horizontal wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "expected matrix Table left pinned lane to keep its screen-space x position"
    );
    assert_eq!(
        left_group_after.left(),
        left_group_before.left(),
        "expected matrix Table left header group to keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "expected matrix Table right pinned lane to keep its screen-space x position"
    );
    assert!(
        cx.debug_bounds(&far_header).is_some(),
        "expected the far metric header to enter the rendered center window after horizontal scrolling"
    );
    assert!(
        cx.debug_bounds(&metrics_group).is_some(),
        "expected the metrics group header to stay mounted while the center window scrolls"
    );
    assert!(
        cx.debug_bounds(&right_group).is_some(),
        "expected the delivery group header to stay mounted while the center window scrolls"
    );
    assert!(
        cx.debug_bounds(&far_cell).is_some(),
        "expected the far metric cell to enter the rendered center window after horizontal scrolling"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_column_visibility_updates_release_matrix(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:release-matrix";
    const SAMPLE_ID: &str = "release-matrix";
    const SAMPLE: &str = "gallery:component-table-sample:release-matrix";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:release-matrix:root";
    const TRIGGER: &str = "popover:component-table-column-visibility:release-matrix:trigger";
    const CONTENT: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:content";
    const METRIC_ROW: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:column:metric_03";
    const SHOW_ALL: &str =
        "table-column-visibility:component-table-column-visibility:release-matrix:show-all";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let plan = sample.behavior_snapshot();
    assert_eq!(plan.aria_column_count(), 16);
    let first_row_identity = plan.rows()[0].identity().clone();
    let metric_header = table_header_selector(TABLE_ID, &table_leaf_header_identity("metric_03"));
    let metric_cell = table_cell_selector(TABLE_ID, &first_row_identity, "metric_03");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, TRIGGER);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected release-matrix controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(&metric_header).is_some(),
        "expected metric_03 header to render before hiding the column"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_some(),
        "expected metric_03 cell to render before hiding the column"
    );

    click(cx, TRIGGER);
    settle(cx);
    assert!(
        cx.debug_bounds(CONTENT).is_some(),
        "expected the column visibility popover content to open"
    );
    click(cx, METRIC_ROW);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes().to_vec()
    });
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, SAMPLE_ID);
    assert_eq!(changes[0].action, "toggle_column");
    assert_eq!(changes[0].column_ids, vec!["metric_03".to_owned()]);
    assert_eq!(changes[0].next_visible, Some(false));
    assert_eq!(changes[0].visible_columns, 15);
    assert_eq!(changes[0].hidden_columns, 1);
    let metric_hidden = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_override(SAMPLE_ID)
            .and_then(|visibility| visibility.override_for(&TableColumnId::new("metric_03")))
    });
    assert_eq!(metric_hidden, Some(false));
    assert!(
        cx.debug_bounds(&metric_header).is_none(),
        "expected metric_03 header to unmount after hiding the column"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_none(),
        "expected metric_03 cell to unmount after hiding the column"
    );

    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-80.0))),
        ..Default::default()
    });
    redraw(cx);
    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected column-visibility popup wheel input to stay inside the table sample"
    );

    if cx.debug_bounds(SHOW_ALL).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    click(cx, SHOW_ALL);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes().to_vec()
    });
    assert_eq!(changes.len(), 2);
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected show-all visibility change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.action, "show_all");
    assert!(last.column_ids.contains(&"metric_03".to_owned()));
    assert_eq!(last.next_visible, Some(true));
    assert_eq!(last.visible_columns, 16);
    assert_eq!(last.hidden_columns, 0);
    assert!(
        cx.debug_bounds(&metric_header).is_some(),
        "expected metric_03 header to return after show-all"
    );
    assert!(
        cx.debug_bounds(&metric_cell).is_some(),
        "expected metric_03 cell to return after show-all"
    );

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected column-visibility interactions to keep the sample card anchored"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:row-pinning";

    let sample = pages::components::table_samples(ThemeTokens::default())
        .iter()
        .find(|sample| sample.id == "row-pinning")
        .expect("row-pinning table sample should exist");
    let plan = sample.behavior_snapshot();
    assert_eq!(plan.row_counts().pinned_top_rows(), 1);
    assert_eq!(plan.row_counts().pinned_center_rows(), 11);
    assert_eq!(plan.row_counts().pinned_bottom_rows(), 2);
    assert!(
        plan.uses_split_pinned_columns(),
        "row-pinning should combine row-pinned bands with pinned column lanes"
    );

    let top_row_identity = plan
        .rows_for_region(TableRowRegion::Top)
        .next()
        .expect("row-pinning should render a top-pinned row")
        .identity()
        .clone();
    let bottom_row_identity = plan
        .rows_for_region(TableRowRegion::Bottom)
        .nth(1)
        .expect("row-pinning should render two bottom-pinned rows")
        .identity()
        .clone();
    let top_row_selector = table_row_selector(TABLE_ID, &top_row_identity);
    let bottom_row_selector = table_row_selector(TABLE_ID, &bottom_row_identity);
    let top_name_selector = table_cell_selector(TABLE_ID, &top_row_identity, "name");
    let top_body_selector = TableDebugSelector::body_region(TABLE_ID, TableRowRegion::Top);
    let center_body_selector = TableDebugSelector::body_region(TABLE_ID, TableRowRegion::Center);
    let bottom_body_selector = TableDebugSelector::body_region(TABLE_ID, TableRowRegion::Bottom);
    let center_cell_selectors = plan
        .rows_for_region(TableRowRegion::Center)
        .map(|row| table_cell_selector(TABLE_ID, row.identity(), "name"))
        .collect::<Vec<_>>();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, "gallery:component-table-sample:row-pinning");

    assert!(
        cx.debug_bounds(&top_body_selector).is_some(),
        "expected row-pinning top band to render"
    );
    assert!(
        cx.debug_bounds(&center_body_selector).is_some(),
        "expected row-pinning center band to render"
    );
    assert!(
        cx.debug_bounds(&bottom_body_selector).is_some(),
        "expected row-pinning bottom band to render"
    );
    let collect_center_cells = |cx: &mut VisualTestContext| {
        center_cell_selectors
            .iter()
            .enumerate()
            .filter_map(|(index, selector)| {
                cx.debug_bounds(selector)
                    .map(|bounds| (index, selector.clone(), bounds))
            })
            .collect::<Vec<_>>()
    };

    let center_rows_before = collect_center_cells(cx);
    assert!(
        !center_rows_before.is_empty(),
        "expected row-pinning center body to render at least one center row cell"
    );
    let interaction_target = scroll_page_selector_into_view(&shell, cx, &center_rows_before[0].1);

    let sample_before = bounds(cx, "gallery:component-table-sample:row-pinning");
    let top_row_before = bounds(cx, &top_row_selector);
    let bottom_row_before = bounds(cx, &bottom_row_selector);
    let top_name_before = bounds(cx, &top_name_selector);
    let center_rows_before = collect_center_cells(cx);
    cx.simulate_event(ScrollWheelEvent {
        position: interaction_target.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:row-pinning");
    let top_row_after = bounds(cx, &top_row_selector);
    let bottom_row_after = bounds(cx, &bottom_row_selector);
    let center_rows_after = collect_center_cells(cx);
    assert!(
        !center_rows_after.is_empty(),
        "expected row-pinning center body to keep rendering center row cells after scrolling"
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected row-pinning Table wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "top pinned row band should stay fixed while center rows scroll"
    );
    assert_eq!(
        bottom_row_after.top(),
        bottom_row_before.top(),
        "bottom pinned row band should stay fixed while center rows scroll"
    );
    assert_eq!(
        bounds(cx, &top_name_selector).left(),
        top_name_before.left(),
        "left-pinned cells inside pinned rows should stay fixed while center rows scroll"
    );
    let center_window_changed = center_rows_before
        .iter()
        .map(|(index, _, _)| *index)
        .collect::<Vec<_>>()
        != center_rows_after
            .iter()
            .map(|(index, _, _)| *index)
            .collect::<Vec<_>>();
    let center_row_moved = center_rows_before.iter().any(|(_, selector, before)| {
        center_rows_after.iter().any(|(_, after_selector, after)| {
            after_selector == selector && after.top() != before.top()
        })
    });
    assert!(
        center_window_changed || center_row_moved,
        "center rows should move inside the center scroll body; before={center_rows_before:?} after={center_rows_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_table_expands_and_activates(cx: &mut open_gpui::TestAppContext) {
    const TABLE_ID: &str = "component-table:dependency-tree";
    const SAMPLE: &str = "gallery:component-table-sample:dependency-tree";
    let root_toggle_selector = table_source_tree_toggle_selector(TABLE_ID, "dependency-workspace");
    let ui_toggle_selector = table_source_tree_toggle_selector(TABLE_ID, "dependency-ui");
    let child_row_selector = table_source_row_selector(TABLE_ID, "dependency-ui-table");
    let child_cell_selector = table_source_cell_selector(TABLE_ID, "dependency-ui-table", "name");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, &ui_toggle_selector);

    assert!(
        cx.debug_bounds(&child_row_selector).is_none(),
        "expected dependency-ui children to start collapsed"
    );
    let root_toggle = bounds(cx, &root_toggle_selector);
    let ui_toggle = bounds(cx, &ui_toggle_selector);
    assert!(
        ui_toggle.left() > root_toggle.left(),
        "expected nested tree table toggle to be indented; root={root_toggle:?} ui={ui_toggle:?}"
    );

    click(cx, &ui_toggle_selector);
    settle(cx);
    let toggles = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles()
            .iter()
            .map(|toggle| {
                (
                    toggle.sample_id.clone(),
                    toggle
                        .source_row_id()
                        .map(|row_id| row_id.as_str().to_owned()),
                    toggle.expanded,
                    toggle.depth,
                )
            })
            .collect::<Vec<_>>()
    });
    assert!(
        cx.debug_bounds(&child_row_selector).is_some(),
        "expected dependency-ui child row to render after expansion; toggles={toggles:?}"
    );
    assert_eq!(
        toggles,
        vec![(
            "dependency-tree".to_owned(),
            Some("dependency-ui".to_owned()),
            true,
            1
        )]
    );
    let activations_after_toggle =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert!(
        activations_after_toggle.is_empty(),
        "expected tree disclosure clicks to avoid row activation"
    );

    click(cx, &child_row_selector);
    assert!(
        cx.debug_selector_is_focused(&child_row_selector),
        "expected clicking a tree table row to focus it for keyboard activation; focused={:?} child={:?}",
        cx.focused_debug_selector(),
        bounds(cx, &child_cell_selector)
    );
    let click_activations =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert_eq!(click_activations.len(), 1);
    assert_eq!(click_activations[0].sample_id, "dependency-tree");
    assert_eq!(
        click_activations[0]
            .source_row_id()
            .map(|row_id| row_id.as_str()),
        Some("dependency-ui-table")
    );
    assert_eq!(click_activations[0].kind, "click");
    assert_eq!(click_activations[0].depth, 2);
    assert!(!click_activations[0].tree_branch);
    assert_eq!(click_activations[0].tree_expanded, None);

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let activations = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.row_activations().to_vec()
    });
    assert_eq!(activations.len(), 2);
    assert_eq!(activations[1].sample_id, "dependency-tree");
    assert_eq!(
        activations[1].source_row_id().map(|row_id| row_id.as_str()),
        Some("dependency-ui-table")
    );
    assert_eq!(activations[1].kind, "keyboard");
    assert_eq!(activations[1].depth, 2);
}

#[open_gpui::test]
fn components_gallery_smoke_table_server_tree_loads_children_from_expansion_request(
    cx: &mut open_gpui::TestAppContext,
) {
    const TABLE_ID: &str = "component-table:server-tree";
    const SAMPLE: &str = "gallery:component-table-sample:server-tree";
    let workspace_toggle_selector = table_source_tree_toggle_selector(TABLE_ID, "server-workspace");
    let cache_toggle_selector = table_source_tree_toggle_selector(TABLE_ID, "server-cache");
    let failed_toggle_selector = table_source_tree_toggle_selector(TABLE_ID, "server-failed");
    let child_row_selector = table_source_row_selector(TABLE_ID, "server-api");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, &workspace_toggle_selector);

    assert!(
        cx.debug_bounds(&child_row_selector).is_none(),
        "expected server children to start app-unloaded"
    );
    assert!(
        cx.debug_bounds(&cache_toggle_selector).is_some(),
        "expected loading server branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(&failed_toggle_selector).is_some(),
        "expected failed server branch to render a disclosure affordance"
    );

    click(cx, &workspace_toggle_selector);
    settle(cx);
    let toggles = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles().to_vec()
    });
    assert!(
        cx.debug_bounds(&child_row_selector).is_some(),
        "expected server child row to render after the app supplies loaded children; toggles={toggles:?}"
    );
    assert_eq!(toggles.len(), 1);
    assert_eq!(toggles[0].sample_id, "server-tree");
    assert_eq!(
        toggles[0].source_row_id().map(|row_id| row_id.as_str()),
        Some("server-workspace")
    );
    assert!(toggles[0].expanded);
    assert_eq!(toggles[0].depth, 0);
    assert_eq!(toggles[0].loaded_child_count, 0);
    assert_eq!(toggles[0].children_load_state, "idle");
    assert_eq!(toggles[0].children_load_message, None);
    let activations_after_toggle =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.row_activations().to_vec()
        });
    assert!(
        activations_after_toggle.is_empty(),
        "expected manual expansion disclosure clicks to avoid row activation"
    );
}
