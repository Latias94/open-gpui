use super::*;

#[open_gpui::test]
fn table_virtual_focus_proxy_preserves_keyboard_claim_without_stealing_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct VirtualFocusProbe {
        outside_focus: FocusHandle,
        activations: Rc<RefCell<Vec<TableRowIdentity>>>,
    }

    impl Render for VirtualFocusProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let rows = (0..80).map(|index| {
                TableRow::new("duplicate")
                    .with_instance_id(format!("row-{index:04}"))
                    .with_cell("name", format!("Package {index:04}"))
                    .with_cell("score", index as usize)
            });
            let table = Table::new(
                "virtual-focus-table",
                "Virtual focus table",
                TableState::new(rows)
                    .with_columns([
                        TableColumn::new("name", "Name"),
                        TableColumn::new("score", "Score"),
                    ])
                    .with_pagination(TablePagination::disabled()),
            )
            .row_height(ui_px(24.0))
            .row_measure_mode(TableRowMeasureMode::Measured)
            .viewport_extent(ui_px(72.0))
            .overscan(0)
            .virtualizer_snapshot(TableVirtualizerSnapshot::new([
                TableVirtualizerSnapshotItem::new(
                    TableRowIdentity::source_instance("duplicate", "row-0040"),
                    ui_px(480.0),
                ),
            ]))
            .default_focused_row(TableRowIdentity::source_instance("duplicate", "row-0000"))
            .on_row_activate(move |activation, _, _| {
                activations.borrow_mut().push(activation.identity().clone());
            });

            div()
                .size_full()
                .on_key_down(|event: &open_gpui::KeyDownEvent, window, cx| {
                    let modifiers = event.keystroke.modifiers;
                    if event.keystroke.key.as_str() != "tab"
                        || modifiers.control
                        || modifiers.alt
                        || modifiers.platform
                        || modifiers.function
                    {
                        return;
                    }
                    if modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                    window.prevent_default();
                })
                .child(div().w(px(360.0)).h(px(120.0)).child(table))
                .child(
                    div()
                        .id("virtual-focus-outside")
                        .debug_selector(|| "virtual-focus-outside".to_owned())
                        .role(accesskit::Role::Button)
                        .aria_label("Outside")
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.outside_focus)
                        .child("Outside"),
                )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, cx| VirtualFocusProbe {
        outside_focus: cx.focus_handle(),
        activations: activations.clone(),
    });
    assert!(cx.activate_accessibility());
    cx.update(|window, cx| window.draw(cx).clear());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial virtual focus tree should publish");
    let (table_id, _) =
        node_with_role_and_label(&initial, accesskit::Role::Table, "Virtual focus table");
    let (row_zero_id, _) = node_with_role_and_row_index(&initial, accesskit::Role::Row, 2);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: row_zero_id,
        data: None,
    }));
    cx.run_until_parked();

    let row_zero_cell = cx
        .debug_bounds(&TableDebugSelector::cell(
            "virtual-focus-table",
            &TableRowIdentity::source_instance("duplicate", "row-0000"),
            &TableColumnId::new("name"),
        ))
        .expect("focused first row should render");
    cx.simulate_event(ScrollWheelEvent {
        position: row_zero_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let offscreen = cx
        .latest_accessibility_tree_update()
        .expect("offscreen focus proxy tree should publish");
    assert!(!offscreen.nodes.iter().any(|(id, _)| *id == row_zero_id));
    assert_eq!(offscreen.focus, table_id);
    assert!(cx.debug_selector_is_focused("table:virtual-focus-table:root"));
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: row_zero_id,
        data: None,
    }));
    assert!(activations.borrow().is_empty());
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("stale action should leave the proxy tree published")
            .focus,
        table_id
    );

    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("virtual-focus-outside"));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.debug_selector_is_focused("table:virtual-focus-table:root"));

    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let row_one_selector = TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0001"),
    );
    assert!(cx.debug_selector_is_focused(&row_one_selector));

    let row_one_cell = cx
        .debug_bounds(&TableDebugSelector::cell(
            "virtual-focus-table",
            &TableRowIdentity::source_instance("duplicate", "row-0001"),
            &TableColumnId::new("name"),
        ))
        .expect("proxy keyboard navigation should reveal the next logical row");
    cx.simulate_event(ScrollWheelEvent {
        position: row_one_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("table:virtual-focus-table:root"));

    cx.simulate_keystrokes("tab");
    assert!(cx.debug_selector_is_focused("virtual-focus-outside"));
    let table_bounds = cx
        .debug_bounds("table:virtual-focus-table:root")
        .expect("table root should remain rendered while its rows recycle");
    cx.simulate_event(ScrollWheelEvent {
        position: table_bounds.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(10_000.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("virtual-focus-outside"));
    assert!(!cx.debug_selector_is_focused(&TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0000"),
    )));

    let returned = cx
        .latest_accessibility_tree_update()
        .expect("returned focused row tree should publish");
    let (returned_row_zero_id, _) =
        node_with_role_and_row_index(&returned, accesskit::Role::Row, 2);
    assert_eq!(returned_row_zero_id, row_zero_id);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: row_zero_id,
        data: None,
    }));
    cx.run_until_parked();

    let row_zero_cell = cx
        .debug_bounds(&TableDebugSelector::cell(
            "virtual-focus-table",
            &TableRowIdentity::source_instance("duplicate", "row-0000"),
            &TableColumnId::new("name"),
        ))
        .expect("refocused first row should render");
    cx.simulate_event(ScrollWheelEvent {
        position: row_zero_cell.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("table:virtual-focus-table:root"));

    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        cx.debug_bounds(&TableDebugSelector::row(
            "virtual-focus-table",
            &TableRowIdentity::source_instance("duplicate", "row-0079"),
        ))
        .is_some()
    );
    assert!(cx.debug_selector_is_focused(&TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0079"),
    )));

    let ended = cx
        .latest_accessibility_tree_update()
        .expect("End navigation tree should publish");
    let (last_row_id, _) = node_with_role_and_row_index(&ended, accesskit::Role::Row, 81);
    assert_eq!(ended.focus, last_row_id);

    cx.simulate_keystrokes("space");
    cx.run_until_parked();
    assert_eq!(
        activations.borrow().as_slice(),
        &[TableRowIdentity::source_instance("duplicate", "row-0079")]
    );

    cx.simulate_keystrokes("home");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused(&TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0000"),
    )));

    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused(&TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0001"),
    )));

    cx.simulate_keystrokes("up");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused(&TableDebugSelector::row(
        "virtual-focus-table",
        &TableRowIdentity::source_instance("duplicate", "row-0000"),
    )));

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            TableRowIdentity::source_instance("duplicate", "row-0079"),
            TableRowIdentity::source_instance("duplicate", "row-0000"),
        ]
    );
}

