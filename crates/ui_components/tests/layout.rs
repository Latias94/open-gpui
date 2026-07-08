use open_gpui::{
    Context, InteractiveElement, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, Styled, Window, div, point, px,
};
use open_gpui_ui_components::{
    Button, ScrollArea, ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy, Splitter,
    SplitterPanel, SplitterPanelDescriptor, SplitterState, Tree, TreeChildrenLoadState,
    TreeDropPosition, TreeItemDescriptor, TreeMove, TreeMoveTarget, VirtualizedList,
    VirtualizedListActivation, VirtualizedListItemDescriptor, VirtualizedListRowRenderContext,
    VirtualizedListScrollStrategy, VirtualizedListSelectionMode, apply_tree_move,
    gpui_adapter::VirtualizedListGpuiExt,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, VirtualizerRange, ui_px};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn scroll_area_state_exposes_axis_metrics_and_reset_policy() {
    let state = ScrollAreaState::resolve(
        "activity-log",
        ScrollAreaAxis::Both,
        Size::Small,
        ScrollResetPolicy::ResetOnKeyChange,
        Some("components".to_string()),
    );

    assert_eq!(state.viewport_id(), "activity-log");
    assert_eq!(state.axis(), ScrollAreaAxis::Both);
    assert_eq!(state.axis().as_str(), "both");
    assert_eq!(state.size(), Size::Small);
    assert!(state.scrolls_x());
    assert!(state.scrolls_y());
    assert_eq!(state.reset_policy(), ScrollResetPolicy::ResetOnKeyChange);
    assert_eq!(state.reset_policy().as_str(), "reset-on-key-change");
    assert_eq!(state.reset_key(), Some("components"));
    assert_eq!(state.metrics().scrollbar_width(), ui_px(8.0));
    assert!(state.should_reset_for_key_change(Some("tokens")));
    assert!(!state.should_reset_for_key_change(Some("components")));
    assert!(!state.should_reset_for_key_change(None));
}

#[test]
fn scroll_area_builder_state_keeps_gpui_handle_out_of_resolved_state() {
    let external_handle = open_gpui::ScrollHandle::new();
    let state = ScrollArea::new("component-scroll", div())
        .horizontal()
        .large()
        .reset_on_key("settings")
        .state();
    let preserved = ScrollArea::new("preserved-scroll", div())
        .both()
        .scroll_handle(&external_handle)
        .preserve_scroll()
        .state();

    assert_eq!(state.viewport_id(), "component-scroll");
    assert_eq!(state.axis(), ScrollAreaAxis::Horizontal);
    assert!(state.scrolls_x());
    assert!(!state.scrolls_y());
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().scrollbar_width(), ui_px(12.0));
    assert_eq!(state.reset_key(), Some("settings"));
    assert!(state.should_reset_for_key_change(Some("overview")));
    assert_eq!(preserved.reset_policy(), ScrollResetPolicy::Preserve);
    assert_eq!(preserved.reset_key(), None);
    assert!(!preserved.should_reset_for_key_change(Some("overview")));
}

#[test]
fn virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts() {
    let items = (0..10_000)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = VirtualizedList::new("contracts-list", "Contracts list", items)
        .with_size(Size::Small)
        .default_active_key("item-0104")
        .default_selected_key("item-0101")
        .viewport_item_count(7)
        .behavior_snapshot_with_viewport(ui_px(2_800.0), ui_px(196.0));

    assert_eq!(snapshot.role(), Role::ListBox);
    assert_eq!(snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(snapshot.state().item_count(), 10_000);
    assert_eq!(snapshot.total_size(), ui_px(280_000.0));
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(100, 107));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(98, 109));
    assert_eq!(snapshot.visible_row_count(), 7);
    assert_eq!(snapshot.rendered_row_count(), 11);
    assert_eq!(snapshot.rows()[0].index(), 98);
    assert_eq!(snapshot.rows()[0].render_key(), "item-0098");

    let active_row = snapshot
        .active_row()
        .expect("active row should be rendered");
    assert_eq!(active_row.index(), 104);
    assert_eq!(active_row.key(), "item-0104");
    assert_eq!(active_row.label(), "Item 0104");
    assert!(active_row.active());
    assert!(!active_row.selected());
    assert_eq!(active_row.role(), Role::ListBoxOption);
    assert_eq!(active_row.position_in_set(), Some(105));
    assert_eq!(active_row.size_of_set(), 10_000);
    assert_eq!(active_row.virtual_start(), ui_px(2_912.0));
    assert_eq!(active_row.virtual_size(), ui_px(28.0));

    let selected_row = snapshot
        .selected_row()
        .expect("selected row should be rendered");
    assert_eq!(selected_row.index(), 101);
    assert!(selected_row.selected());

    let activation =
        VirtualizedListActivation::new(active_row.index(), active_row.key(), active_row.label());
    assert_eq!(activation.index(), 104);
    assert_eq!(activation.key(), "item-0104");
    let reveal = snapshot.state().scroll_target_for_key(
        activation.key(),
        VirtualizedListScrollStrategy::Top,
        snapshot.viewport_extent(),
        snapshot.scroll_offset(),
    );
    assert_eq!(
        reveal,
        open_gpui_ui_components::VirtualizedListRevealResult::Revealed(
            open_gpui_ui_components::VirtualizedListRevealTarget::new(
                activation.key(),
                activation.index(),
                ui_px(2_912.0),
                false,
            ),
        )
    );
}

