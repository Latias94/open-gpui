use super::*;

#[open_gpui::test]
fn table_runtime_resize_emits_controlled_sizing_change(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = sample_table_state(12)
                .with_column_sizing(TableColumnSizing::new().with_width("name", ui_px(160.0)));
            let table = Table::new("resize-runtime-table", "Resize runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .column_resize_mode(TableColumnResizeMode::OnEnd)
                .on_column_sizing_change(move |change, _, _| {
                    changes.borrow_mut().push(change);
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let handle = cx
        .debug_bounds("table:resize-runtime-table:resize:name")
        .expect("name resize handle should be rendered")
        .center();

    cx.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(handle.x + px(18.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_move(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());

    cx.simulate_mouse_up(
        point(handle.x + px(58.0), handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].column_id().as_str(), "name");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(
        changes[0]
            .sizing()
            .width(changes[0].column_id())
            .expect("controlled sizing should include resized column"),
        changes[0].width()
    );
}

#[open_gpui::test]
fn table_runtime_header_drag_emits_controlled_column_order_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnOrderChange>>>,
        state: Rc<RefCell<TableState>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = self.state.borrow().clone();
            let state_for_order = self.state.clone();
            let table = Table::new("order-runtime-table", "Order runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_column_order_change(move |change, _, _| {
                    changes.borrow_mut().push(change.clone());
                    let next = change.apply_to(state_for_order.borrow().clone());
                    *state_for_order.borrow_mut() = next;
                });

            div().w(px(420.0)).h(px(180.0)).child(table)
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 42_usize)])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ])
        .with_column_order(["name", "team", "score"])
        .with_pagination(TablePagination::disabled()),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
        state: state.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let start = cx
        .debug_bounds("table:order-runtime-table:header:score")
        .expect("score header should render")
        .center();
    let end = cx
        .debug_bounds("table:order-runtime-table:header-order-drop:before:team")
        .expect("team before-drop zone should render")
        .center();

    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.2,
            start.y + (end.y - start.y) * 0.2,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(
            start.x + (end.x - start.x) * 0.6,
            start.y + (end.y - start.y) * 0.6,
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.target_column_id().as_str(), "team");
    assert_eq!(change.placement(), TableColumnOrderPlacement::Before);
    assert_eq!(change.source_region(), TableColumnRegion::Center);
    assert_eq!(change.target_region(), TableColumnRegion::Center);
    assert_eq!(
        state
            .borrow()
            .column_order()
            .iter()
            .map(|column_id| column_id.as_str())
            .collect::<Vec<_>>(),
        ["name", "score", "team"]
    );
    assert!(
        cx.debug_bounds("table:order-runtime-table:header:score")
            .expect("score header should still render")
            .left()
            < cx.debug_bounds("table:order-runtime-table:header:team")
                .expect("team header should still render")
                .left(),
        "score should render before team after the reorder"
    );
}

