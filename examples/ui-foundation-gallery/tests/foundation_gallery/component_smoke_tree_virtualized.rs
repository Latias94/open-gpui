use super::*;

#[open_gpui::test]
fn components_gallery_smoke_tree_expands_and_selects(cx: &mut open_gpui::TestAppContext) {
    const SAMPLE: &str = "gallery:component-tree-sample:document-outline";
    const PAPER: &str = "tree:component-tree:document-outline:item:paper";
    const INTRO: &str = "tree:component-tree:document-outline:item:intro";
    const NOTES: &str = "tree:component-tree:document-outline:item:notes";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_section(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, PAPER);
    assert!(
        cx.debug_bounds(INTRO).is_none(),
        "expected collapsed Tree descendants to stay hidden before expansion"
    );

    click(cx, PAPER);
    assert!(
        cx.debug_selector_is_focused(PAPER),
        "expected clicking a Tree row to focus that row for keyboard handling; focused={:?} paper={:?} viewport={:?}",
        cx.focused_debug_selector(),
        bounds(cx, PAPER),
        bounds(
            cx,
            "scroll-area:tree:component-tree:document-outline:scroll"
        )
    );
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    cx.simulate_keystrokes("right");
    redraw(cx);
    assert!(
        cx.debug_bounds(INTRO).is_some(),
        "expected the Paper branch to reveal its child after toggling open"
    );
    let toggles = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.toggles()
            .iter()
            .map(|toggle| {
                (
                    toggle.sample_id.clone(),
                    toggle.value.clone(),
                    toggle.expanded,
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(
        toggles,
        vec![("document-outline".to_owned(), "paper".to_owned(), true)],
        "expected right arrow to expand the focused root branch"
    );

    cx.simulate_keystrokes("down");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(INTRO),
        "expected Down to move focus to the newly revealed child row"
    );

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let selections = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.selections()
            .iter()
            .map(|selection| (selection.sample_id.clone(), selection.value.clone()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        selections,
        vec![("document-outline".to_owned(), "intro".to_owned())],
        "expected Enter to select the focused child row"
    );

    cx.simulate_keystrokes("n o");
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(NOTES),
        "expected Tree typeahead to focus the visible Notes row; focused={:?}",
        cx.focused_debug_selector()
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_drag_updates_sample(cx: &mut open_gpui::TestAppContext) {
    const CHILD: &str = "tree:component-tree:editable-outline:item:child";
    const PEER: &str = "tree:component-tree:editable-outline:item:peer";
    const SIBLING: &str = "tree:component-tree:editable-outline:item:sibling";
    const DROP: &str = "tree:component-tree:editable-outline:drop:before:sibling";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, CHILD);
    scroll_page_selector_into_view(&shell, cx, DROP);
    let child_before = visible_page_interaction_point(cx, CHILD);
    let peer_before = visible_page_interaction_point(cx, PEER);
    let sibling_before = visible_page_interaction_point(cx, SIBLING);
    let drop_before = visible_page_interaction_point(cx, DROP);
    assert!(
        child_before.y < peer_before.y,
        "expected child row to render above peer before drag"
    );
    assert!(peer_before.y < sibling_before.y);

    cx.simulate_click(child_before, Default::default());
    redraw(cx);
    let selections = cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.selections().to_vec()
    });
    assert_eq!(
        selections.len(),
        1,
        "expected the editable Tree row to accept a normal click before dragging"
    );
    assert_eq!(selections[0].sample_id, "editable-outline");
    assert_eq!(selections[0].value, "child");
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    cx.simulate_mouse_down(child_before, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(child_before.x + px(18.0), child_before.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(child_before.x + px(42.0), child_before.y + px(2.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(drop_before, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(drop_before, MouseButton::Left, Default::default());
    cx.run_until_parked();
    redraw(cx);

    let moves =
        cx.read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| log.moves().to_vec());
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].sample_id, "editable-outline");
    assert_eq!(moves[0].tree_move.value(), "child");
    assert_eq!(moves[0].tree_move.source_parent_value(), Some("root"));
    assert_eq!(
        moves[0].tree_move.position(),
        open_gpui_ui_components::TreeDropPosition::Before
    );
    assert_eq!(moves[0].tree_move.target().target_value(), "sibling");
    assert_eq!(moves[0].tree_move.target_parent_value(), None);
    assert_eq!(moves[0].tree_move.sibling_anchor_value(), Some("sibling"));

    redraw(cx);
    let child_after = bounds(cx, CHILD).center();
    let peer_after = bounds(cx, PEER).center();
    let sibling_after = bounds(cx, SIBLING).center();
    assert!(
        child_after.y > peer_after.y,
        "expected child row to move below peer after a before-drop move"
    );
    assert!(peer_after.y < sibling_after.y);
}

#[open_gpui::test]
fn components_gallery_smoke_tree_lazy_branches_emit_load_metadata(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:remote-workspace";
    const UNLOADED_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-src";
    const LOADING_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-crates";
    const FAILED_TOGGLE: &str = "tree:component-tree:remote-workspace:toggle:remote-build";
    const UNLOADED_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-src";
    const LOADING_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-crates";
    const FAILED_ITEM: &str = "tree:component-tree:remote-workspace:item:remote-build";
    const LOADING_HINT: &str = "tree:component-tree:remote-workspace:load-state:remote-crates";
    const FAILED_HINT: &str = "tree:component-tree:remote-workspace:load-state:remote-build";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::TreeSampleRuntimeLog::default());
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, UNLOADED_ITEM);
    assert!(
        cx.debug_bounds(UNLOADED_TOGGLE).is_some(),
        "expected unloaded remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(LOADING_TOGGLE).is_some(),
        "expected loading remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(FAILED_TOGGLE).is_some(),
        "expected failed remote branch to render a disclosure affordance"
    );
    assert!(
        cx.debug_bounds(LOADING_HINT).is_some(),
        "expected loading branch to expose a visible load-state hint"
    );
    assert!(
        cx.debug_bounds(FAILED_HINT).is_some(),
        "expected failed branch to expose a visible load-state hint"
    );
    cx.update_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    click(cx, UNLOADED_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(UNLOADED_ITEM),
        "expected unloaded branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);
    click(cx, LOADING_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(LOADING_ITEM),
        "expected loading branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);
    click(cx, FAILED_ITEM);
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(FAILED_ITEM),
        "expected failed branch row to receive focus before Right; focused={:?}",
        cx.focused_debug_selector()
    );
    cx.simulate_keystrokes("right");
    redraw(cx);

    let toggles = cx
        .read_global::<pages::components::TreeSampleRuntimeLog, _>(|log, _| log.toggles().to_vec());
    assert_eq!(
        toggles.len(),
        2,
        "expected unloaded and failed branches to toggle while loading branch is blocked; toggles={toggles:?}"
    );
    assert_eq!(toggles[0].sample_id, "remote-workspace");
    assert_eq!(toggles[0].value, "remote-src");
    assert!(toggles[0].expanded);
    assert_eq!(toggles[0].loaded_child_count, 0);
    assert_eq!(toggles[0].children_load_state, "unloaded");
    assert_eq!(toggles[0].children_load_message, None);
    assert_eq!(toggles[1].sample_id, "remote-workspace");
    assert_eq!(toggles[1].value, "remote-build");
    assert!(toggles[1].expanded);
    assert_eq!(toggles[1].loaded_child_count, 0);
    assert_eq!(toggles[1].children_load_state, "failed");
    assert_eq!(
        toggles[1].children_load_message.as_deref(),
        Some("Network unavailable")
    );
}

#[open_gpui::test]
fn components_gallery_smoke_tree_card_wheel_does_not_leak_to_page(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:document-outline";
    const VIEWPORT: &str = "scroll-area:tree:component-tree:document-outline:scroll";
    const ITEM: &str = "tree:component-tree:document-outline:item:appendix-01";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);

    jump_components_directory_to(cx, "gallery:component-page-jump:tree");
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    scroll_page_selector_into_view(&shell, cx, VIEWPORT);
    let sample_before = bounds(cx, SAMPLE);
    let item_before = bounds(cx, ITEM);
    let viewport_position = visible_page_interaction_point(cx, VIEWPORT);

    cx.simulate_event(ScrollWheelEvent {
        position: viewport_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(cx, SAMPLE);
    let item_after = bounds(cx, ITEM);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected Tree card wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        item_after.top() < item_before.top(),
        "expected Tree card wheel input to move the inner viewport; before={item_before:?} after={item_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_tree_scrolls_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-tree-sample:release-outline";
    const ROOT: &str = "tree:component-tree:release-outline:item:release-node-0000";
    const LAST: &str = "tree:component-tree:release-outline:item:release-node-0239";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    let tree_entry = pages::components::COMPONENT_CATALOG
        .iter()
        .find(|entry| entry.name == "Tree")
        .unwrap_or_else(|| panic!("expected catalog entry `Tree`"));
    focus_components_catalog_entry(&shell, cx, tree_entry);

    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    let sample_before = bounds(cx, SAMPLE);
    assert!(cx.debug_bounds(ROOT).is_some());
    assert!(cx.debug_bounds(LAST).is_none());

    click(cx, ROOT);
    redraw(cx);
    cx.simulate_keystrokes("end");
    redraw(cx);
    let sample_after = bounds(cx, SAMPLE);

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected virtualized Tree keyboard navigation to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds(LAST).is_none(),
        "expected the far Tree row to remain outside the initial render window after End"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_scroll_stays_inside_sample(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let sample_before = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000"
        )
        .is_some(),
        "expected the initial VirtualizedList window to render the first row"
    );
    let row_0 = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0016"
        )
        .is_none(),
        "expected row 0016 to start outside the initial rendered window"
    );
    let row_0_before = row_0;
    cx.simulate_event(ScrollWheelEvent {
        position: point(
            sample_before.left() + px(24.0),
            sample_before.top() + px(24.0),
        ),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-56.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_chrome_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_0_after = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );
    assert_eq!(
        sample_chrome_after.top(),
        sample_before.top(),
        "expected VirtualizedList card chrome wheel input to stay inside the sample card; before={sample_before:?} after={sample_chrome_after:?}"
    );
    assert_eq!(
        row_0_after.top(),
        row_0_before.top(),
        "expected VirtualizedList card chrome wheel input to leave the rendered window unchanged; before={row_0_before:?} after={row_0_after:?}"
    );

    click(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:root",
    );
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(
            "virtualized-list:component-virtualized-list:release-navigation:root"
        ),
        "expected the VirtualizedList root to own focus after clicking a row"
    );
    cx.simulate_keystrokes("pagedown");
    redraw(cx);
    cx.simulate_keystrokes("pagedown");
    redraw(cx);

    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0016"
        )
        .is_some(),
        "expected repeated PageDown to reveal row 0016 inside the sample"
    );

    let viewport = bounds(
        cx,
        "scroll-area:virtualized-list:component-virtualized-list:release-navigation:viewport",
    );
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected VirtualizedList viewport wheel input to stay inside the sample card; before={sample_before:?} after={sample_after:?}"
    );
    assert!(
        cx.debug_bounds(
            "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0025"
        )
        .is_some(),
        "expected virtualized list row 0025 to enter the rendered window after keyboard and wheel scroll"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = open_components_gallery(cx);

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let sample_before = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_before = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(
            sample_before.left() + px(24.0),
            sample_before.top() + px(24.0),
        ),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    redraw(cx);

    let sample_after = bounds(
        cx,
        "gallery:component-virtualized-list-sample:release-navigation",
    );
    let row_after = bounds(
        cx,
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0000",
    );

    assert_eq!(
        sample_after.top(),
        sample_before.top(),
        "expected VirtualizedList card chrome wheel input to stay local to the sample; before={sample_before:?} after={sample_after:?}"
    );
    assert_eq!(
        row_after, row_before,
        "expected VirtualizedList card chrome wheel input to leave the inner viewport unchanged; before={row_before:?} after={row_after:?}"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-virtualized-list-sample:release-navigation";
    const ROOT: &str = "virtualized-list:component-virtualized-list:release-navigation:root";
    const ROW_8: &str =
        "virtualized-list:component-virtualized-list:release-navigation:row:release-nav-0008";

    let cx = open_components_gallery(cx);
    cx.set_global(pages::components::VirtualizedListSampleRuntimeLog::default());

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_until_visible(cx, SAMPLE);
    click(cx, ROOT);
    assert!(
        cx.debug_selector_is_focused(ROOT),
        "expected clicking a VirtualizedList row to focus the list root for keyboard handling"
    );
    cx.update_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    let row_8_before = bounds(cx, ROW_8);
    cx.simulate_keystrokes("pagedown");
    redraw(cx);
    redraw(cx);
    let queued_callbacks = cx.update(|window, cx| window.drain_next_frame_callbacks_for_test(cx));
    assert!(
        queued_callbacks > 0,
        "expected the deferred VirtualizedList reveal to submit after target binding"
    );
    redraw(cx);
    cx.run_until_parked();
    redraw(cx);

    let row_8_after = bounds(cx, ROW_8);
    assert!(
        row_8_after.top() < row_8_before.top(),
        "expected PageDown to reveal the next active VirtualizedList row; before={row_8_before:?} after={row_8_after:?}"
    );

    cx.simulate_keystrokes("enter");
    redraw(cx);
    let enter_activations = cx
        .read_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
            log.activations()
                .iter()
                .map(|activation| (activation.sample_id.clone(), activation.index))
                .collect::<Vec<_>>()
        });
    assert_eq!(enter_activations.len(), 1);
    assert_eq!(enter_activations[0].0, "release-navigation");
    let activated_index = enter_activations[0].1;
    assert!(
        activated_index >= 8,
        "expected Enter to activate the row revealed by PageDown; activations={enter_activations:?}"
    );

    cx.simulate_keystrokes("space");
    redraw(cx);
    let activations =
        cx.read_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
            log.activations()
                .iter()
                .map(|activation| (activation.sample_id.clone(), activation.index))
                .collect::<Vec<_>>()
        });
    assert_eq!(
        activations,
        vec![
            ("release-navigation".to_owned(), activated_index),
            ("release-navigation".to_owned(), activated_index),
        ],
        "expected Space to activate the same active row after Enter"
    );
}