#[test]
fn virtualized_list_behavior_snapshot_applies_builder_metrics() {
    let items = (0..32)
        .map(|index| {
            VirtualizedListItemDescriptor::new(
                format!("item-{index:04}"),
                format!("Item {index:04}"),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = VirtualizedList::new("builder-list", "Builder list", items)
        .with_size(Size::Small)
        .row_height(ui_px(24.0))
        .overscan(2)
        .default_active_key("item-0005")
        .default_selected_key("item-0003")
        .viewport_item_count(4)
        .behavior_snapshot_with_viewport(ui_px(48.0), ui_px(96.0));

    assert_eq!(snapshot.metrics().row_height(), ui_px(24.0));
    assert_eq!(snapshot.overscan_count(), 2);
    assert_eq!(snapshot.visible_row_count(), 4);
    assert_eq!(*snapshot.visible_range(), VirtualizerRange::new(2, 6));
    assert_eq!(*snapshot.overscan_range(), VirtualizerRange::new(1, 7));
    assert_eq!(snapshot.active_row().map(|row| row.index()), Some(5));
    assert_eq!(snapshot.selected_row().map(|row| row.index()), Some(3));
}

#[open_gpui::test]
fn tree_runtime_expands_reveals_and_selects_items(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
        toggles: Rc<RefCell<Vec<(String, bool)>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let toggles = self.toggles.clone();
            let tree = Tree::new(
                "runtime-tree",
                "Runtime tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("intro", "Introduction"))
                        .child(
                            TreeItemDescriptor::new("figures", "Figures")
                                .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
                        ),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_toggle(move |toggle, _, _| {
                toggles
                    .borrow_mut()
                    .push((toggle.value().to_owned(), toggle.expanded()));
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
        toggles: toggles.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tree:runtime-tree:item:paper").is_some(),
        "expected the root tree item to render before expansion"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_none(),
        "expected collapsed descendants to stay hidden before expansion"
    );

    let root = cx
        .debug_bounds("tree:runtime-tree:root")
        .expect("tree root should render as a focusable interaction region");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("tree:runtime-tree:item:paper"),
        "clicking Tree chrome should focus the current roving item for keyboard navigation"
    );

    let paper = cx
        .debug_bounds("tree:runtime-tree:item:paper")
        .expect("paper row should be visible");
    cx.simulate_click(paper.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    selections.borrow_mut().clear();

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        toggles.borrow().as_slice(),
        [("paper".to_owned(), true)],
        "expected right arrow to expand the focused root branch"
    );
    assert!(
        cx.debug_bounds("tree:runtime-tree:item:intro").is_some(),
        "expected expanded descendants to render after toggling open"
    );

    cx.simulate_keystrokes("down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(selections.borrow().as_slice(), ["intro".to_owned()]);
}

#[open_gpui::test]
fn tree_runtime_typeahead_focuses_visible_matching_row(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-typeahead-tree",
                "Runtime typeahead tree",
                vec![
                    TreeItemDescriptor::new("paper", "Paper")
                        .child(TreeItemDescriptor::new("figures", "Figures")),
                    TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
                    TreeItemDescriptor::new("notes", "Notes"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("paper")
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            });

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(180.0)).child(tree))
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("tree:runtime-typeahead-tree:root")
        .expect("tree root should render");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Default::default(),
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("n o");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_selector_is_focused("tree:runtime-typeahead-tree:item:notes"),
        "expected typeahead to focus the visible Notes row; focused={:?}",
        cx.focused_debug_selector()
    );
    assert!(
        selections.borrow().is_empty(),
        "typeahead should move focus without selecting a row"
    );
}

#[open_gpui::test]
fn tree_runtime_drag_move_emits_controlled_payload(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        moves: Rc<RefCell<Vec<TreeMove>>>,
        selections: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let moves = self.moves.clone();
            let selections = self.selections.clone();
            let tree = Tree::new(
                "runtime-drag-tree",
                "Runtime drag tree",
                vec![
                    TreeItemDescriptor::new("root", "Root")
                        .expanded(true)
                        .child(TreeItemDescriptor::new("child", "Child"))
                        .child(TreeItemDescriptor::new("peer", "Peer")),
                    TreeItemDescriptor::new("sibling", "Sibling"),
                ],
            )
            .with_size(Size::Small)
            .default_focused("root")
            .draggable(true)
            .on_select(move |selection, _, _| {
                selections.borrow_mut().push(selection.value().to_owned());
            })
            .on_move(move |tree_move, _, _| {
                moves.borrow_mut().push(tree_move);
            });

            div()
                .size_full()
                .child(div().w(px(320.0)).h(px(220.0)).child(tree))
        }
    }

    let moves = Rc::new(RefCell::new(Vec::new()));
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        moves: moves.clone(),
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let child = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("expanded child row should render");
    cx.simulate_click(child.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().as_slice(),
        ["child".to_owned()],
        "enabling tree drag affordances should not break regular row clicks"
    );
    assert!(
        moves.borrow().is_empty(),
        "regular clicks should not emit controlled tree moves"
    );
    selections.borrow_mut().clear();

    let source = cx
        .debug_bounds("tree:runtime-drag-tree:item:child")
        .expect("child row should remain rendered")
        .center();
    let target = cx
        .debug_bounds("tree:runtime-drag-tree:drop:before:sibling")
        .expect("before-sibling drop zone should render")
        .center();

    cx.simulate_mouse_down(source, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(source.x + px(18.0), source.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let moves = moves.borrow();
    assert_eq!(
        moves.len(),
        1,
        "expected one controlled move after dropping child before sibling"
    );
    let tree_move = &moves[0];
    assert_eq!(tree_move.value(), "child");
    assert_eq!(tree_move.label(), "Child");
    assert_eq!(tree_move.source_parent_value(), Some("root"));
    assert_eq!(tree_move.position(), TreeDropPosition::Before);
    assert_eq!(tree_move.target().target_value(), "sibling");
    assert_eq!(tree_move.target_parent_value(), None);
    assert_eq!(tree_move.sibling_anchor_value(), Some("sibling"));
    assert!(
        selections.borrow().is_empty(),
        "drag drops should not also emit row selections"
    );
}

#[test]
fn feedback_tree_and_virtualized_list_public_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_status_cue: root::StatusCue = root::StatusCue::new("status", "Ready");
    let prelude_status_cue: prelude::StatusCue = prelude::StatusCue::new("status", "Ready");
    let root_empty_state: root::EmptyState = root::EmptyState::new("empty", "No results");
    let prelude_empty_state: prelude::EmptyState = prelude::EmptyState::new("empty", "No results");
    let root_tree_descriptor: root::TreeItemDescriptor =
        root::TreeItemDescriptor::new("root", "Root")
            .with_children_unloaded()
            .child(root::TreeItemDescriptor::new("child", "Child"));
    let prelude_tree_descriptor: prelude::TreeItemDescriptor =
        prelude::TreeItemDescriptor::new("root", "Root").with_children_load_failed("Offline");
    let root_tree_load_state: root::TreeChildrenLoadState =
        root::TreeChildrenLoadState::loading("Loading children");
    let prelude_tree_load_state: prelude::TreeChildrenLoadState =
        prelude::TreeChildrenLoadState::unloaded();
    let direct_tree_load_state: TreeChildrenLoadState = TreeChildrenLoadState::loaded();
    let root_tree: root::Tree =
        root::Tree::new("root-tree", "Root tree", [root_tree_descriptor.clone()])
            .default_selected("root")
            .default_focused("root")
            .virtualized(true)
            .viewport_item_count(2)
            .overscan_count(1);
    let prelude_tree: prelude::Tree = prelude::Tree::new(
        "prelude-tree",
        "Prelude tree",
        [prelude::TreeItemDescriptor::new("root", "Root")],
    )
    .default_focused("root")
    .virtualized(true)
    .viewport_item_count(2)
    .overscan_count(1);
    let root_tree_state: root::TreeState = root::TreeState::resolve(
        Size::Medium,
        "Tree",
        None,
        None,
        [root_tree_descriptor.clone()],
    );
    let prelude_tree_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Tree", None, None, [prelude_tree_descriptor]);
    let move_items = [
        root::TreeItemDescriptor::new("root", "Root")
            .expanded(true)
            .child(root::TreeItemDescriptor::new("child", "Child")),
        root::TreeItemDescriptor::new("sibling", "Sibling"),
    ];
    let move_state: root::TreeState =
        root::TreeState::resolve(Size::Medium, "Move tree", None, None, move_items.clone());
    let root_tree_move: root::TreeMove = move_state
        .move_for_drop("child", "sibling", root::TreeDropPosition::Before)
        .expect("public Tree move payload should resolve");
    let _root_tree_move_target: &root::TreeMoveTarget = root_tree_move.target();
    let prelude_tree_position: prelude::TreeDropPosition = prelude::TreeDropPosition::Inside;
    let _direct_tree_move: TreeMove = root_tree_move.clone();
    let _direct_tree_move_target: &TreeMoveTarget = root_tree_move.target();
    let moved_tree = root::apply_tree_move(move_items, &root_tree_move)
        .expect("public apply_tree_move helper should apply valid payload");
    let _direct_moved_tree = apply_tree_move(moved_tree.clone(), &root_tree_move);
    let prelude_move_state: prelude::TreeState =
        prelude::TreeState::resolve(Size::Medium, "Move tree", None, None, moved_tree);
    let root_virtualized_state: root::VirtualizedListState = root::VirtualizedListState::resolve(
        Size::Small,
        false,
        (0..12).map(|index| {
            root::VirtualizedListStateItem::new(
                format!("root-item-{index}"),
                format!("Root item {index}"),
            )
        }),
        Some("root-item-4"),
        ["root-item-4"],
        root::VirtualizedListSelectionMode::Single,
        Some(3),
    );
    let prelude_virtualized_state: prelude::VirtualizedListState =
        prelude::VirtualizedListState::resolve(
            Size::Small,
            false,
            (0..12).map(|index| {
                prelude::VirtualizedListStateItem::new(
                    format!("prelude-item-{index}"),
                    format!("Prelude item {index}"),
                )
            }),
            Some("prelude-item-4"),
            ["prelude-item-4"],
            prelude::VirtualizedListSelectionMode::Single,
            Some(3),
        );
    let root_virtualized_items = (0..12)
        .map(|index| {
            root::VirtualizedListItemDescriptor::new(
                format!("root-item-{index}"),
                format!("Root item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let root_virtualized_list: root::VirtualizedList = root::VirtualizedList::new(
        "root-virtualized-component",
        "Root virtualized component",
        root_virtualized_items.clone(),
    )
    .with_size(Size::Small)
    .default_active_key("root-item-4")
    .default_selected_key("root-item-4")
    .row_measure_mode(root::VirtualizedListRowMeasureMode::Fixed)
    .render_row(|context: root::VirtualizedListRowRenderContext, _, _| {
        div().px(px(4.0)).child(context.label().to_owned())
    })
    .viewport_item_count(3);
    let prelude_virtualized_items = (0..12)
        .map(|index| {
            prelude::VirtualizedListItemDescriptor::new(
                format!("prelude-item-{index}"),
                format!("Prelude item {index}"),
            )
        })
        .collect::<Vec<_>>();
    let prelude_virtualized_list: prelude::VirtualizedList = prelude::VirtualizedList::new(
        "prelude-virtualized-component",
        "Prelude virtualized component",
        prelude_virtualized_items.clone(),
    )
    .with_size(Size::Small)
    .default_active_key("prelude-item-4")
    .default_selected_key("prelude-item-4")
    .row_measure_mode(prelude::VirtualizedListRowMeasureMode::Fixed)
    .render_row(|context: prelude::VirtualizedListRowRenderContext, _, _| {
        div().px(px(4.0)).child(context.label().to_owned())
    })
    .viewport_item_count(3);
    let root_virtualized_snapshot: root::VirtualizedListBehaviorSnapshot =
        root_virtualized_list.behavior_snapshot_with_viewport(ui_px(28.0), ui_px(56.0));
    let prelude_virtualized_snapshot: prelude::VirtualizedListBehaviorSnapshot =
        prelude_virtualized_list.behavior_snapshot_with_viewport(ui_px(28.0), ui_px(56.0));
    let _root_virtualized_row: &root::VirtualizedListRowBehaviorSnapshot =
        root_virtualized_snapshot.active_row().unwrap();
    let _prelude_virtualized_row: &prelude::VirtualizedListRowBehaviorSnapshot =
        prelude_virtualized_snapshot.active_row().unwrap();
    let root_virtualized_component_state = root_virtualized_list.state();
    let prelude_virtualized_component_state = prelude_virtualized_list.state();
    let root_tree_component_state = root_tree.state();
    let root_tree_component_snapshot: root::TreeBehaviorSnapshot =
        root_tree.behavior_snapshot(ui_px(0.0), ui_px(32.0));
    let prelude_tree_component_state = prelude_tree.state();
    let prelude_tree_component_snapshot: prelude::TreeBehaviorSnapshot =
        prelude_tree.behavior_snapshot(ui_px(0.0), ui_px(32.0));
    let _root_tree_row: &root::TreeRowBehaviorSnapshot =
        root_tree_component_snapshot.rows().first().unwrap();
    let _prelude_tree_row: &prelude::TreeRowBehaviorSnapshot =
        prelude_tree_component_snapshot.rows().first().unwrap();
    let _root_tree_toggle: Option<root::TreeToggle> =
        root::TreeToggle::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_toggle: Option<prelude::TreeToggle> =
        prelude::TreeToggle::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_selection: Option<root::TreeSelection> =
        root::TreeSelection::from_item(&root_tree_state.items()[0]);
    let _prelude_tree_selection: Option<prelude::TreeSelection> =
        prelude::TreeSelection::from_item(&prelude_tree_state.items()[0]);
    let _root_tree_focus: root::TreeFocusTarget = root::TreeFocusTarget::new(0, "root");
    let _prelude_tree_focus: prelude::TreeFocusTarget = prelude::TreeFocusTarget::new(0, "root");
    let _root_tree_action: Option<root::TreeKeyboardAction> =
        root_tree_state.keyboard_action_for_key("right");
    let _prelude_tree_action: Option<prelude::TreeKeyboardAction> =
        prelude_tree_state.keyboard_action_for_key("right");
    let _root_virtualized_activation: root::VirtualizedListActivation =
        root::VirtualizedListActivation::new(4, "root-item-4", "Root item 4");
    let _prelude_virtualized_activation: prelude::VirtualizedListActivation =
        prelude::VirtualizedListActivation::new(4, "prelude-item-4", "Prelude item 4");
    let _root_virtualized_state_item: root::VirtualizedListStateItem =
        root::VirtualizedListStateItem::new("root-item", "Root item");
    let _prelude_virtualized_state_item: prelude::VirtualizedListStateItem =
        prelude::VirtualizedListStateItem::new("prelude-item", "Prelude item");
    let _direct_virtualized_context_type: Option<VirtualizedListRowRenderContext> = None;
    let _root_virtualized_colors: root::VirtualizedListColors =
        root::VirtualizedListColors::from_tokens(open_gpui_ui_core::ThemeTokens::default());
    let _prelude_virtualized_colors: prelude::VirtualizedListColors =
        prelude::VirtualizedListColors::from_tokens(open_gpui_ui_core::ThemeTokens::default());
    let _root_virtualized_overlay: Option<&root::VirtualizedListStickyOverlaySnapshot> =
        root_virtualized_snapshot.sticky_overlay();
    let _prelude_virtualized_overlay: Option<&prelude::VirtualizedListStickyOverlaySnapshot> =
        prelude_virtualized_snapshot.sticky_overlay();
    let root_virtualized_row_kind: root::VirtualizedListRowKind =
        root::VirtualizedListRowKind::Item;
    let prelude_virtualized_row_kind: prelude::VirtualizedListRowKind =
        prelude::VirtualizedListRowKind::Section;
    let root_virtualized_status_kind: root::VirtualizedListStatusKind =
        root::VirtualizedListStatusKind::AppendLoading;
    let prelude_virtualized_status_kind: prelude::VirtualizedListStatusKind =
        prelude::VirtualizedListStatusKind::Retry;
    let root_virtualized_measure_mode: root::VirtualizedListRowMeasureMode =
        root::VirtualizedListRowMeasureMode::Measured;
    let prelude_virtualized_measure_mode: prelude::VirtualizedListRowMeasureMode =
        prelude::VirtualizedListRowMeasureMode::Fixed;
    let root_virtualized_selection_mode: root::VirtualizedListSelectionMode =
        root::VirtualizedListSelectionMode::Multiple;
    let prelude_virtualized_selection_mode: prelude::VirtualizedListSelectionMode =
        prelude::VirtualizedListSelectionMode::Single;
    let _root_scroll_strategy: root::VirtualizedListScrollStrategy =
        root::VirtualizedListScrollStrategy::Center;
    let _prelude_scroll_strategy: prelude::VirtualizedListScrollStrategy =
        prelude::VirtualizedListScrollStrategy::Center;

    assert_eq!(root_status_cue.state().role(), Role::Label);
    assert_eq!(prelude_status_cue.state().role(), Role::Label);
    assert_eq!(root_empty_state.state().role(), Role::Section);
    assert_eq!(prelude_empty_state.state().role(), Role::Section);
    assert_eq!(root_tree_component_state.role(), Role::Tree);
    assert_eq!(prelude_tree_component_state.item_role(), Role::TreeItem);
    assert!(root_virtualized_measure_mode.measured());
    assert!(!prelude_virtualized_measure_mode.measured());
    assert_eq!(root_tree_component_state.focused_value(), Some("root"));
    assert_eq!(root_tree_component_snapshot.role(), Role::Tree);
    assert_eq!(prelude_tree_component_snapshot.row_role(), Role::TreeItem);
    assert_eq!(root_tree_state.items().len(), 1);
    assert_eq!(prelude_tree_state.items().len(), 1);
    assert_eq!(root_tree_state.role(), Role::Tree);
    assert_eq!(root_tree_state.items()[0].role(), Role::TreeItem);
    assert!(root_tree_state.items()[0].has_children());
    assert_eq!(
        root_tree_state.items()[0].children_load_state().as_str(),
        "unloaded"
    );
    assert!(prelude_tree_state.items()[0].children_load_failed());
    assert!(root_tree_load_state.is_loading());
    assert!(prelude_tree_load_state.is_unloaded());
    assert!(direct_tree_load_state.is_loaded());
    assert_eq!(root::tree_navigation_target("home", 0, &[false]), Some(0));
    assert_eq!(
        prelude::tree_navigation_target("home", 0, &[false]),
        Some(0)
    );
    assert_eq!(
        root_tree_component_snapshot.rows()[0].render_key(),
        "0:root"
    );
    assert_eq!(prelude_tree_component_snapshot.state().items().len(), 1);
    assert_eq!(root_tree_component_snapshot.rendered_row_count(), 1);
    assert_eq!(root_tree_move.position(), TreeDropPosition::Before);
    assert_eq!(root_tree_move.target_parent_value(), None);
    assert_eq!(root_tree_move.sibling_anchor_value(), Some("sibling"));
    assert_eq!(prelude_tree_position.as_str(), "inside");
    assert_eq!(prelude_move_state.items()[0].value(), "root");
    assert_eq!(prelude_move_state.items()[1].value(), "child");
    assert_eq!(
        root_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(
        prelude_virtualized_state.navigation_target("pagedown"),
        Some(7)
    );
    assert_eq!(root_virtualized_component_state.active_index(), Some(4));
    assert_eq!(
        root_virtualized_component_state.active_key(),
        Some("root-item-4")
    );
    assert_eq!(
        prelude_virtualized_component_state.selected_index(),
        Some(4)
    );
    assert_eq!(
        prelude_virtualized_component_state.selected_keys(),
        ["prelude-item-4"]
    );
    assert_eq!(root_virtualized_selection_mode.as_str(), "multiple");
    assert_eq!(prelude_virtualized_selection_mode.as_str(), "single");
    assert_eq!(root_virtualized_row_kind.as_str(), "item");
    assert_eq!(prelude_virtualized_row_kind.role(), Role::Group);
    assert_eq!(root_virtualized_status_kind.as_str(), "append-loading");
    assert_eq!(
        prelude_virtualized_status_kind.row_kind().role(),
        Role::AlertDialog
    );
    assert_eq!(root_virtualized_snapshot.role(), Role::ListBox);
    assert_eq!(prelude_virtualized_snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(
        root_virtualized_snapshot.state().scroll_target_for_key(
            "root-item-4",
            root::VirtualizedListScrollStrategy::Top,
            root_virtualized_snapshot.viewport_extent(),
            root_virtualized_snapshot.scroll_offset(),
        ),
        root::VirtualizedListRevealResult::Revealed(root::VirtualizedListRevealTarget::new(
            "root-item-4",
            4,
            ui_px(112.0),
            false,
        ))
    );
    assert_eq!(
        prelude_virtualized_snapshot.state().scroll_target_for_key(
            "prelude-item-4",
            prelude::VirtualizedListScrollStrategy::Top,
            prelude_virtualized_snapshot.viewport_extent(),
            prelude_virtualized_snapshot.scroll_offset(),
        ),
        prelude::VirtualizedListRevealResult::Revealed(prelude::VirtualizedListRevealTarget::new(
            "prelude-item-4",
            4,
            ui_px(112.0),
            false,
        ),)
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_reveals_active_row_and_emits_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let items = (0..100).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(112.0)).child(
                    VirtualizedList::new("runtime-list", "Runtime list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(4)
                        .overscan(2)
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.index());
                        }),
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-list:root")
        .expect("virtualized list root should render as a focusable target");
    cx.simulate_click(root.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        cx.debug_selector_is_focused("virtualized-list:runtime-list:root"),
        "clicking the VirtualizedList root should focus it for keyboard navigation"
    );

    let row_0 = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0000")
        .expect("row 0 should render before keyboard navigation");
    cx.simulate_click(row_0.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    activations.borrow_mut().clear();

    let row_4_before = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should be present in the overscan window before PageDown");
    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_4_after = cx
        .debug_bounds("virtualized-list:runtime-list:row:item-0004")
        .expect("row 4 should stay rendered after PageDown reveal");
    assert!(
        row_4_after.top() < row_4_before.top(),
        "expected PageDown to scroll the new active row upward; before={row_4_before:?} after={row_4_after:?}"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(activations.borrow().as_slice(), &[4]);
}

#[open_gpui::test]
fn virtualized_list_runtime_uses_host_scroll_handle_for_controlled_reveal(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        scroll_handle: open_gpui::ScrollHandle,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let items = (0..100).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(112.0)).child(
                    VirtualizedList::new("host-scroll-list", "Host scroll list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(4)
                        .overscan(0)
                        .scroll_handle(&self.scroll_handle)
                        .reveal_key("item-0010", VirtualizedListScrollStrategy::Top),
                ),
            )
        }
    }

    let scroll_handle = open_gpui::ScrollHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        scroll_handle: scroll_handle.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(scroll_handle.offset().y, px(-280.0));
}

#[open_gpui::test]
fn virtualized_list_runtime_nested_action_click_does_not_activate_row(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<String>>>,
        nested_actions: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let nested_actions = self.nested_actions.clone();
            let items = [VirtualizedListItemDescriptor::new("alpha", "Alpha")];

            div().size_full().child(
                div().w(px(240.0)).h(px(56.0)).child(
                    VirtualizedList::new("nested-action-list", "Nested action list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(2)
                        .render_row(move |context, _, _| {
                            let key = context.key().to_owned();
                            let action_key = key.clone();
                            let selector_key = key.clone();
                            let nested_actions = nested_actions.clone();

                            div()
                                .w_full()
                                .debug_selector(move || {
                                    format!(
                                        "virtualized-list:nested-action-list:row-action:{selector_key}"
                                    )
                                })
                                .child(
                                    Button::new(format!("nested-action-button-{key}"), "Open")
                                        .with_size(Size::Small)
                                        .on_click(move |_, _, _| {
                                            nested_actions.borrow_mut().push(action_key.clone());
                                        }),
                                )
                        })
                        .on_activate(move |activation, _, _| {
                            activations
                                .borrow_mut()
                                .push(activation.key().to_owned());
                        }),
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let nested_actions = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
        nested_actions: nested_actions.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_action = cx
        .debug_bounds("virtualized-list:nested-action-list:row-action:alpha")
        .expect("nested row action should render inside the item row");
    cx.simulate_click(row_action.center(), Modifiers::none());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(nested_actions.borrow().as_slice(), &["alpha".to_owned()]);
    assert!(
        activations.borrow().is_empty(),
        "nested action clicks should not bubble into row activation"
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_renders_sticky_overlay_as_inert_presentation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        activations: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let items = [
                VirtualizedListItemDescriptor::section("recent", "Recent"),
                VirtualizedListItemDescriptor::item("alpha", "Alpha"),
                VirtualizedListItemDescriptor::section("archived", "Archived"),
                VirtualizedListItemDescriptor::item("spacer", "Loading gap").disabled(true),
                VirtualizedListItemDescriptor::item("gamma", "Gamma"),
            ];

            div().size_full().child(
                div().w(px(240.0)).h(px(56.0)).child(
                    VirtualizedList::new("runtime-sticky-list", "Runtime sticky list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(2)
                        .overscan(0)
                        .default_active_key("alpha")
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.key().to_owned());
                        }),
                ),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-sticky-list:root")
        .expect("sticky list root should render");
    cx.simulate_click(root.center(), Modifiers::none());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("pagedown");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let overlay = cx
        .debug_bounds("virtualized-list:runtime-sticky-list:sticky-overlay:archived")
        .expect("sticky overlay should render as a presentation layer");
    let viewport = cx
        .debug_bounds("scroll-area:virtualized-list:runtime-sticky-list:viewport")
        .expect("sticky list viewport should render");
    assert!(overlay.size.height > px(0.0));
    assert_eq!(
        overlay.top(),
        viewport.top(),
        "sticky overlay should stay pinned to the viewport after scrolling; viewport={viewport:?} overlay={overlay:?}"
    );

    cx.simulate_click(overlay.center(), Modifiers::none());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_selector_is_focused("virtualized-list:runtime-sticky-list:root"),
        "clicking presentation overlay space should leave focus on the list root"
    );
    assert!(
        activations.borrow().is_empty(),
        "sticky overlay must not become an accidental activation target"
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_typeahead_reveals_without_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selection_changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection_changes = self.selection_changes.clone();
            let mut items = (0..40)
                .map(|index| {
                    VirtualizedListItemDescriptor::new(
                        format!("item-{index:04}"),
                        format!("Item {index:04}"),
                    )
                })
                .collect::<Vec<_>>();
            items[20] = VirtualizedListItemDescriptor::new("item-0020", "Zulu Target");

            div().size_full().child(
                div().w(px(240.0)).h(px(84.0)).child(
                    VirtualizedList::new("runtime-typeahead-list", "Runtime typeahead list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(3)
                        .overscan(1)
                        .selection_mode(VirtualizedListSelectionMode::Multiple)
                        .on_selection_change(move |selection, _, _| {
                            selection_changes.borrow_mut().push(
                                selection
                                    .selected_keys()
                                    .into_iter()
                                    .map(str::to_owned)
                                    .collect(),
                            );
                        }),
                ),
            )
        }
    }

    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selection_changes: selection_changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-typeahead-list:root")
        .expect("virtualized list root should render");
    cx.simulate_click(root.center(), Modifiers::none());
    selection_changes.borrow_mut().clear();
    cx.simulate_keystrokes("z");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("virtualized-list:runtime-typeahead-list:row:item-0020")
            .is_some(),
        "typeahead should reveal the matching row"
    );
    assert!(
        selection_changes.borrow().is_empty(),
        "typeahead should move active state without selecting"
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_shift_navigation_replaces_range_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selection_changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection_changes = self.selection_changes.clone();
            let items = (0..8).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(112.0)).child(
                    VirtualizedList::new("runtime-range-list", "Runtime range list", items)
                        .with_size(Size::Small)
                        .row_height(ui_px(28.0))
                        .viewport_item_count(4)
                        .selection_mode(VirtualizedListSelectionMode::Multiple)
                        .on_selection_change(move |selection, _, _| {
                            selection_changes.borrow_mut().push(
                                selection
                                    .selected_keys()
                                    .into_iter()
                                    .map(str::to_owned)
                                    .collect(),
                            );
                        }),
                ),
            )
        }
    }

    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selection_changes: selection_changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_0 = cx
        .debug_bounds("virtualized-list:runtime-range-list:row:item-0000")
        .expect("row 0 should render");
    cx.simulate_click(row_0.center(), Modifiers::none());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    selection_changes.borrow_mut().clear();

    cx.simulate_keystrokes("shift-down");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selection_changes.borrow().as_slice(),
        &[vec!["item-0000".to_owned(), "item-0001".to_owned()]]
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_shift_click_replaces_range_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selection_changes: Rc<RefCell<Vec<Vec<String>>>>,
        activations: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection_changes = self.selection_changes.clone();
            let activations = self.activations.clone();
            let items = [
                VirtualizedListItemDescriptor::new("item-0000", "Item 0000"),
                VirtualizedListItemDescriptor::new("item-0001", "Item 0001"),
                VirtualizedListItemDescriptor::section("section-a", "Section A"),
                VirtualizedListItemDescriptor::new("item-0003", "Item 0003").disabled(true),
                VirtualizedListItemDescriptor::new("item-0004", "Item 0004"),
                VirtualizedListItemDescriptor::new("item-0005", "Item 0005"),
            ];

            div().size_full().child(
                div().w(px(240.0)).h(px(168.0)).child(
                    VirtualizedList::new(
                        "runtime-shift-click-list",
                        "Runtime shift-click list",
                        items,
                    )
                    .with_size(Size::Small)
                    .row_height(ui_px(28.0))
                    .viewport_item_count(6)
                    .selection_mode(VirtualizedListSelectionMode::Multiple)
                    .on_selection_change(move |selection, _, _| {
                        selection_changes.borrow_mut().push(
                            selection
                                .selected_keys()
                                .into_iter()
                                .map(str::to_owned)
                                .collect(),
                        );
                    })
                    .on_activate(move |activation, _, _| {
                        activations.borrow_mut().push(activation.index());
                    }),
                ),
            )
        }
    }

    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selection_changes: selection_changes.clone(),
        activations: activations.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let row_0 = cx
        .debug_bounds("virtualized-list:runtime-shift-click-list:row:item-0000")
        .expect("row 0 should render");
    cx.simulate_click(row_0.center(), Modifiers::none());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    selection_changes.borrow_mut().clear();
    activations.borrow_mut().clear();

    let row_4 = cx
        .debug_bounds("virtualized-list:runtime-shift-click-list:row:item-0004")
        .expect("row 4 should render");
    cx.simulate_click(
        row_4.center(),
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selection_changes.borrow().as_slice(),
        &[vec![
            "item-0000".to_owned(),
            "item-0001".to_owned(),
            "item-0004".to_owned()
        ]]
    );
    assert!(
        activations.borrow().is_empty(),
        "multi-select shift-click should update selection without activation"
    );
}