#[open_gpui::test]
fn table_virtual_focus_proxy_preserves_claim_from_cell_editor_descendant(
    cx: &mut open_gpui::TestAppContext,
) {
    struct VirtualEditorFocusProbe;

    impl Render for VirtualEditorFocusProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..80).map(|index| {
                TableRow::new(format!("row-{index:04}"))
                    .with_cell("name", format!("Package {index:04}"))
            });
            let table = Table::new(
                "virtual-editor-focus-table",
                "Virtual editor focus table",
                TableState::new(rows)
                    .with_columns([TableColumn::new("name", "Name").with_text_editable(true)])
                    .with_pagination(TablePagination::disabled()),
            )
            .row_height(ui_px(32.0))
            .viewport_extent(ui_px(96.0))
            .overscan(0)
            .default_focused_row(TableRowIdentity::source("row-0000"))
            .on_cell_edit_change(|_, _, _| {});

            div()
                .size_full()
                .child(div().w(px(360.0)).h(px(144.0)).child(table))
        }
    }

    cx.update(init_text_input);
    let (_, cx) = cx.add_window_view(|_, _| VirtualEditorFocusProbe);
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let focused_editor_selector =
        table_source_text_input_editor_selector("virtual-editor-focus-table", "row-0001", "name");
    let focused_editor = cx
        .debug_bounds(&focused_editor_selector)
        .expect("second row editor should render inside the initial virtual window");
    cx.simulate_click(focused_editor.center(), Default::default());
    assert!(cx.debug_selector_is_focused(&focused_editor_selector));
    assert!(
        !cx.debug_selector_is_focused(&table_source_row_selector(
            "virtual-editor-focus-table",
            "row-0001",
        )),
        "the editor descendant, rather than the row focus handle itself, should own focus"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: focused_editor.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-320.0))),
        ..Default::default()
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds(&focused_editor_selector).is_none(),
        "scrolling should recycle the focused editor's row"
    );
    assert!(
        cx.debug_selector_is_focused("table:virtual-editor-focus-table:root"),
        "a focused editor descendant should preserve the table claim through proxy handoff"
    );

    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let expected_row = table_source_row_selector("virtual-editor-focus-table", "row-0002");
    let focused_selector = cx.focused_debug_selector();
    assert_eq!(focused_selector.as_deref(), Some(expected_row.as_str()));
}