#[open_gpui::test]
fn components_gallery_smoke_virtualized_list_host_reveal_and_nested_action(
    cx: &mut open_gpui::TestAppContext,
) {
    const SAMPLE: &str = "gallery:component-virtualized-list-sample:host-controlled-actions";
    const ROOT: &str = "virtualized-list:component-virtualized-list:host-controlled-actions:root";
    const REVEALED_ROW: &str =
        "virtualized-list:component-virtualized-list:host-controlled-actions:row:host-action-0010";
    const ROW_ACTION: &str = "virtualized-list:component-virtualized-list:host-controlled-actions:row-action:host-action-0010";

    let (shell, cx) = open_gallery_page_with_shell(cx, GalleryPage::Components);
    cx.set_global(pages::components::VirtualizedListSampleRuntimeLog::default());

    jump_components_directory_to(cx, "gallery:component-page-jump:virtualized-list");
    scroll_page_selector_into_view(&shell, cx, SAMPLE);
    redraw(cx);
    redraw(cx);

    assert!(
        cx.debug_bounds(REVEALED_ROW).is_some(),
        "expected host-controlled reveal to mount row host-action-0010"
    );
    scroll_page_selector_into_view(&shell, cx, ROW_ACTION);
    redraw(cx);

    let revealed_row = bounds(cx, REVEALED_ROW);
    let row_point = visible_page_interaction_point(cx, REVEALED_ROW);
    click_point(cx, point(revealed_row.left() + px(12.0), row_point.y));
    redraw(cx);
    assert!(
        cx.debug_selector_is_focused(ROOT),
        "expected clicking a visible host-controlled row to focus the list"
    );
    scroll_page_selector_into_view(&shell, cx, ROW_ACTION);
    redraw(cx);
    cx.update_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
        log.clear();
    });

    let row_action = bounds(cx, ROW_ACTION);
    let row_action_point = visible_page_interaction_point(cx, ROW_ACTION);
    cx.simulate_mouse_move(row_action_point, None, Default::default());
    redraw(cx);
    click_point(cx, row_action_point);
    redraw(cx);
    let (nested_actions, activations) = cx
        .read_global::<pages::components::VirtualizedListSampleRuntimeLog, _>(|log, _| {
            (
                log.nested_actions()
                    .iter()
                    .map(|action| (action.sample_id.clone(), action.key.clone()))
                    .collect::<Vec<_>>(),
                log.activations()
                    .iter()
                    .map(|activation| (activation.sample_id.clone(), activation.key.clone()))
                    .collect::<Vec<_>>(),
            )
        });

    assert_eq!(
        nested_actions,
        vec![(
            "host-controlled-actions".to_owned(),
            "host-action-0010".to_owned()
        )],
        "expected nested Button click to record the row action; row_action={row_action:?}"
    );
    assert!(
        activations.is_empty(),
        "nested Button click should not activate the containing VirtualizedList row; activations={activations:?}"
    );
}
