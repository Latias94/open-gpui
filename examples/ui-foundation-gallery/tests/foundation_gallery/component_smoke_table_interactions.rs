use super::*;

#[open_gpui::test]
fn components_gallery_smoke_table_scroll_stays_inside_sample(cx: &mut open_gpui::TestAppContext) {
    const TABLE_ID: &str = "component-table:release-queue";

    let body_scroll_selector = TableDebugSelector::body_scroll(TABLE_ID);
    let first_row = table_source_row_selector(TABLE_ID, "release-queue-row-0000");
    let scrolled_row = table_source_row_selector(TABLE_ID, "release-queue-row-0010");
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:table");
    scroll_page_selector_into_view(&shell, cx, &body_scroll_selector);
    let sample_before = bounds(cx, "gallery:component-table-sample:release-queue");
    let table_viewport = bounds(cx, &body_scroll_selector);

    assert!(
        cx.debug_bounds(&first_row).is_some(),
        "expected the initial release queue table window to render the first row"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: table_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, "gallery:component-table-sample:release-queue");

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Table viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds(&first_row).is_none(),
        "expected virtualized Table row 0000 to leave the rendered window after internal scroll"
    );
    assert!(
        cx.debug_bounds(&scrolled_row).is_some(),
        "expected virtualized Table row 0010 to enter the rendered window after internal scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_resizable_table_resize_updates_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let name_cell = table_source_cell_selector(
        "component-table:release-resize",
        "release-resize-row-000",
        "name",
    );
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());

    jump_components_directory_to(cx, "gallery:component-page-jump:table");
    scroll_page_selector_into_view(
        &shell,
        cx,
        "table:component-table:release-resize:resize:name",
    );
    let sample_before = bounds(cx, "gallery:component-table-sample:release-resize");
    let header_before = bounds(cx, "table:component-table:release-resize:header:name");
    let cell_before = bounds(cx, &name_cell);
    let resize_handle = bounds(cx, "table:component-table:release-resize:resize:name");

    assert_eq!(header_before.size.width, cell_before.size.width);
    assert!(
        cx.debug_bounds("table:component-table:release-resize:resize:score")
            .is_none(),
        "expected the score column to stay non-resizable"
    );

    drag(
        cx,
        resize_handle.center(),
        point(
            resize_handle.center().x + px(60.0),
            resize_handle.center().y,
        ),
    );

    let sample_after = bounds(cx, "gallery:component-table-sample:release-resize");
    let header_after = bounds(cx, "table:component-table:release-resize:header:name");
    let cell_after = bounds(cx, &name_cell);
    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.sizing_changes().to_vec()
    });
    let committed_width =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.committed_sizing("release-resize")
                .and_then(|sizing| sizing.width(&TableColumnId::new("name")))
        });

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Table resize drag to keep the sample card anchored"
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "release-resize");
    assert_eq!(changes[0].column_id, "name");
    assert!(changes[0].width > ui_px(188.0));
    assert_eq!(committed_width, Some(changes[0].width));
    assert_eq!(header_after.size.width, cell_after.size.width);
    assert!(header_after.size.width > header_before.size.width);
}