#[open_gpui::test]
fn virtualized_list_runtime_shift_space_applies_active_range_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selection_changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection_changes = self.selection_changes.clone();
            let items = (0..5).map(|index| {
                VirtualizedListItemDescriptor::new(
                    format!("item-{index:04}"),
                    format!("Item {index:04}"),
                )
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(140.0)).child(
                    VirtualizedList::new(
                        "runtime-shift-space-list",
                        "Runtime shift-space list",
                        items,
                    )
                    .with_size(Size::Small)
                    .row_height(ui_px(28.0))
                    .viewport_item_count(5)
                    .selection_mode(VirtualizedListSelectionMode::Multiple)
                    .default_active_key("item-0002")
                    .default_selected_keys(["item-0000"])
                    .on_selection_change(move |selection, _, _| {
                        selection_changes.borrow_mut().push(
                            selection
                                .selected_keys()
                                .into_iter()
                                .map(str::to_owned)
                                .collect(),
                        );
                    }),
                ),
            )
        }
    }

    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selection_changes: selection_changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let root = cx
        .debug_bounds("virtualized-list:runtime-shift-space-list:root")
        .expect("virtualized list root should render");
    cx.simulate_click(root.center(), Modifiers::none());
    selection_changes.borrow_mut().clear();

    cx.simulate_keystrokes("shift-space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        selection_changes.borrow().as_slice(),
        &[vec!["item-0002".to_owned()]]
    );
}