#[open_gpui::test]
fn table_runtime_exposes_pinned_region_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("team", "UI")
                .with_cell("score", 42_usize)
                .with_cell("status", "Ready")])
            .with_columns([
                TableColumn::new("name", "Name"),
                TableColumn::new("team", "Team"),
                TableColumn::new("score", "Score"),
                TableColumn::new("status", "Status"),
            ])
            .with_column_order(["status", "score", "team", "name"])
            .with_column_pinning(
                TableColumnPinning::new()
                    .pinned_left(["name", "score"])
                    .pinned_right(["status"]),
            )
            .with_pagination(TablePagination::disabled());
            let table = Table::new("pinned-runtime-table", "Pinned runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(520.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for region in ["left", "center", "right"] {
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:header-region:{region}"
            ))
            .is_some(),
            "expected header {region} region selector to render"
        );
        assert!(
            cx.debug_bounds(&format!(
                "table:pinned-runtime-table:row-region:row-a:{region}"
            ))
            .is_some(),
            "expected body {region} region selector to render"
        );
    }

    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:header-center-scroll")
            .is_some(),
        "expected pinned header center region to render a horizontal scroll viewport"
    );
    assert!(
        cx.debug_bounds("scroll-area:table:pinned-runtime-table:row-center-scroll:row-a")
            .is_some(),
        "expected pinned body center region to render a horizontal scroll viewport"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_center_scrolls_without_moving_fixed_lanes(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-scroll-runtime-table",
                "Pinned scroll table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0));

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should render before horizontal scrolling");
    let body_center_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should render before horizontal scrolling");
    let left_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-scroll-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-64.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let header_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:header:team")
        .expect("center header should remain rendered after horizontal scrolling");
    let body_center_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:team")
        .expect("center body cell should remain rendered after horizontal scrolling");
    let left_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:score")
        .expect("left pinned body cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:pinned-scroll-runtime-table:cell:row-a:status")
        .expect("right pinned body cell should remain rendered after horizontal scrolling");

    assert!(
        header_center_after.left() < header_center_before.left(),
        "expected shared horizontal handle to move center header left; before={header_center_before:?} after={header_center_after:?}"
    );
    assert!(
        body_center_after.left() < body_center_before.left(),
        "expected horizontal body center lane to move left; before={body_center_before:?} after={body_center_after:?}"
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
}

#[open_gpui::test]
fn table_runtime_center_column_window_mounts_only_rendered_center_cells(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-runtime-table",
                "Center window runtime table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_some(),
        "expected the first center header to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_some(),
        "expected the first center body cell to render before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_none(),
        "far-right center headers should stay unmounted before horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_none(),
        "far-right center body cells should stay unmounted before horizontal scrolling"
    );

    let left_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should render before horizontal scrolling");
    let right_before = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should render before horizontal scrolling");
    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:center-window-runtime-table:row-center-scroll:row-0000")
        .expect("body center lane should expose a horizontal scroll viewport");

    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_00")
            .is_none(),
        "leftmost center headers should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_00")
            .is_none(),
        "leftmost center cells should unmount after the center window advances"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:header:metric_05")
            .is_some(),
        "far-right center headers should render after horizontal scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "far-right center cells should render after horizontal scrolling"
    );

    let left_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain rendered after horizontal scrolling");
    let right_after = cx
        .debug_bounds("table:center-window-runtime-table:cell:row-0000:status")
        .expect("right pinned cell should remain rendered after horizontal scrolling");
    assert_eq!(
        left_after.left(),
        left_before.left(),
        "left pinned lane should keep its screen-space x position"
    );
    assert_eq!(
        right_after.left(),
        right_before.left(),
        "right pinned lane should keep its screen-space x position"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_still_emits_sort_for_rendered_center_header(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "center-window-sort-runtime-table",
                "Center window sort table",
                sample_center_window_table_state_with_rows(20),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0)
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-sort-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let metric_05_header = cx
        .debug_bounds("table:center-window-sort-runtime-table:header:metric_05")
        .expect("virtualized center header should render after horizontal scrolling");
    cx.simulate_click(metric_05_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].column_id().as_str(), "metric_05");
    assert_eq!(actions[0].label(), "Metric 05");
    assert_eq!(actions[0].current_direction(), None);
    assert_eq!(
        actions[0].next_direction(),
        Some(TableSortDirection::Ascending)
    );
}