#[open_gpui::test]
fn components_gallery_smoke_faceted_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TRIGGER: &str = "popover:component-table-faceted-filter:filter-board:status:trigger";
    const CONTENT: &str =
        "table-faceted-filter:component-table-faceted-filter:filter-board:status:content";
    const DONE_OPTION: &str =
        "table-faceted-filter:component-table-faceted-filter:filter-board:status:option:Done";
    let initial_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-177");
    let filtered_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-171");

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
        cx.debug_bounds(&initial_row).is_some(),
        "expected the initial filtered board row to render before selecting a status facet"
    );

    click(cx, TRIGGER);
    settle(cx);
    if cx.debug_bounds(CONTENT).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-180.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected faceted-filter popup wheel input to stay inside the table sample"
    );

    click(cx, DONE_OPTION);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes().to_vec()
    });
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sample_id, "filter-board");
    assert_eq!(changes[0].column_id, "status");
    assert_eq!(changes[0].selected_values, vec!["Done".to_owned()]);
    assert_eq!(changes[0].toggled_value, Some("Done".to_owned()));
    assert!(changes[0].selected);
    assert_eq!(changes[0].filtered_rows, 15);
    assert_eq!(changes[0].final_rows, 15);
    assert!(
        cx.debug_bounds(&initial_row).is_none(),
        "expected the Doing row to leave the rendered window after selecting Done"
    );
    assert!(
        cx.debug_bounds(&filtered_row).is_some(),
        "expected the highest-scoring Done row to render after selecting Done"
    );

    if cx.debug_bounds(DONE_OPTION).is_none() {
        click(cx, TRIGGER);
        settle(cx);
    }
    click(cx, DONE_OPTION);
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes().to_vec()
    });
    assert_eq!(changes.len(), 2);
    assert!(changes[1].selected_values.is_empty());
    assert_eq!(changes[1].toggled_value, Some("Done".to_owned()));
    assert!(!changes[1].selected);
    assert_eq!(changes[1].filtered_rows, 60);
    assert_eq!(changes[1].final_rows, 24);
    assert!(
        cx.debug_bounds(&initial_row).is_some(),
        "expected clearing the status facet to restore the original filtered board rows"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_global_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:filter-board:root";
    const INPUT: &str = "text-input:component-table-global-filter:filter-board-input:root";
    let initial_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-177");
    let filtered_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-012");

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let expected_state = TableGlobalFilterChange::new("012").apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, INPUT);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected filter-board controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(&initial_row).is_some(),
        "expected the initial filtered board row to render before applying a global search"
    );

    click(cx, INPUT);
    settle(cx);
    cx.simulate_input("012");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the global-search input to stay inside the table sample"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_changes().to_vec()
    });
    assert!(!changes.is_empty());
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one global-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.query, "012");
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);

    let persisted = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_override(SAMPLE_ID)
            .and_then(|state| state.global_filter().map(str::to_owned))
    });
    assert_eq!(persisted.as_deref(), Some("012"));
    assert!(
        cx.debug_bounds(&initial_row).is_none(),
        "expected the initial board row to leave the rendered window after applying global search"
    );
    assert!(
        cx.debug_bounds(&filtered_row).is_some(),
        "expected the matching board row to render after applying global search"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_predicate_filter_updates_table_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TOOLBAR: &str = "table-toolbar:component-table-toolbar:filter-board:root";
    const INPUT: &str = "text-input:component-table-predicate-filter:filter-board:name-value:root";
    let initial_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-177");
    let filtered_row =
        table_source_row_selector("component-table:filter-board", "filter-board-row-012");

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let expected_state = TablePredicateFilterChange::new(
        "name",
        TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
        "012",
    )
    .apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, INPUT);

    let sample_before = bounds(cx, SAMPLE);
    assert!(
        cx.debug_bounds(TOOLBAR).is_some(),
        "expected filter-board controls to render inside the table toolbar recipe"
    );
    assert!(
        cx.debug_bounds(&initial_row).is_some(),
        "expected the initial filtered board row to render before applying a name predicate"
    );

    click(cx, INPUT);
    settle(cx);
    cx.simulate_input("012");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected the predicate input to stay inside the table sample"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_changes().to_vec()
    });
    assert!(
        changes.len() >= "012".len(),
        "typing a board-item predicate should record controlled changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one predicate-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.column_id, "name");
    assert_eq!(last.operator.as_deref(), Some("text:contains"));
    assert_eq!(last.value, "012");
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);

    let persisted = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_override(SAMPLE_ID).and_then(|state| {
            state
                .filters()
                .iter()
                .find(|filter| filter.column() == &TableColumnId::new("name"))
                .and_then(|filter| {
                    filter
                        .text_predicate()
                        .map(|(operator, query, _)| (operator, query.to_owned()))
                })
        })
    });
    assert_eq!(
        persisted,
        Some((TableTextFilterOperator::Contains, "012".to_owned()))
    );
    assert!(
        cx.debug_bounds(&initial_row).is_none(),
        "expected the initial board row to leave the rendered window after applying name predicate"
    );
    assert!(
        cx.debug_bounds(&filtered_row).is_some(),
        "expected the matching board row to render after applying name predicate"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_range_filter_updates_table_rows(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE_ID: &str = "filter-board";
    const SAMPLE: &str = "gallery:component-table-sample:filter-board";
    const TRIGGER: &str = "popover:component-table-range-filter:filter-board:score:trigger";
    const CONTENT: &str =
        "table-range-filter:component-table-range-filter:filter-board:score:content";
    const MIN_INPUT: &str = "text-input:component-table-range-filter:filter-board:score-min:root";

    let table_samples = pages::components::table_samples(ThemeTokens::default());
    let sample = table_sample(&table_samples, SAMPLE_ID);
    let baseline = sample.state.resolve();
    let baseline_rows = baseline.filtered_model().rows().len();
    let expected_state =
        TableRangeFilterChange::new("score", "170", "").apply_to(sample.state.clone());
    let expected = expected_state.resolve();
    let expected_filtered_rows = expected.filtered_model().rows().len();
    let expected_final_rows = expected.final_model().rows().len();
    let expected_page_row_identities = expected
        .final_model()
        .rows()
        .iter()
        .map(|row| row.identity().clone())
        .collect::<Vec<_>>();
    let removed_row_identity = baseline
        .final_model()
        .rows()
        .iter()
        .find(|row| !expected_page_row_identities.contains(row.identity()))
        .unwrap_or_else(|| panic!("expected score range to remove at least one initial page row"))
        .identity()
        .clone();
    let removed_row_selector =
        TableDebugSelector::row("component-table:filter-board", &removed_row_identity);

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
        cx.debug_bounds(&removed_row_selector).is_some(),
        "expected the initial filter-board row to render before applying a score range"
    );

    click(cx, TRIGGER);
    settle(cx);
    let popup_content = bounds(cx, CONTENT);
    cx.simulate_event(ScrollWheelEvent {
        position: popup_content.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-180.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after_popup_wheel = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after_popup_wheel.top(),
        sample_before.top(),
        "expected range-filter popup wheel input to stay inside the table sample"
    );

    click(cx, MIN_INPUT);
    settle(cx);
    cx.simulate_input("170");
    settle(cx);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.range_filter_changes().to_vec()
    });
    assert!(
        changes.len() >= 3,
        "typing a three-digit range minimum should record controlled changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one range-filter change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(last.column_id, "score");
    assert_eq!(last.min_text, "170");
    assert_eq!(last.max_text, "");
    assert_eq!(last.min_value, Some(170.0));
    assert_eq!(last.max_value, None);
    assert!(!last.cleared);
    assert_eq!(last.filtered_rows, expected_filtered_rows);
    assert_eq!(last.final_rows, expected_final_rows);
    assert!(
        last.filtered_rows < baseline_rows,
        "score range should narrow the table row model"
    );

    let persisted_range =
        cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
            log.range_filter_override(SAMPLE_ID).and_then(|state| {
                state
                    .filters()
                    .iter()
                    .find(|filter| filter.column() == &TableColumnId::new("score"))
                    .and_then(|filter| filter.number_range_bounds())
            })
        });
    assert_eq!(persisted_range, Some((Some(170.0), None)));
    assert!(
        cx.debug_bounds(&removed_row_selector).is_none(),
        "expected lower-score filter-board row to leave the rendered window after applying score range"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_editable_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "editable-release";
    const SAMPLE: &str = "gallery:component-table-sample:editable-release";
    const TABLE_ID: &str = "component-table:editable-release";
    const SOURCE_ROW_ID: &str = "editable-release-row-000";
    let name_input = table_source_text_input_editor_selector(TABLE_ID, SOURCE_ROW_ID, "name");
    let status_input = table_source_text_input_editor_selector(TABLE_ID, SOURCE_ROW_ID, "status");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &name_input);

    assert!(
        cx.debug_bounds(&status_input).is_none(),
        "read-only status column should not mount a text input"
    );
    let sample_before = bounds(cx, SAMPLE);
    let input = bounds(cx, &name_input);
    cx.simulate_click(
        point(input.right() - px(8.0), input.center().y),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input(" Prime");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a table cell should not move the sample card"
    );
    assert!(
        cx.debug_bounds(&name_input).is_some(),
        "editable input should remain mounted after app-owned state feedback"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert!(
        changes.len() >= 2,
        "gallery edit should record controlled text changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(
        last.source_row_id().map(|id| id.as_str()),
        Some(SOURCE_ROW_ID)
    );
    assert_eq!(last.column_id, "name");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("Prime"));

    let edited_name = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("name")))
            .map(TableCellValue::filter_text)
    });
    assert_eq!(edited_name.as_deref(), Some("Editable release 000 Prime"));
}