#[open_gpui::test]
fn scroll_area_default_handle_survives_reconstructed_component_values(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("scroll-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "default-runtime-scroll",
                        div().flex().flex_col().children(rows),
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

    let before = cx
        .debug_bounds("scroll-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let after = cx
        .debug_bounds("scroll-row-2")
        .expect("row should still be rendered after scrolling");

    assert!(
        after.top() < before.top(),
        "expected row bounds to move upward after wheel scrolling; before={before:?} after={after:?}"
    );
}

#[open_gpui::test]
fn scroll_area_reset_key_resets_default_runtime_handle(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        reset_key: String,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("reset-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Row {index}"))
            });

            div().size_full().child(
                div().w(px(180.0)).h(px(60.0)).child(
                    ScrollArea::new(
                        "reset-runtime-scroll",
                        div().flex().flex_col().children(rows),
                    )
                    .vertical()
                    .reset_on_key(self.reset_key.clone()),
                ),
            )
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| TestView {
        reset_key: "overview".to_string(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let initial = cx
        .debug_bounds("reset-row-2")
        .expect("row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let scrolled = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after scrolling");
    assert!(
        scrolled.top() < initial.top(),
        "expected row bounds to move upward after wheel scrolling; initial={initial:?} scrolled={scrolled:?}"
    );

    view.update(cx, |view, cx| {
        view.reset_key = "details".to_string();
        cx.notify();
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let reset = cx
        .debug_bounds("reset-row-2")
        .expect("row should still be rendered after reset");
    assert_eq!(
        reset.top(),
        initial.top(),
        "expected reset key change to restore the scroll origin; initial={initial:?} reset={reset:?}"
    );
}

#[open_gpui::test]
fn scroll_area_runtime_scrolls_horizontal_and_two_axis_content(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal_cells = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("horizontal-cell-{index}"))
                    .w(px(96.0))
                    .h(px(40.0))
                    .flex_none()
                    .child(format!("Column {index}"))
            });
            let grid_rows = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("grid-row-{index}"))
                    .w(px(520.0))
                    .h(px(36.0))
                    .flex_none()
                    .child(format!("Grid row {index}"))
            });

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div().w(px(180.0)).h(px(64.0)).child(
                        ScrollArea::new(
                            "horizontal-runtime-scroll",
                            div()
                                .flex()
                                .gap_2()
                                .min_w(px(820.0))
                                .children(horizontal_cells),
                        )
                        .horizontal(),
                    ),
                )
                .child(
                    div().w(px(180.0)).h(px(70.0)).child(
                        ScrollArea::new(
                            "two-axis-runtime-scroll",
                            div().flex().flex_col().min_w(px(520.0)).children(grid_rows),
                        )
                        .both(),
                    ),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_before = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should be rendered before scrolling");
    let grid_before_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(24.0)),
        delta: ScrollDelta::Pixels(point(px(-70.0), px(0.0))),
        ..Default::default()
    });
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(-60.0), px(0.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let horizontal_after = cx
        .debug_bounds("horizontal-cell-2")
        .expect("horizontal cell should remain rendered after scrolling");
    let grid_after_x = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after scrolling");

    assert!(
        horizontal_after.left() < horizontal_before.left(),
        "expected horizontal content to move left after wheel scrolling; before={horizontal_before:?} after={horizontal_after:?}"
    );
    assert!(
        grid_after_x.left() < grid_before_x.left(),
        "expected two-axis content to move left after horizontal wheel scrolling; before={grid_before_x:?} after={grid_after_x:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(40.0), px(108.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-42.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let grid_after_y = cx
        .debug_bounds("grid-row-2")
        .expect("grid row should remain rendered after vertical scrolling");
    assert!(
        grid_after_y.top() < grid_after_x.top(),
        "expected two-axis content to move up after vertical wheel scrolling; before={grid_after_x:?} after={grid_after_y:?}"
    );
}

#[open_gpui::test]
fn scroll_area_nested_scroll_keeps_parent_static(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let queue_lanes = (0..8).map(|index| {
                div()
                    .debug_selector(move || format!("nested-lane-{index}"))
                    .w(px(128.0))
                    .h(px(32.0))
                    .flex_none()
                    .child(format!("Lane {index}"))
            });
            let outer_rows = (0..10).map(|index| {
                div()
                    .debug_selector(move || format!("nested-outer-row-{index}"))
                    .h(px(24.0))
                    .w_full()
                    .child(format!("Outer row {index}"))
            });

            div().size_full().child(
                div().w(px(240.0)).h(px(120.0)).child(
                    ScrollArea::new(
                        "nested-outer-scroll",
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-header".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer header"),
                            )
                            .child(
                                div().h(px(52.0)).min_h(px(0.0)).overflow_hidden().child(
                                    ScrollArea::new(
                                        "nested-inner-scroll",
                                        div()
                                            .flex()
                                            .gap_2()
                                            .min_w(px(1024.0))
                                            .children(queue_lanes),
                                    )
                                    .horizontal()
                                    .with_size(Size::Small),
                                ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "nested-outer-bottom".into())
                                    .h(px(24.0))
                                    .w_full()
                                    .child("Outer bottom marker"),
                            )
                            .child(div().flex().flex_col().gap_1().children(outer_rows)),
                    )
                    .vertical()
                    .with_size(Size::Small),
                ),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_before = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should be rendered before scrolling");
    let outer_before = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should be rendered before scrolling");
    let inner_viewport = cx
        .debug_bounds("scroll-area:nested-inner-scroll")
        .expect("inner scroll viewport should be rendered before scrolling");

    cx.simulate_event(ScrollWheelEvent {
        position: inner_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-48.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let lane_after = cx
        .debug_bounds("nested-lane-2")
        .expect("inner lane should remain rendered after scrolling");
    let outer_after = cx
        .debug_bounds("nested-outer-bottom")
        .expect("outer marker should remain rendered after scrolling");

    assert!(
        lane_after.left() < lane_before.left(),
        "expected nested horizontal ScrollArea to move after wheel scrolling; before={lane_before:?} after={lane_after:?}"
    );
    assert_eq!(
        outer_after.top(),
        outer_before.top(),
        "expected wheel scrolling inside the nested ScrollArea to leave the parent viewport in place; before={outer_before:?} after={outer_after:?}"
    );
}

#[open_gpui::test]
fn splitter_runtime_drag_resizes_horizontal_and_vertical_panels(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let horizontal = Splitter::new("horizontal-drag-split")
                .horizontal()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("left", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("right", 0.5).min_fraction(0.2),
                    div(),
                ));
            let vertical = Splitter::new("vertical-drag-split")
                .vertical()
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("top", 0.5).min_fraction(0.2),
                    div(),
                ))
                .panel(SplitterPanel::new(
                    SplitterPanelDescriptor::new("bottom", 0.5).min_fraction(0.2),
                    div(),
                ));

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(div().w(px(400.0)).h(px(120.0)).child(horizontal))
                .child(div().w(px(240.0)).h(px(360.0)).child(vertical))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_before = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should be rendered");
    let right_before = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should be rendered");
    let horizontal_handle = cx
        .debug_bounds("splitter:horizontal-drag-split:handle:0")
        .expect("horizontal handle should be rendered")
        .center();
    let top_before = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should be rendered");
    let bottom_before = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should be rendered");
    let vertical_handle = cx
        .debug_bounds("splitter:vertical-drag-split:handle:0")
        .expect("vertical handle should be rendered")
        .center();

    cx.simulate_mouse_down(horizontal_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(4.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(24.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(horizontal_handle.x + px(80.0), horizontal_handle.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_down(vertical_handle, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(4.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(24.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_up(
        point(vertical_handle.x, vertical_handle.y + px(72.0)),
        MouseButton::Left,
        Default::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left_after = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should remain rendered");
    let right_after = cx
        .debug_bounds("splitter-panel:right")
        .expect("right panel should remain rendered");
    let top_after = cx
        .debug_bounds("splitter-panel:top")
        .expect("top panel should remain rendered");
    let bottom_after = cx
        .debug_bounds("splitter-panel:bottom")
        .expect("bottom panel should remain rendered");

    assert!(
        left_after.size.width > left_before.size.width
            && right_after.size.width < right_before.size.width,
        "expected horizontal drag to grow the first panel and shrink the second; before=({left_before:?}, {right_before:?}) after=({left_after:?}, {right_after:?})"
    );
    assert!(
        top_after.size.height > top_before.size.height
            && bottom_after.size.height < bottom_before.size.height,
        "expected vertical drag to grow the first panel and shrink the second; before=({top_before:?}, {bottom_before:?}) after=({top_after:?}, {bottom_after:?})"
    );
}

#[test]
fn splitter_state_normalizes_panel_fractions_and_constraints() {
    let state = SplitterState::resolve(
        "workspace",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("nav", 0.2)
                .min_fraction(0.18)
                .max_fraction(0.32),
            SplitterPanelDescriptor::new("main", 0.65)
                .min_fraction(0.42)
                .max_fraction(0.7),
            SplitterPanelDescriptor::new("inspector", 0.35)
                .min_fraction(0.12)
                .max_fraction(0.28),
        ],
    );

    let sum: f32 = state.panels().iter().map(|panel| panel.fraction()).sum();
    assert_eq!(state.group_id(), "workspace");
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Medium);
    assert!((sum - 1.0).abs() < 0.001);
    assert_eq!(state.panels().len(), 3);
    assert!(state.panels()[0].fraction() >= 0.18);
    assert!(state.panels()[1].fraction() <= 0.7);
    assert!(state.panels()[2].fraction() <= 0.28);
    assert_eq!(state.handles().len(), 2);
    assert_eq!(state.handles()[0].before_id(), "nav");
    assert_eq!(state.handles()[0].after_id(), "main");
    assert_eq!(state.metrics().handle_hit_size(), ui_px(12.0));
}

#[test]
fn splitter_resize_delta_clamps_to_adjacent_min_max() {
    let state = SplitterState::resolve(
        "editor",
        Orientation::Horizontal,
        Size::Small,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.35)
                .min_fraction(0.2)
                .max_fraction(0.4),
            SplitterPanelDescriptor::new("right", 0.65)
                .min_fraction(0.5)
                .max_fraction(0.8),
        ],
    );
    let grown = state.resized_by(0, 0.3);
    let shrunk = grown.resized_by(0, -0.5);

    assert!((grown.panels()[0].fraction() - 0.4).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.6).abs() < 0.001);
    assert!((shrunk.panels()[0].fraction() - 0.2).abs() < 0.001);
    assert!((shrunk.panels()[1].fraction() - 0.8).abs() < 0.001);
}

#[test]
fn splitter_resize_ignores_disabled_and_invalid_deltas() {
    let disabled = SplitterState::resolve(
        "disabled-editor",
        Orientation::Horizontal,
        Size::Small,
        true,
        [
            SplitterPanelDescriptor::new("left", 0.4),
            SplitterPanelDescriptor::new("right", 0.6),
        ],
    );

    assert!(disabled.disabled());
    assert!(disabled.handles()[0].disabled());
    assert_eq!(disabled.resized_by(0, 0.2), disabled);

    let enabled = SplitterState::resolve(
        "enabled-editor",
        Orientation::Horizontal,
        Size::Small,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.4),
            SplitterPanelDescriptor::new("right", 0.6),
        ],
    );

    assert_eq!(enabled.resized_by(0, f32::NAN), enabled);
    assert_eq!(enabled.resized_by(0, f32::INFINITY), enabled);
    assert_eq!(enabled.resized_by(4, 0.2), enabled);
}