#[test]
fn table_behavior_snapshot_updates_center_column_summary_for_resize() {
    let base_snapshot = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(120.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let base_metric = base_snapshot
        .column(&TableColumnId::new("metric_05"))
        .expect("metric_05 should resolve before resize");

    let resized_snapshot = Table::new(
        "center-window-resize-plan-table",
        "Center window resize plan table",
        sample_center_window_table_state()
            .with_column_sizing(TableColumnSizing::new().with_width("metric_05", ui_px(180.0))),
    )
    .row_height(ui_px(24.0))
    .viewport_extent(ui_px(96.0))
    .overscan(0)
    .behavior_snapshot(UiPx::ZERO, ui_px(96.0));
    let resized_metric = resized_snapshot
        .column(&TableColumnId::new("metric_05"))
        .expect("metric_05 should resolve after resize");

    assert_eq!(
        base_snapshot.column_regions().center_columns(),
        resized_snapshot.column_regions().center_columns()
    );
    assert!(
        resized_snapshot.column_regions().center_width()
            > base_snapshot.column_regions().center_width()
    );
    assert!(
        resized_metric.width() > base_metric.width(),
        "expected the resized center column to widen"
    );
}

#[open_gpui::test]
fn table_runtime_center_column_window_keeps_row_virtualizer_independent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "center-window-rows-runtime-table",
                "Center window rows runtime table",
                sample_center_window_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(0);

            div()
                .size_full()
                .child(div().w(px(340.0)).h(px(160.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds(
            "scroll-area:table:center-window-rows-runtime-table:row-center-scroll:row-0000",
        )
        .expect("body center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontal center window should reveal far-right cells before vertical scrolling"
    );

    let first_row_pinned_cell = cx
        .debug_bounds("table:center-window-rows-runtime-table:cell:row-0000:name")
        .expect("left pinned cell should remain reachable before vertical scrolling");
    cx.simulate_event(ScrollWheelEvent {
        position: first_row_pinned_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0000")
            .is_none(),
        "vertical scrolling should still advance the row virtualizer"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:row:row-0010")
            .is_some(),
        "row 10 should render after vertical scrolling"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_05")
            .is_some(),
        "newly rendered rows should consume the current center column window"
    );
    assert!(
        cx.debug_bounds("table:center-window-rows-runtime-table:cell:row-0010:metric_00")
            .is_none(),
        "off-window center cells should remain unmounted on newly rendered rows"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_body_scrolls_without_moving_parent(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "pinned-body-scroll-runtime-table",
                "Pinned body scroll table",
                sample_pinned_table_state_with_rows(80),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(440.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "pinned-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "pinned-table-wrapper".into())
                                    .h(px(140.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let first_row_before = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
        .expect("first pinned body row should render before vertical scrolling");
    let header_before = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:header-row")
        .expect("pinned table header should render before vertical scrolling");
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_none(),
        "row 10 should start outside the initial pinned body window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:pinned-body-scroll-runtime-table:body-scroll")
        .expect("pinned table body viewport should expose a stable scroll selector");
    let first_row_cell = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:cell:row-0000:name")
        .expect("first pinned body row cell should render before vertical scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: first_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    let header_after = cx
        .debug_bounds("table:pinned-body-scroll-runtime-table:header-row")
        .expect("pinned table header should still be rendered after vertical scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside pinned Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert_eq!(
        header_after.top(),
        header_before.top(),
        "expected the table header to stay fixed while the body scrolls; before={header_before:?} after={header_after:?}"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0000")
            .is_none(),
        "expected first pinned row to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:pinned-body-scroll-runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the pinned table body"
    );
    assert!(
        viewport.size.width > px(0.0) && first_row_before.top() <= parent_bottom_after.bottom(),
        "pinned body viewport should remain measurable during the test"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keeps_bands_fixed_while_center_scrolls(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80).with_row_pinning(
                TableRowPinning::new()
                    .pinned_top(["row-0000"])
                    .pinned_bottom(["row-0079"]),
            );
            let table = Table::new(
                "row-pinning-runtime-table",
                "Row pinning runtime table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div().w(px(480.0)).h(px(240.0)).child(
                    ScrollArea::new(
                        "row-pinning-table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-table-wrapper".into())
                                    .h(px(164.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "row-pinning-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:top")
            .is_some(),
        "top row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:center")
            .is_some(),
        "center row-pinning band should expose a stable debug selector"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:body:bottom")
            .is_some(),
        "bottom row-pinning band should expose a stable debug selector"
    );
    let top_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should render before scrolling");
    let bottom_row_before = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should render before scrolling");
    let parent_bottom_before = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should render before table scrolling");
    let top_name_before = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should render before horizontal scrolling");
    let top_center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:row-center-scroll:row-0000")
        .expect("top pinned row should expose a horizontal center lane");

    cx.simulate_event(ScrollWheelEvent {
        position: top_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-440.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_name_after_horizontal = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0000:name")
        .expect("top pinned row left-pinned cell should stay mounted after horizontal scrolling");
    assert_eq!(
        top_name_after_horizontal.left(),
        top_name_before.left(),
        "left-pinned cells inside pinned rows should not move with the center lane"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0000:metric_05")
            .is_some(),
        "horizontally scrolled pinned rows should reveal far-right center cells"
    );
    let _center_viewport = cx
        .debug_bounds("scroll-area:table:row-pinning-runtime-table:body-scroll")
        .expect("center body should expose the vertical scroll viewport");
    let center_row_cell = cx
        .debug_bounds("table:row-pinning-runtime-table:cell:row-0001:name")
        .expect("first center row left-pinned cell should render before center scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: center_row_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0000")
        .expect("top pinned row should remain mounted after center scrolling");
    let bottom_row_after = cx
        .debug_bounds("table:row-pinning-runtime-table:row:row-0079")
        .expect("bottom pinned row should remain mounted after center scrolling");
    let parent_bottom_after = cx
        .debug_bounds("row-pinning-parent-bottom")
        .expect("parent bottom should remain mounted after center scrolling");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "top pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        bottom_row_after.top(),
        bottom_row_before.top(),
        "bottom pinned rows should stay fixed while center rows scroll"
    );
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "vertical wheel input inside row-pinned Table should not move the outer page"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:row:row-0011")
            .is_some(),
        "center rows should advance independently between pinned bands"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-runtime-table:cell:row-0011:metric_05")
            .is_some(),
        "new center rows should consume the current horizontal center window"
    );
}

#[open_gpui::test]
fn table_runtime_row_pinning_keyboard_navigation_scrolls_to_unrendered_center_row(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let state = sample_center_window_table_state_with_rows(80)
                .with_row_pinning(TableRowPinning::new().pinned_top(["row-0000"]));
            let table = Table::new(
                "row-pinning-keyboard-table",
                "Row pinning keyboard table",
                state,
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(120.0))
            .overscan(2);

            div().size_full().child(
                div()
                    .w(px(480.0))
                    .h(px(164.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(table),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_before = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should render before keyboard navigation");
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_none(),
        "far center row should start outside the rendered virtual window"
    );

    cx.simulate_click(top_row_before.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("end");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let top_row_after = cx
        .debug_bounds("table:row-pinning-keyboard-table:row:row-0000")
        .expect("top pinned row should remain mounted after keyboard navigation");
    assert_eq!(
        top_row_after.top(),
        top_row_before.top(),
        "keyboard navigation into the center region should not move the top pinned band"
    );
    assert!(
        cx.debug_bounds("table:row-pinning-keyboard-table:row:row-0079")
            .is_some(),
        "End should scroll an unrendered center row into the center virtual window"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_headers_still_sort_after_center_scroll(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        actions: Rc<RefCell<Vec<TableHeaderAction>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let table = Table::new(
                "pinned-sort-runtime-table",
                "Pinned sort table",
                sample_pinned_table_state(),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .on_sort_requested(move |action, _, _| {
                actions.borrow_mut().push(action);
            });

            div()
                .size_full()
                .child(div().w(px(420.0)).h(px(140.0)).child(table))
        }
    }

    let actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        actions: actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let body_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:row-center-scroll:row-a")
        .expect("body center lane should expose a horizontal scroll viewport");
    let header_center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-sort-runtime-table:header-center-scroll")
        .expect("header center lane should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: body_center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-160.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:pinned-sort-runtime-table:header:team")
            .is_some(),
        "center header should remain visible after scrolling"
    );
    let score_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:score")
        .expect("left pinned header should remain visible after scrolling");
    let status_header = cx
        .debug_bounds("table:pinned-sort-runtime-table:header:status")
        .expect("right pinned header should remain visible after scrolling");

    cx.simulate_click(header_center_viewport.center(), Default::default());
    cx.simulate_click(score_header.center(), Default::default());
    cx.simulate_click(status_header.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let actions = actions.borrow();
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].column_id().as_str(), "team");
    assert_eq!(actions[1].column_id().as_str(), "score");
    assert_eq!(actions[2].column_id().as_str(), "status");
}

#[open_gpui::test]
fn table_runtime_pinned_header_drag_emits_controlled_column_order_change(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnOrderChange>>>,
        state: Rc<RefCell<TableState>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let state = self.state.borrow().clone();
            let state_for_order = self.state.clone();
            let table = Table::new("pinned-order-runtime-table", "Pinned order table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .on_column_order_change(move |change, _, _| {
                    changes.borrow_mut().push(change.clone());
                    let next = change.apply_to(state_for_order.borrow().clone());
                    *state_for_order.borrow_mut() = next;
                });

            div()
                .size_full()
                .child(div().w(px(560.0)).h(px(180.0)).child(table))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(
        TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 42_usize)
            .with_cell("status", "Ready")])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
            TableColumn::new("status", "Status"),
        ])
        .with_column_order(["name", "team", "score", "status"])
        .with_column_pinning(
            TableColumnPinning::new()
                .pinned_left(["name"])
                .pinned_right(["status"]),
        )
        .with_pagination(TablePagination::disabled()),
    ));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
        state: state.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let center_viewport = cx
        .debug_bounds("scroll-area:table:pinned-order-runtime-table:header-center-scroll")
        .expect("center header should expose a horizontal scroll viewport");
    cx.simulate_event(ScrollWheelEvent {
        position: center_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-180.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let score_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header:score")
        .expect("center score header should remain visible after scrolling");
    let _team_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header:team")
        .expect("center team header should remain visible after scrolling");
    let drop_before = cx
        .debug_bounds("table:pinned-order-runtime-table:header-order-drop:before:team")
        .expect("team before-drop zone should render in split pinned layout");

    cx.simulate_mouse_down(score_before.center(), MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(score_before.center().x + px(18.0), score_before.center().y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(
            score_before.center().x + px(42.0),
            score_before.center().y + px(2.0),
        ),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(drop_before.center(), MouseButton::Left, Default::default());
    cx.simulate_mouse_up(drop_before.center(), MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.column_id().as_str(), "score");
    assert_eq!(change.target_column_id().as_str(), "team");
    assert_eq!(change.placement(), TableColumnOrderPlacement::Before);
    assert_eq!(change.source_region(), TableColumnRegion::Center);
    assert_eq!(change.target_region(), TableColumnRegion::Center);
    assert_eq!(
        state
            .borrow()
            .column_order()
            .iter()
            .map(|column_id| column_id.as_str())
            .collect::<Vec<_>>(),
        ["name", "score", "team", "status"]
    );
    assert!(
        cx.debug_bounds("table:pinned-order-runtime-table:header:score")
            .expect("score header should still render")
            .left()
            < cx.debug_bounds("table:pinned-order-runtime-table:header:team")
                .expect("team header should still render")
                .left(),
        "score should render before team after the reorder"
    );
}

#[open_gpui::test]
fn table_runtime_pinned_resize_handles_emit_changes_for_center_and_pinned_columns(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        changes: Rc<RefCell<Vec<TableColumnSizingChange>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let table = Table::new(
                "pinned-resize-runtime-table",
                "Pinned resize table",
                sample_pinned_table_state()
                    .with_column_sizing(TableColumnSizing::new().with_width("team", ui_px(160.0))),
            )
            .row_height(ui_px(24.0))
            .viewport_extent(ui_px(96.0))
            .column_resize_mode(TableColumnResizeMode::OnEnd)
            .on_column_sizing_change(move |change, _, _| {
                changes.borrow_mut().push(change);
            });

            div()
                .size_full()
                .child(div().w(px(620.0)).h(px(140.0)).child(table))
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let team_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:team")
        .expect("center resize handle should remain reachable in split layout");
    let team_handle = point(
        team_handle_bounds.right() - px(1.0),
        team_handle_bounds.center().y,
    );
    let score_handle_bounds = cx
        .debug_bounds("table:pinned-resize-runtime-table:resize:score")
        .expect("pinned resize handle should remain reachable");
    let score_handle = point(
        score_handle_bounds.right() - px(1.0),
        score_handle_bounds.center().y,
    );

    cx.simulate_mouse_down(team_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(team_handle.x + px(4.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(24.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert!(changes.borrow().is_empty());
    cx.simulate_mouse_move(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(team_handle.x + px(60.0), team_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(changes.borrow().len(), 1);
    assert_eq!(changes.borrow()[0].column_id().as_str(), "team");

    cx.simulate_mouse_down(score_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(score_handle.x + px(4.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(24.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    assert_eq!(changes.borrow().len(), 1);
    cx.simulate_mouse_move(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(score_handle.x + px(60.0), score_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let changes = changes.borrow();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].column_id().as_str(), "team");
    assert!(changes[0].width() > ui_px(160.0));
    assert_eq!(changes[1].column_id().as_str(), "score");
    assert!(changes[1].width() > ui_px(128.0));
}

#[open_gpui::test]
fn table_runtime_virtualized_body_scrolls_without_moving_parent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new("runtime-table", "Runtime table", sample_table_state(80))
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(120.0))
                .overscan(2);

            div().size_full().child(
                div().w(px(360.0)).h(px(220.0)).child(
                    ScrollArea::new(
                        "table-parent-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-top".into())
                                    .h(px(72.0))
                                    .w_full()
                                    .child("Parent top"),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-wrapper".into())
                                    .h(px(132.0))
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .child(table),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "table-parent-bottom".into())
                                    .h(px(240.0))
                                    .w_full()
                                    .child("Parent bottom"),
                            ),
                    )
                    .vertical(),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_some(),
        "expected first table row to render before scrolling"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_none(),
        "expected row 10 to stay outside the initial overscan window"
    );
    let parent_bottom_before = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should be rendered before table scrolling");
    let viewport = cx
        .debug_bounds("scroll-area:table:runtime-table:body-scroll")
        .expect("table body viewport should expose a stable scroll selector");

    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let parent_bottom_after = cx
        .debug_bounds("table-parent-bottom")
        .expect("parent bottom should still be rendered after table scrolling");
    assert_eq!(
        parent_bottom_after.top(),
        parent_bottom_before.top(),
        "expected wheel input inside Table to stay inside the table body; before={parent_bottom_before:?} after={parent_bottom_after:?}"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0000")
            .is_none(),
        "expected row 0 to unmount after the virtual window advances"
    );
    assert!(
        cx.debug_bounds("table:runtime-table:row:row-0010")
            .is_some(),
        "expected row 10 to render after scrolling the table body"
    );
}

#[open_gpui::test]
fn table_runtime_cache_invalidates_when_table_state_changes(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        descending: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut state = sample_table_state(20);
            if self.descending {
                state = state.with_sorting([TableSort::descending("score")]);
            }

            let table = Table::new("cache-runtime-table", "Cache runtime table", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0))
                .overscan(0);

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { descending: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_some(),
        "expected unsorted table to render row 0 first"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_none(),
        "expected last row to stay outside the initial unsorted window"
    );

    view.update(cx, |view, cx| {
        view.descending = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0019")
            .is_some(),
        "expected cache invalidation to expose the descending first row"
    );
    assert!(
        cx.debug_bounds("table:cache-runtime-table:row:row-0000")
            .is_none(),
        "expected stale unsorted row window to be replaced"
    );
}

#[open_gpui::test]
fn table_runtime_content_fit_widths_follow_visible_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        long_value: bool,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let status_value = if self.long_value {
                "Ready for release rollout"
            } else {
                "Ready"
            };
            let state = TableState::new([TableRow::new("row-a")
                .with_cell("name", "Alpha")
                .with_cell("status", status_value)])
            .with_columns([
                TableColumn::new("name", "Name").with_width(ui_px(140.0)),
                TableColumn::new("status", "Status").with_content_fit(),
            ])
            .with_pagination(TablePagination::disabled());
            let table = Table::new("content-fit-runtime-table", "Content fit runtime", state)
                .row_height(ui_px(24.0))
                .viewport_extent(ui_px(96.0));

            div().w(px(360.0)).h(px(140.0)).child(table)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView { long_value: false });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_before = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should render before content growth");
    let status_cell_before = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should render before content growth");
    assert_eq!(status_header_before.left(), status_cell_before.left());
    assert_eq!(status_header_before.right(), status_cell_before.right());

    view.update(cx, |view, cx| {
        view.long_value = true;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let status_header_after = cx
        .debug_bounds("table:content-fit-runtime-table:header:status")
        .expect("status header should still render after content growth");
    let status_cell_after = cx
        .debug_bounds("table:content-fit-runtime-table:cell:row-a:status")
        .expect("status cell should still render after content growth");
    assert_eq!(status_header_after.left(), status_cell_after.left());
    assert_eq!(status_header_after.right(), status_cell_after.right());
    assert!(
        (status_header_after.right() - status_header_after.left())
            > (status_header_before.right() - status_header_before.left()),
        "expected the content-fit column to widen when a longer visible value appears"
    );
    assert_eq!(
        cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
            .expect("fixed-width name cell should stay rendered")
            .right()
            - cx.debug_bounds("table:content-fit-runtime-table:cell:row-a:name")
                .expect("fixed-width name cell should stay rendered")
                .left(),
        px(140.0)
    );
}

#[open_gpui::test]
fn table_runtime_measured_row_height_reflows_after_paint(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        state: TableState,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let table = Table::new(
                "measured-row-runtime-table",
                "Measured row runtime",
                self.state.clone(),
            )
            .row_height(ui_px(24.0))
            .row_measure_mode(TableRowMeasureMode::Measured)
            .viewport_extent(ui_px(120.0));

            div().w(px(260.0)).h(px(180.0)).child(table)
        }
    }

    let state = TableState::new([
        TableRow::new("row-a").with_cell(
            "description",
            "Measured rows should wrap onto multiple lines when the adapter can grow them from rendered content",
        ),
        TableRow::new("row-b").with_cell("description", "Short"),
    ])
    .with_columns([TableColumn::new("description", "Description").with_width(ui_px(72.0))])
    .with_pagination(TablePagination::disabled());

    let (_, cx) = cx.add_window_view(move |_, _| TestView { state });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_a_after = cx
        .debug_bounds("table:measured-row-runtime-table:row:row-a")
        .expect("measured row A should remain rendered after repaint");
    let row_b_after = cx
        .debug_bounds("table:measured-row-runtime-table:row:row-b")
        .expect("measured row B should remain rendered after repaint");
    assert!(
        row_a_after.bottom() - row_a_after.top() > px(24.0),
        "expected the measured first row to grow taller than the fallback row height"
    );
    assert!(
        row_b_after.top() >= row_a_after.bottom() - px(1.0),
        "expected the second row to sit below the expanded first row after the measurement cache is applied; row_a_after.bottom={:?}, row_b_after.top={:?}",
        row_a_after.bottom(),
        row_b_after.top()
    );
}