#[open_gpui::test]
fn components_gallery_smoke_checkbox_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "toggle-release";
    const SAMPLE: &str = "gallery:component-table-sample:toggle-release";
    const TABLE_ID: &str = "component-table:toggle-release";
    const SOURCE_ROW_ID: &str = "toggle-release-row-000";
    let enabled_checkbox =
        table_source_checkbox_editor_selector(TABLE_ID, SOURCE_ROW_ID, "enabled");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &enabled_checkbox);

    let sample_before = bounds(cx, SAMPLE);
    let checkbox = bounds(cx, &enabled_checkbox);
    cx.simulate_click(checkbox.center(), Default::default());
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "toggling a table cell should not move the sample card"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert_eq!(
        changes.len(),
        1,
        "checkbox toggle should record one controlled change"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one checkbox edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(
        last.source_row_id().map(|id| id.as_str()),
        Some(SOURCE_ROW_ID)
    );
    assert_eq!(last.column_id, "enabled");
    assert_eq!(last.outcome, "updated");
    assert_eq!(last.previous_text, "true");
    assert_eq!(last.next_text, "false");

    let edited_enabled = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("enabled")))
            .cloned()
    });
    assert_eq!(edited_enabled, Some(TableCellValue::Bool(false)));
}

#[open_gpui::test]
fn components_gallery_smoke_select_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "select-release";
    const SAMPLE: &str = "gallery:component-table-sample:select-release";
    const TABLE_ID: &str = "component-table:select-release";
    const SOURCE_ROW_ID: &str = "select-release-row-000";
    const STATUS_CONTENT: &str =
        "select:Edit status for row select-release-row-000:select-content-scroll:content";
    let status_select = table_source_select_editor_selector(TABLE_ID, SOURCE_ROW_ID, "status");
    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &status_select);

    let sample_before = bounds(cx, SAMPLE);
    let trigger = bounds(cx, &status_select);
    cx.simulate_click(trigger.center(), Default::default());
    settle(cx);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    if cx.debug_bounds(STATUS_CONTENT).is_none() {
        cx.simulate_keystrokes("space");
        settle(cx);
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    assert!(
        cx.debug_bounds(STATUS_CONTENT).is_some(),
        "select content should open from the table trigger"
    );
    let blocked_option = required_table_source_select_option_selector(
        cx,
        TABLE_ID,
        SOURCE_ROW_ID,
        "status",
        "blocked",
    );
    let blocked = bounds(cx, &blocked_option);
    cx.simulate_click(blocked.center(), Default::default());
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "selecting a table cell should not move the sample card"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert_eq!(
        changes.len(),
        1,
        "select choice should record one controlled change"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one select edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(
        last.source_row_id().map(|id| id.as_str()),
        Some(SOURCE_ROW_ID)
    );
    assert_eq!(last.column_id, "status");
    assert_eq!(last.outcome, "updated");
    assert_eq!(last.previous_text, "ready");
    assert_eq!(last.next_text, "blocked");

    let edited_status = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("status")))
            .map(TableCellValue::filter_text)
    });
    assert_eq!(edited_status.as_deref(), Some("blocked"));
}