#[test]
fn splitter_state_sanitizes_non_finite_panel_inputs() {
    let state = SplitterState::resolve(
        "unstable-inputs",
        Orientation::Vertical,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("nan", f32::NAN)
                .min_fraction(f32::NAN)
                .max_fraction(f32::INFINITY),
            SplitterPanelDescriptor::new("negative", -0.8)
                .min_fraction(-0.2)
                .max_fraction(0.7),
            SplitterPanelDescriptor::new("valid", 0.5)
                .min_fraction(0.2)
                .max_fraction(0.8),
        ],
    );

    let sum: f32 = state.panels().iter().map(|panel| panel.fraction()).sum();
    assert!((sum - 1.0).abs() < 0.001);
    assert!(state.panels().iter().all(|panel| {
        panel.fraction().is_finite()
            && panel.min_fraction().is_finite()
            && panel.max_fraction().is_finite()
            && panel.fraction() >= panel.min_fraction()
            && panel.fraction() <= panel.max_fraction()
    }));
    assert_eq!(state.panels()[0].min_fraction(), 0.0);
    assert_eq!(state.panels()[0].max_fraction(), 1.0);
}

#[test]
fn splitter_runtime_fraction_overrides_still_use_resize_constraints() {
    let state = SplitterState::resolve(
        "runtime-editor",
        Orientation::Horizontal,
        Size::Medium,
        false,
        [
            SplitterPanelDescriptor::new("left", 0.3)
                .min_fraction(0.15)
                .max_fraction(0.75),
            SplitterPanelDescriptor::new("right", 0.7)
                .min_fraction(0.25)
                .max_fraction(0.85),
        ],
    );

    let overridden = state.with_panel_fractions(&[0.45, 0.55]);
    let grown = overridden.resized_by(0, 0.5);
    let invalid = overridden.with_panel_fractions(&[0.2]);

    assert!((overridden.panels()[0].fraction() - 0.45).abs() < 0.001);
    assert!((overridden.panels()[1].fraction() - 0.55).abs() < 0.001);
    assert!((grown.panels()[0].fraction() - 0.75).abs() < 0.001);
    assert!((grown.panels()[1].fraction() - 0.25).abs() < 0.001);
    assert_eq!(invalid, overridden);
}