#[open_gpui::test]
fn table_virtual_focus_honors_pending_navigation_when_previous_row_remains_rendered(
    cx: &mut open_gpui::TestAppContext,
) {
    struct OverlappingVirtualWindowProbe;

    impl Render for OverlappingVirtualWindowProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..20).map(|index| {
                TableRow::new(format!("row-{index:04}"))
                    .with_cell("name", format!("Package {index:04}"))
            });
            let table = Table::new(
                "overlapping-focus-table",
                "Overlapping focus table",
                TableState::new(rows)
                    .with_columns([TableColumn::new("name", "Name")])
                    .with_pagination(TablePagination::disabled()),
            )
            .row_height(ui_px(32.0))
            .viewport_extent(ui_px(56.0))
            .overscan(0)
            .default_focused_row(TableRowIdentity::source("row-0001"));

            div()
                .size_full()
                .child(div().w(px(360.0)).h(px(120.0)).child(table))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| OverlappingVirtualWindowProbe);
    assert!(cx.activate_accessibility());
    cx.update(|window, cx| window.draw(cx).clear());

    let pending_index = (1..20)
        .find(|index| {
            cx.debug_bounds(&table_source_row_selector(
                "overlapping-focus-table",
                format!("row-{index:04}"),
            ))
            .is_none()
        })
        .expect("the initial virtual window should leave at least one row unrendered");
    let previous_index = pending_index - 1;
    let previous_row = table_source_row_selector(
        "overlapping-focus-table",
        format!("row-{previous_index:04}"),
    );
    let pending_row =
        table_source_row_selector("overlapping-focus-table", format!("row-{pending_index:04}"));
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("the initial overlapping focus tree should publish");
    let (previous_row_node, _) =
        node_with_role_and_row_index(&initial, accesskit::Role::Row, previous_index + 2);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: previous_row_node,
        data: None,
    }));
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused(&previous_row));

    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(
        cx.debug_bounds(&previous_row).is_some(),
        "nearest reveal should keep the previous row partially rendered"
    );
    assert!(cx.debug_selector_is_focused(&pending_row));
}

#[open_gpui::test]
fn table_focus_falls_back_only_when_identity_leaves_the_final_model(
    cx: &mut open_gpui::TestAppContext,
) {
    struct FocusRemovalProbe {
        row_count: usize,
        outside_focus: FocusHandle,
    }

    impl Render for FocusRemovalProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut rows = Vec::new();
            if self.row_count >= 1 {
                rows.push(TableRow::new("alpha").with_cell("name", "Alpha"));
            }
            if self.row_count >= 2 {
                rows.push(TableRow::new("beta").with_cell("name", "Beta"));
            }
            let state = TableState::new(rows)
                .with_columns([TableColumn::new("name", "Name")])
                .with_pagination(TablePagination::disabled());

            div()
                .child(
                    div()
                        .id("focus-removal-outside")
                        .debug_selector(|| "focus-removal-outside".to_owned())
                        .role(accesskit::Role::Button)
                        .aria_label("Outside focus")
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.outside_focus)
                        .child("Outside"),
                )
                .child(
                    div().w(px(320.0)).h(px(120.0)).child(
                        Table::new("focus-removal-table", "Focus removal table", state)
                            .row_height(ui_px(24.0))
                            .viewport_extent(ui_px(72.0))
                            .default_focused_row(TableRowIdentity::source("beta")),
                    ),
                )
        }
    }

    let (view, cx) = cx.add_window_view(|_, cx| FocusRemovalProbe {
        row_count: 2,
        outside_focus: cx.focus_handle(),
    });
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial focus-removal tree should publish");
    let (outside_id, _) =
        node_with_role_and_label(&initial, accesskit::Role::Button, "Outside focus");
    let (alpha_id, _) = node_with_role_and_row_index(&initial, accesskit::Role::Row, 2);
    let (beta_id, _) = node_with_role_and_row_index(&initial, accesskit::Role::Row, 3);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action: accesskit::Action::Focus,
        target_tree: accesskit::TreeId::ROOT,
        target_node: beta_id,
        data: None,
    }));
    cx.run_until_parked();

    view.update(cx, |probe, cx| {
        probe.row_count = 1;
        cx.notify();
    });
    cx.run_until_parked();
    let fallback = cx
        .latest_accessibility_tree_update()
        .expect("fallback focus tree should publish");
    assert!(!fallback.nodes.iter().any(|(id, _)| *id == beta_id));
    assert_eq!(fallback.focus, alpha_id);
    assert!(
        cx.debug_selector_is_focused(&table_source_row_selector("focus-removal-table", "alpha",))
    );

    cx.update(|window, cx| {
        let outside_focus = view.read(cx).outside_focus.clone();
        outside_focus.focus(window, cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("focus-removal-outside"));
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("outside focus should publish")
            .focus,
        outside_id
    );

    view.update(cx, |probe, cx| {
        probe.row_count = 0;
        cx.notify();
    });
    cx.run_until_parked();
    let empty = cx
        .latest_accessibility_tree_update()
        .expect("empty focus-removal tree should publish");
    assert!(!empty.nodes.iter().any(|(id, _)| *id == alpha_id));
    assert_eq!(empty.focus, outside_id);
    assert!(cx.debug_selector_is_focused("focus-removal-outside"));
    assert!(
        !cx.debug_selector_is_focused(&table_source_row_selector("focus-removal-table", "alpha",))
    );
}