#[open_gpui::test]
fn components_gallery_smoke_multiline_table_cell_updates_sample_rows(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "multiline-release";
    const SAMPLE: &str = "gallery:component-table-sample:multiline-release";
    const TABLE_ID: &str = "component-table:multiline-release";
    const SOURCE_ROW_ID: &str = "multiline-release-row-000";
    let notes_input = table_source_textarea_editor_selector(TABLE_ID, SOURCE_ROW_ID, "notes");
    let notes_text_input =
        table_source_text_input_editor_selector(TABLE_ID, SOURCE_ROW_ID, "notes");
    let status_textarea = table_source_textarea_editor_selector(TABLE_ID, SOURCE_ROW_ID, "status");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &notes_input);

    assert!(
        cx.debug_bounds(&notes_text_input).is_none(),
        "multiline notes column should not mount a single-line text input"
    );
    assert!(
        cx.debug_bounds(&status_textarea).is_none(),
        "read-only status column should not mount a textarea"
    );
    let sample_before = bounds(cx, SAMPLE);
    let input = bounds(cx, &notes_input);
    cx.simulate_click(
        point(input.right() - px(8.0), input.bottom() - px(12.0)),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input("\nQA note");
    settle(cx);

    let sample_after = bounds(cx, SAMPLE);
    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a multiline table cell should not move the sample card"
    );
    assert!(
        cx.debug_bounds(&notes_input).is_some(),
        "multiline textarea should remain mounted after app-owned state feedback"
    );

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    assert!(
        changes.len() >= 2,
        "gallery multiline edit should record controlled text changes; changes={changes:?}"
    );
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one multiline edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(
        last.source_row_id().map(|id| id.as_str()),
        Some(SOURCE_ROW_ID)
    );
    assert_eq!(last.column_id, "notes");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("QA note"));
    assert!(
        last.next_text.contains('\n'),
        "multiline edit payload should preserve newlines"
    );

    let edited_notes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_override(SAMPLE_ID)
            .and_then(|state| state.rows().first())
            .and_then(|row| row.cell(&TableColumnId::new("notes")))
            .map(TableCellValue::filter_text)
    });
    assert!(
        edited_notes
            .as_deref()
            .is_some_and(|notes| notes.contains("QA note") && notes.contains('\n')),
        "app-owned table state should store the newline-preserving textarea edit; notes={edited_notes:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_content_fit_table_cell_edit_widens_name_column(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE_ID: &str = "content-fit-release";
    const SAMPLE: &str = "gallery:component-table-sample:content-fit-release";
    const TABLE_ID: &str = "component-table:content-fit-release";
    const SOURCE_ROW_ID: &str = "editable-release-row-000";
    const NAME_HEADER: &str = "table:component-table:content-fit-release:header:name";
    let name_input = table_source_text_input_editor_selector(TABLE_ID, SOURCE_ROW_ID, "name");
    let name_cell = table_source_cell_selector(TABLE_ID, SOURCE_ROW_ID, "name");
    let score_cell = table_source_cell_selector(TABLE_ID, SOURCE_ROW_ID, "score");

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TableSampleRuntimeLog::default());
    let table_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Table")
        .unwrap_or_else(|| panic!("expected catalog entry `Table`"));
    focus_components_catalog_entry(&shell, cx, table_entry);
    scroll_page_selector_into_view(&shell, cx, &name_input);

    let sample_before = bounds(cx, SAMPLE);
    let header_before = bounds(cx, NAME_HEADER);
    let cell_before = bounds(cx, &name_cell);
    let score_before = bounds(cx, &score_cell);
    let input = bounds(cx, &name_input);

    assert_eq!(header_before.size.width, cell_before.size.width);
    assert_eq!(
        pages::components::table_samples(ThemeTokens::default())
            .iter()
            .find(|sample| sample.id == SAMPLE_ID)
            .expect("content-fit sample should exist")
            .behavior_snapshot()
            .columns()[0]
            .width_policy(),
        TableColumnWidthPolicy::ContentFit
    );

    cx.simulate_click(
        point(input.right() - px(8.0), input.center().y),
        Default::default(),
    );
    settle(cx);
    cx.simulate_input(" Prime");
    settle(cx);
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let header_after = bounds(cx, NAME_HEADER);
    let cell_after = bounds(cx, &name_cell);
    let score_after = bounds(cx, &score_cell);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "editing a content-fit table cell should not move the sample card"
    );
    assert_eq!(header_after.size.width, cell_after.size.width);
    assert!(header_after.size.width > header_before.size.width);
    assert_eq!(score_after.size.width, score_before.size.width);

    let changes = cx.read_global::<pages::components::TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes().to_vec()
    });
    let last = changes
        .last()
        .unwrap_or_else(|| panic!("expected at least one edit change"));
    assert_eq!(last.sample_id, SAMPLE_ID);
    assert_eq!(
        last.source_row_id().map(|id| id.as_str()),
        Some(SOURCE_ROW_ID)
    );
    assert_eq!(last.column_id, "name");
    assert_eq!(last.outcome, "updated");
    assert!(last.next_text.contains("Prime"));
}