#[test]
fn splitter_collapsed_panel_uses_collapsed_fraction() {
    let state = Splitter::new("collapsed-split")
        .vertical()
        .small()
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("summary", 0.3)
                .min_fraction(0.2)
                .collapsible(true)
                .collapsed(true)
                .collapsed_fraction(0.05),
            div(),
        ))
        .panel(SplitterPanel::new(
            SplitterPanelDescriptor::new("details", 0.7).min_fraction(0.4),
            div(),
        ))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert!(state.panels()[0].collapsible());
    assert!(state.panels()[0].collapsed());
    assert!((state.panels()[0].fraction() - 0.05).abs() < 0.001);
    assert_eq!(state.panels()[0].collapsed_fraction(), 0.05);
    assert_eq!(state.handles().len(), 1);
    assert!(!state.handles()[0].disabled());

    let unchanged = state.resized_by(0, 0.1);
    let restored = state.resized_by(0, 0.16);
    let runtime_restored = state.with_panel_fractions(&[0.22, 0.78]);

    assert_eq!(unchanged, state);
    assert!(!restored.panels()[0].collapsed());
    assert!(restored.panels()[0].fraction() >= 0.2);
    assert!(!runtime_restored.panels()[0].collapsed());
    assert!((runtime_restored.panels()[0].fraction() - 0.22).abs() < 0.001);
}
