use open_gpui::{
    div, point, px, AppContext, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, ScrollDelta, ScrollWheelEvent, Styled, Window,
};
use open_gpui_ui_components::{
    active_index_from_str_keys, apply_tree_move, first_enabled,
    gpui_adapter::{focus_ring_shadow, init_text_input, TextInputController},
    last_enabled, next_enabled, sidebar_navigation_target, toolbar_navigation_target,
    virtualized_list_scroll_target, AlertDialog, AlertDialogIntent, Avatar, AvatarGroup,
    AvatarGroupCount, Badge, BadgeVariant, Button, ButtonVariant, Checkbox, ColorIntent,
    ColorState, Combobox, ComboboxOption, Command, CommandItem, EmptyState, FeedbackIntent, Field,
    FocusRing, HoverCard, IconButton, Kbd, Label, Listbox, ListboxOption, Menu, MenuItem, Progress,
    ProgressVisualMode, RadioGroup, RadioGroupState, RadioItem, RadioItemDescriptor,
    RadioSelection, ScrollArea, ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy, Select,
    Separator, Sheet, Sidebar, SidebarCollapseMode, SidebarItem, SidebarItemDescriptor,
    SidebarSection, SidebarSectionDescriptor, SidebarSide, SidebarState, SidebarVariant, Skeleton,
    Splitter, SplitterPanel, SplitterPanelDescriptor, SplitterState, StatusCue, Switch,
    TableToolbar, Tabs, TabsActivationMode, TabsItem, TabsItemDescriptor, TabsSelection, TabsState,
    TextInput, TextInputDisplayMode, Textarea, ThemeColor, ThemeDefinition, ThemeMode,
    ThemeRegistry, ThemeResolver, ThemeSnapshot, ThemeValidationError, Toggle, ToggleGroup,
    ToggleGroupItem, ToggleVariant, Toolbar, ToolbarItem, ToolbarItemDescriptor, ToolbarItemKind,
    ToolbarSelection, ToolbarState, Tree, TreeChildrenLoadState, TreeDropPosition,
    TreeItemDescriptor, TreeMove, TreeMoveTarget, VirtualizedList, VirtualizedListActivation,
    VirtualizedListItemDescriptor, VirtualizedListScrollStrategy, VirtualizerRange,
    DEFAULT_FOCUS_RING_WIDTH,
};
use open_gpui_ui_core::{
    semantic, ui_px, Orientation, Role, Sizable, Size, ThemeTokens, Toggled, TokenKey,
};
use std::cell::RefCell;
use std::rc::Rc;

const TEST_SURFACE: TokenKey = TokenKey::new("test.surface");
const TEST_SURFACE_MUTED: TokenKey = TokenKey::new("test.surface_muted");
const TEST_BORDER: TokenKey = TokenKey::new("test.border");
const TEST_TEXT: TokenKey = TokenKey::new("test.text");
const TEST_TEXT_MUTED: TokenKey = TokenKey::new("test.text_muted");
const TEST_ACCENT: TokenKey = TokenKey::new("test.accent");
const TEST_FOCUS_RING: TokenKey = TokenKey::new("test.focus_ring");
const TEST_DESTRUCTIVE: TokenKey = TokenKey::new("test.destructive");

fn custom_tokens() -> ThemeTokens {
    ThemeTokens {
        surface: TEST_SURFACE,
        surface_muted: TEST_SURFACE_MUTED,
        border: TEST_BORDER,
        text: TEST_TEXT,
        text_muted: TEST_TEXT_MUTED,
        accent: TEST_ACCENT,
        focus_ring: TEST_FOCUS_RING,
        destructive: TEST_DESTRUCTIVE,
        ..ThemeTokens::default()
    }
}

#[test]
fn default_button_state_uses_button_role_and_medium_metrics() {
    let state = Button::new("save", "Save").state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Default);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.button_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.button_px());
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert_eq!(state.focus_ring().width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn destructive_button_uses_destructive_token_intent() {
    let state = Button::new("delete", "Delete")
        .variant(ButtonVariant::Destructive)
        .state();

    assert_eq!(state.colors().background().token(), semantic::DESTRUCTIVE);
    assert_eq!(
        state.colors().foreground().token(),
        semantic::DESTRUCTIVE_FOREGROUND
    );
}

#[test]
fn disabled_button_blocks_activation_metadata() {
    let state = Button::new("disabled", "Disabled").disabled(true).state();

    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn button_size_helpers_apply_foundation_size_metrics() {
    let state = Button::new("large", "Large").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[test]
fn tabs_navigation_helpers_skip_disabled_tabs() {
    let keys = vec![
        "overview".to_string(),
        "details".to_string(),
        "history".to_string(),
    ];
    let disabled = [false, true, false];

    assert_eq!(first_enabled(&disabled), Some(0));
    assert_eq!(last_enabled(&disabled), Some(2));
    assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
    assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    assert_eq!(
        active_index_from_str_keys(&keys, Some("details"), &disabled),
        Some(0)
    );
    assert_eq!(
        active_index_from_str_keys(&keys, Some("missing"), &disabled),
        Some(0)
    );
}

#[test]
fn tabs_state_resolution_tracks_selected_focus_and_tab_stop() {
    let state = TabsState::resolve(
        Orientation::Vertical,
        TabsActivationMode::Manual,
        Size::Small,
        Some("security"),
        Some("billing"),
        [
            TabsItemDescriptor::new("profile", "Profile"),
            TabsItemDescriptor::new("security", "Security"),
            TabsItemDescriptor::new("billing", "Billing").disabled(true),
            TabsItemDescriptor::new("integrations", "Integrations"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("security"));
    assert_eq!(state.focused_value(), Some("security"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].focused());
}

#[test]
fn tabs_builder_state_falls_back_to_first_enabled_tab() {
    let state = Tabs::new("settings")
        .orientation(Orientation::Horizontal)
        .activation_mode(TabsActivationMode::Automatic)
        .with_size(Size::Large)
        .default_selected("history")
        .item(TabsItem::new("overview", "Overview", div()))
        .item(TabsItem::new("details", "Details", div()))
        .item(TabsItem::new("history", "History", div()).disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.activation_mode(), TabsActivationMode::Automatic);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.selected_value(), Some("overview"));
    assert_eq!(state.focused_value(), Some("overview"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

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
        .default_active_index(104)
        .default_selected_index(101)
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
    assert_eq!(active_row.position_in_set(), 105);
    assert_eq!(active_row.size_of_set(), 10_000);
    assert_eq!(active_row.virtual_start(), ui_px(2_912.0));
    assert_eq!(active_row.virtual_size(), ui_px(28.0));

    let selected_row = snapshot
        .selected_row()
        .expect("selected row should be rendered");
    assert_eq!(selected_row.index(), 101);
    assert!(selected_row.selected());

    let activation = VirtualizedListActivation::new(active_row.index());
    assert_eq!(activation.index(), 104);
    assert_eq!(
        virtualized_list_scroll_target(
            VirtualizedListScrollStrategy::Top,
            activation.index(),
            snapshot.state().item_count(),
            snapshot.metrics().row_height(),
            snapshot.viewport_extent(),
            snapshot.scroll_offset(),
        ),
        ui_px(2_912.0)
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
        .default_active_index(5)
        .default_selected_index(3)
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
    let root_virtualized_state: root::VirtualizedListState =
        root::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
    let prelude_virtualized_state: prelude::VirtualizedListState =
        prelude::VirtualizedListState::resolve(Size::Small, false, 12, Some(4), Some(4), Some(3));
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
    .default_active_index(4)
    .default_selected_index(4)
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
    .default_active_index(4)
    .default_selected_index(4)
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
        root::VirtualizedListActivation::new(4);
    let _prelude_virtualized_activation: prelude::VirtualizedListActivation =
        prelude::VirtualizedListActivation::new(4);
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
        prelude_virtualized_component_state.selected_index(),
        Some(4)
    );
    assert_eq!(root_virtualized_snapshot.role(), Role::ListBox);
    assert_eq!(prelude_virtualized_snapshot.row_role(), Role::ListBoxOption);
    assert_eq!(
        root::virtualized_list_scroll_target(
            root::VirtualizedListScrollStrategy::Top,
            4,
            root_virtualized_snapshot.state().item_count(),
            root_virtualized_snapshot.metrics().row_height(),
            root_virtualized_snapshot.viewport_extent(),
            root_virtualized_snapshot.scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        prelude::virtualized_list_scroll_target(
            prelude::VirtualizedListScrollStrategy::Top,
            4,
            prelude_virtualized_snapshot.state().item_count(),
            prelude_virtualized_snapshot.metrics().row_height(),
            prelude_virtualized_snapshot.viewport_extent(),
            prelude_virtualized_snapshot.scroll_offset(),
        ),
        ui_px(112.0)
    );
    assert_eq!(
        root::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
    );
    assert_eq!(
        prelude::virtualized_list_navigation_target("end", 4, 12, 3),
        Some(11)
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
fn tabs_vertical_tablist_scrolls_when_constrained(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let tabs = (0..12).fold(
                Tabs::new("overflow-tabs")
                    .orientation(Orientation::Vertical)
                    .small()
                    .default_selected("tab-0"),
                |tabs, index| {
                    tabs.item(TabsItem::new(
                        format!("tab-{index}"),
                        format!("Tab {index}"),
                        div().child(format!("Panel {index}")),
                    ))
                },
            );

            div()
                .size_full()
                .child(div().w(px(280.0)).h(px(120.0)).child(tabs))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_before = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should be rendered before scrolling");
    let tablist = cx
        .debug_bounds("tabs:overflow-tabs:tablist")
        .expect("tablist should be rendered");
    let tablist_viewport = cx
        .debug_bounds("scroll-area:tabs:overflow-tabs:tablist-scroll")
        .expect("vertical tablist should use the shared ScrollArea viewport");

    assert!(
        tablist.contains(&tablist_viewport.center()),
        "expected ScrollArea viewport to stay inside the tablist shell; tablist={tablist:?} viewport={tablist_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: tablist_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-64.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let tab_after = cx
        .debug_bounds("tabs:overflow-tabs:trigger:tab-3")
        .expect("tab trigger should remain rendered after scrolling");

    assert!(
        tab_after.top() < tab_before.top(),
        "expected constrained vertical tablist to scroll; before={tab_before:?} after={tab_after:?}"
    );
}

#[open_gpui::test]
fn tabs_runtime_manual_keyboard_activation_preserves_selected_seed_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<TabsSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Tabs::new("runtime-tabs")
                    .activation_mode(TabsActivationMode::Manual)
                    .default_selected("details")
                    .item(TabsItem::new(
                        "overview",
                        "Overview",
                        div()
                            .debug_selector(|| "tabs-panel:overview".to_string())
                            .child("Overview panel"),
                    ))
                    .item(
                        TabsItem::new(
                            "billing",
                            "Billing",
                            div()
                                .debug_selector(|| "tabs-panel:billing".to_string())
                                .child("Billing panel"),
                        )
                        .disabled(true),
                    )
                    .item(TabsItem::new(
                        "details",
                        "Details",
                        div()
                            .debug_selector(|| "tabs-panel:details".to_string())
                            .child("Details panel"),
                    ))
                    .item(TabsItem::new(
                        "history",
                        "History",
                        div()
                            .debug_selector(|| "tabs-panel:history".to_string())
                            .child("History panel"),
                    ))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "expected seeded selected tab to render the Details panel"
    );

    let disabled_billing = cx
        .debug_bounds("tabs:runtime-tabs:trigger:billing")
        .expect("disabled Billing tab trigger should be rendered");
    cx.simulate_click(disabled_billing.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled tab click should not emit a selection change"
    );
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "disabled tab click should keep the current selected panel"
    );

    let overview = cx
        .debug_bounds("tabs:runtime-tabs:trigger:overview")
        .expect("Overview tab trigger should be rendered");
    cx.simulate_click(overview.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 0);
    assert_eq!(after_click[0].value(), "overview");
    assert_eq!(after_click[0].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "enabled tab click should render the selected panel"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(
        selections.borrow().len(),
        1,
        "manual activation should move roving focus without selecting on arrow key"
    );
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "manual activation should keep the selected panel until Enter or Space"
    );

    cx.simulate_keystrokes("enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_enter = selections.borrow().clone();
    assert_eq!(after_enter.len(), 2);
    assert_eq!(after_enter[1].index(), 2);
    assert_eq!(after_enter[1].value(), "details");
    assert_eq!(after_enter[1].label(), "Details");
    assert!(
        cx.debug_bounds("tabs-panel:details").is_some(),
        "Enter should activate the focused tab after keyboard navigation skips disabled tabs"
    );

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 3);
    assert_eq!(after_home[2].index(), 0);
    assert_eq!(after_home[2].value(), "overview");
    assert_eq!(after_home[2].label(), "Overview");
    assert!(
        cx.debug_bounds("tabs-panel:overview").is_some(),
        "Home plus Enter should activate the first enabled tab in manual mode"
    );

    cx.simulate_keystrokes("end space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(after_space.len(), 4);
    assert_eq!(after_space[3].index(), 3);
    assert_eq!(after_space[3].value(), "history");
    assert_eq!(after_space[3].label(), "History");
    assert!(
        cx.debug_bounds("tabs-panel:history").is_some(),
        "End plus Space should activate the last enabled tab in manual mode"
    );
}

#[open_gpui::test]
fn toolbar_runtime_keyboard_navigation_skips_disabled_and_separator_items(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<ToolbarSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                Toolbar::new("keyboard-toolbar", "Keyboard toolbar")
                    .small()
                    .default_focused("bold")
                    .item(ToolbarItem::icon("undo", "U", "Undo"))
                    .item(ToolbarItem::icon("redo", "R", "Redo").disabled(true))
                    .item(ToolbarItem::separator("history-separator"))
                    .item(ToolbarItem::toggle_icon("bold", "B", "Bold").pressed(true))
                    .item(ToolbarItem::toggle_icon("italic", "I", "Italic"))
                    .on_select(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let undo = cx
        .debug_bounds("toolbar:keyboard-toolbar:item:undo")
        .expect("undo toolbar item should be rendered");
    cx.simulate_click(undo.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 2);
    assert_eq!(after_right[0].value(), "undo");
    assert_eq!(after_right[0].kind(), ToolbarItemKind::Action);
    assert_eq!(after_right[1].value(), "bold");
    assert_eq!(after_right[1].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("right enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_second_right = selections.borrow().clone();
    assert_eq!(after_second_right.len(), 3);
    assert_eq!(after_second_right[2].value(), "italic");
    assert_eq!(after_second_right[2].kind(), ToolbarItemKind::Toggle);

    cx.simulate_keystrokes("home enter");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_home = selections.borrow().clone();
    assert_eq!(after_home.len(), 4);
    assert_eq!(after_home[3].value(), "undo");
    assert_eq!(after_home[3].kind(), ToolbarItemKind::Action);
}

#[open_gpui::test]
fn toggle_group_controlled_values_override_runtime_selection(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        changes: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();

            div().size_full().child(
                ToggleGroup::new("controlled-toggle-group", "Alignment")
                    .default_selected_values(["right"])
                    .selected_values(Vec::<String>::new())
                    .item(ToggleGroupItem::new("left", "Left"))
                    .item(ToggleGroupItem::new("right", "Right"))
                    .on_change(move |change, _, _| {
                        changes.borrow_mut().push(change.selected_values().to_vec());
                    }),
            )
        }
    }

    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should expose a stable debug selector");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(changes.borrow().as_slice(), &[vec!["left".to_string()]]);

    let left = cx
        .debug_bounds("toggle-group:controlled-toggle-group:item:left")
        .expect("left toggle item should remain rendered after controlled redraw");
    cx.simulate_click(left.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(
        changes.borrow().as_slice(),
        &[vec!["left".to_string()], vec!["left".to_string()]],
        "controlled empty selection should reset adapter runtime before each activation"
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

#[open_gpui::test]
fn radio_group_runtime_keyboard_navigation_skips_disabled_items_and_payloads(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        selections: Rc<RefCell<Vec<RadioSelection>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selections = self.selections.clone();

            div().size_full().child(
                RadioGroup::new("runtime-radio")
                    .label("Runtime radio")
                    .orientation(Orientation::Horizontal)
                    .default_selected("personal")
                    .item(RadioItem::new("personal", "Personal"))
                    .item(RadioItem::new("team", "Team").disabled(true))
                    .item(RadioItem::new("enterprise", "Enterprise"))
                    .on_selection_change(move |selection, _, _| {
                        selections.borrow_mut().push(selection);
                    }),
            )
        }
    }

    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        selections: selections.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(
        cx.debug_bounds("radio-group:runtime-radio").is_some(),
        "radio group root should expose a stable debug selector"
    );

    let disabled_team = cx
        .debug_bounds("radio-group:runtime-radio:item:team")
        .expect("disabled Team radio item should be rendered");
    cx.simulate_click(disabled_team.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        selections.borrow().is_empty(),
        "disabled radio click should not emit a selection change"
    );

    let enterprise = cx
        .debug_bounds("radio-group:runtime-radio:item:enterprise")
        .expect("Enterprise radio item should be rendered");
    cx.simulate_click(enterprise.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_click = selections.borrow().clone();
    assert_eq!(after_click.len(), 1);
    assert_eq!(after_click[0].index(), 2);
    assert_eq!(after_click[0].value(), "enterprise");
    assert_eq!(after_click[0].label(), "Enterprise");

    cx.simulate_keystrokes("left");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_left = selections.borrow().clone();
    assert_eq!(after_left.len(), 2);
    assert_eq!(after_left[1].index(), 0);
    assert_eq!(after_left[1].value(), "personal");
    assert_eq!(after_left[1].label(), "Personal");

    cx.simulate_keystrokes("space");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_space = selections.borrow().clone();
    assert_eq!(
        after_space.len(),
        2,
        "Space on the already selected radio should not emit a duplicate selection change"
    );

    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    let after_right = selections.borrow().clone();
    assert_eq!(after_right.len(), 3);
    assert_eq!(after_right[2].index(), 2);
    assert_eq!(after_right[2].value(), "enterprise");
    assert_eq!(after_right[2].label(), "Enterprise");
}

#[test]
fn radio_group_state_exposes_selection_required_and_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Vertical,
        Size::Medium,
        false,
        true,
        Some("team"),
        None,
        [
            RadioItemDescriptor::new("personal", "Personal"),
            RadioItemDescriptor::new("team", "Team"),
            RadioItemDescriptor::new("enterprise", "Enterprise").disabled(true),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::RadioGroup);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("team"));
    assert_eq!(state.focused_value(), Some("team"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items().len(), 3);
    assert!(state.items()[1].selected());
    assert!(state.items()[1].focused());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[0].role(), Role::RadioButton);
}

#[test]
fn radio_group_reuses_roving_focus_helpers_and_skips_disabled_items() {
    let state = RadioGroupState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        false,
        Some("missing"),
        Some("enterprise"),
        [
            RadioItemDescriptor::new("starter", "Starter"),
            RadioItemDescriptor::new("pro", "Pro").disabled(true),
            RadioItemDescriptor::new("enterprise", "Enterprise"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("enterprise"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[1].disabled());
    assert!(!state.items()[1].focused());
}

#[test]
fn radio_group_builder_state_falls_back_to_first_enabled_item() {
    let state = RadioGroup::new("plan")
        .label("Plan")
        .orientation(Orientation::Horizontal)
        .with_size(Size::Large)
        .required(true)
        .default_selected("enterprise")
        .item(RadioItem::new("starter", "Starter"))
        .item(RadioItem::new("pro", "Pro"))
        .item(RadioItem::new("enterprise", "Enterprise").disabled(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Large);
    assert!(state.required());
    assert_eq!(state.selected_value(), Some("starter"));
    assert_eq!(state.focused_value(), Some("starter"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[2].disabled());
    assert!(!state.items()[2].selected());
}

#[test]
fn toggle_pressed_state_maps_to_button_role_and_toggled_state() {
    let state = Toggle::new("notifications", "Notifications")
        .variant(ToggleVariant::Outline)
        .pressed(true)
        .small()
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::True);
    assert!(state.pressed());
    assert_eq!(state.variant(), ToggleVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.colors().background().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn disabled_toggle_blocks_activation_without_checkbox_semantics() {
    let state = Toggle::new("locked", "Locked")
        .pressed(false)
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.toggled(), Toggled::False);
    assert!(!state.pressed());
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn badge_variants_resolve_display_only_token_intents() {
    let default = Badge::new("status", "Live").state();
    let secondary = Badge::new("beta", "Beta")
        .variant(BadgeVariant::Secondary)
        .small()
        .state();
    let destructive = Badge::new("risk", "Risk")
        .variant(BadgeVariant::Destructive)
        .state();
    let outline = Badge::new("neutral", "Neutral")
        .variant(BadgeVariant::Outline)
        .state();

    assert_eq!(default.variant(), BadgeVariant::Default);
    assert!(default.display_only());
    assert_eq!(default.role(), None);
    assert_eq!(default.colors().background().token(), semantic::ACCENT);
    assert_eq!(secondary.size(), Size::Small);
    assert_eq!(
        secondary.colors().background().token(),
        semantic::SURFACE_MUTED
    );
    assert_eq!(
        destructive.colors().background().token(),
        semantic::DESTRUCTIVE
    );
    assert_eq!(outline.colors().border().token(), semantic::BORDER);
}

#[test]
fn icon_button_requires_accessible_label_and_reuses_button_primitives() {
    let button = IconButton::new("search", "?", "Search")
        .variant(ButtonVariant::Outline)
        .small();
    let state = button.state();

    assert_eq!(button.accessible_label(), "Search");
    assert_eq!(state.role(), Role::Button);
    assert_eq!(state.variant(), ButtonVariant::Outline);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.metrics().size(), Size::Small.icon_button_size());
    assert_eq!(state.metrics().icon_size(), Size::Small.icon_size());
    assert_eq!(state.colors().border().token(), semantic::BORDER);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(state.activation_enabled());
}

#[test]
fn sidebar_state_exposes_shell_navigation_contract() {
    let state = SidebarState::resolve(
        SidebarSide::Left,
        SidebarVariant::Docked,
        SidebarCollapseMode::Icon,
        false,
        false,
        "Primary navigation",
        Some("projects"),
        None,
        [
            SidebarSectionDescriptor::new("workspace", "Workspace").items([
                SidebarItemDescriptor::new("home", "Home").icon("H"),
                SidebarItemDescriptor::new("projects", "Projects")
                    .icon("P")
                    .badge("12"),
                SidebarItemDescriptor::new("archive", "Archive")
                    .icon("A")
                    .disabled(true),
            ]),
            SidebarSectionDescriptor::new("account", "Account").items([
                SidebarItemDescriptor::new("settings", "Settings").icon("S"),
                SidebarItemDescriptor::new("billing", "Billing")
                    .icon("B")
                    .action_label("new"),
            ]),
        ],
        Size::Medium,
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Navigation);
    assert_eq!(state.side(), SidebarSide::Left);
    assert_eq!(state.variant(), SidebarVariant::Docked);
    assert_eq!(state.collapse_mode(), SidebarCollapseMode::Icon);
    assert!(!state.collapsed());
    assert_eq!(state.sections().len(), 2);
    assert_eq!(state.sections()[0].role(), Role::Section);
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.selected_value(), Some("projects"));
    assert_eq!(state.focused_value(), Some("projects"));
    assert_eq!(state.focused_index(), Some(1));
    assert!(state.scrollable());
    assert!(state.items()[1].selected());
    assert_eq!(state.items()[1].badge_label(), Some("12"));
    assert!(!state.items()[2].activation_enabled());
    assert_eq!(state.items()[1].role(), Role::Button);
    assert_eq!(state.items()[1].position_in_set(), Some(2));
    assert_eq!(state.items()[1].size_of_set(), 4);
    assert_eq!(
        state.navigation_target("down").map(|item| item.value()),
        Some("settings")
    );
    assert_eq!(
        state
            .activation_for_key("enter")
            .map(|selection| selection.value().to_owned()),
        Some("projects".to_string())
    );
}

#[test]
fn sidebar_icon_collapse_keeps_accessible_items_but_hides_text() {
    let state = Sidebar::new("app-sidebar", "Application")
        .collapse_mode(SidebarCollapseMode::Icon)
        .collapsed(true)
        .selected("dashboard")
        .section(
            SidebarSection::new("main", "Main")
                .item(SidebarItem::new("dashboard", "Dashboard").icon("D"))
                .item(SidebarItem::new("inbox", "Inbox").icon("I").badge("4")),
        )
        .state();

    assert!(state.collapsed());
    assert!(state.icon_collapsed());
    assert!(!state.offcanvas_collapsed());
    assert_eq!(
        state.metrics().resolved_width(),
        state.metrics().collapsed_width()
    );
    assert_eq!(state.selected_value(), Some("dashboard"));
    assert_eq!(state.focused_value(), Some("dashboard"));
    assert!(state.scrollable());
    assert!(state.items()[0].focusable());
    assert_eq!(state.items()[0].label(), "Dashboard");
    assert_eq!(state.items()[1].badge_label(), Some("4"));
}

#[test]
fn sidebar_offcanvas_collapse_removes_items_from_roving_focus() {
    let state = SidebarState::resolve(
        SidebarSide::Right,
        SidebarVariant::Floating,
        SidebarCollapseMode::Offcanvas,
        true,
        false,
        "Secondary navigation",
        Some("reports"),
        None,
        [SidebarSectionDescriptor::new("main", "Main").items([
            SidebarItemDescriptor::new("overview", "Overview"),
            SidebarItemDescriptor::new("reports", "Reports"),
        ])],
        Size::Small,
        ThemeTokens::default(),
    );

    assert!(state.collapsed());
    assert!(state.offcanvas_collapsed());
    assert_eq!(state.metrics().resolved_width(), ui_px(0.0));
    assert_eq!(state.selected_value(), None);
    assert_eq!(state.focused_value(), None);
    assert_eq!(state.focused_index(), None);
    assert!(!state.scrollable());
    assert!(!state.items()[0].focusable());
    assert!(state.activation_for_key("space").is_none());
}

#[open_gpui::test]
fn sidebar_long_navigation_scrolls_inside_shared_scroll_area(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section = (0..14).fold(SidebarSection::new("main", "Main"), |section, index| {
                section.item(
                    SidebarItem::new(format!("item-{index}"), format!("Item {index}"))
                        .icon(index.to_string()),
                )
            });

            div().size_full().child(
                div()
                    .h(px(120.0))
                    .child(Sidebar::new("long-sidebar", "Long navigation").section(section)),
            )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_before = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should be rendered before scrolling");
    let sidebar = cx
        .debug_bounds("sidebar:long-sidebar")
        .expect("sidebar shell should be rendered");
    let sidebar_viewport = cx
        .debug_bounds("scroll-area:long-sidebar-scroll")
        .expect("long Sidebar should use the shared ScrollArea viewport");

    assert!(
        sidebar.contains(&sidebar_viewport.center()),
        "expected Sidebar ScrollArea viewport to stay inside the sidebar shell; sidebar={sidebar:?} viewport={sidebar_viewport:?}"
    );

    cx.simulate_event(ScrollWheelEvent {
        position: sidebar_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-72.0))),
        ..Default::default()
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let item_after = cx
        .debug_bounds("sidebar:long-sidebar:item:item-2")
        .expect("sidebar item should remain rendered after scrolling");

    assert!(
        item_after.top() < item_before.top(),
        "expected long Sidebar navigation to scroll inside its ScrollArea; before={item_before:?} after={item_after:?}"
    );
}

#[test]
fn sidebar_navigation_helper_skips_disabled_items() {
    assert_eq!(
        sidebar_navigation_target("down", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("up", 0, &[false, true, false]),
        Some(2)
    );
    assert_eq!(
        sidebar_navigation_target("home", 2, &[false, true, false]),
        Some(0)
    );
    assert_eq!(sidebar_navigation_target("right", 0, &[false, false]), None);
}

#[test]
fn toolbar_state_exposes_roving_focus_and_toggle_metadata() {
    let state = ToolbarState::resolve(
        Orientation::Horizontal,
        Size::Small,
        false,
        "Editor toolbar",
        Some("bold"),
        [
            ToolbarItemDescriptor::action("undo", "Undo"),
            ToolbarItemDescriptor::separator("history-separator"),
            ToolbarItemDescriptor::toggle("bold", "Bold").pressed(true),
            ToolbarItemDescriptor::toggle("italic", "Italic").disabled(true),
            ToolbarItemDescriptor::action("save", "Save"),
        ],
        ThemeTokens::default(),
    );

    assert_eq!(state.role(), Role::Toolbar);
    assert_eq!(state.orientation(), Orientation::Horizontal);
    assert_eq!(state.size(), Size::Small);
    assert_eq!(state.label(), "Editor toolbar");
    assert_eq!(state.items().len(), 5);
    assert_eq!(state.focused_value(), Some("bold"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert_eq!(state.items()[0].role(), Some(Role::Button));
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert_eq!(state.items()[1].role(), None);
    assert!(!state.items()[1].focusable());
    assert!(state.items()[2].pressed());
    assert_eq!(state.items()[2].toggled(), Some(Toggled::True));
    assert!(!state.items()[3].activation_enabled());
    assert_eq!(
        state.navigation_target("right").map(|item| item.value()),
        Some("save")
    );
    assert_eq!(
        state
            .activation_for_key("space")
            .map(|selection| (selection.value().to_owned(), selection.kind())),
        Some(("bold".to_string(), ToolbarItemKind::Toggle))
    );
}

#[test]
fn toolbar_builder_state_skips_disabled_and_separator_items() {
    let state = Toolbar::new("editor-tools", "Editor")
        .orientation(Orientation::Vertical)
        .large()
        .default_focused("missing")
        .item(ToolbarItem::action("cut", "Cut").disabled(true))
        .item(ToolbarItem::separator("clipboard-separator"))
        .item(ToolbarItem::icon("copy", "C", "Copy"))
        .item(ToolbarItem::toggle("wrap", "Wrap").pressed(true))
        .state();

    assert_eq!(state.orientation(), Orientation::Vertical);
    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.focused_value(), Some("copy"));
    assert_eq!(state.tab_stop_index(), state.focused_index());
    assert!(state.items()[0].disabled());
    assert_eq!(state.items()[1].kind(), ToolbarItemKind::Separator);
    assert!(state.items()[3].pressed());
    assert_eq!(
        toolbar_navigation_target(
            Orientation::Vertical,
            "down",
            state.focused_index().unwrap(),
            &[true, true, false, false],
        ),
        Some(3)
    );
}

#[test]
fn disabled_icon_button_blocks_activation_metadata() {
    let state = IconButton::new("locked", "x", "Locked")
        .disabled(true)
        .state();

    assert_eq!(state.role(), Role::Button);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn avatar_fallback_initials_derive_from_display_names_and_empty_names() {
    let ada = Avatar::new("ada", "Ada Lovelace").state();
    let single = Avatar::new("single", "Grace").state();
    let trio = Avatar::new("trio", "Foo Bar Dar").state();
    let empty = Avatar::new("empty", "  ").state();

    assert_eq!(ada.name(), "Ada Lovelace");
    assert_eq!(ada.fallback(), "AL");
    assert_eq!(ada.accessible_label(), "Ada Lovelace");
    assert_eq!(ada.role(), Role::Image);

    assert_eq!(single.fallback(), "GR");
    assert_eq!(trio.fallback(), "FB");
    assert_eq!(empty.fallback(), "?");
    assert_eq!(empty.accessible_label(), "Avatar");
}

#[test]
fn avatar_explicit_fallback_overrides_derived_initials() {
    let state = Avatar::new("current-user", "Ada Lovelace")
        .fallback("ME")
        .state();

    assert_eq!(state.name(), "Ada Lovelace");
    assert_eq!(state.fallback(), "ME");
}

#[test]
fn avatar_source_metadata_does_not_own_loading_state() {
    let state = Avatar::new("profile", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .state();

    assert!(state.has_source());
    assert_eq!(
        state.source().map(|source| source.uri()),
        Some("asset://avatars/ada.png")
    );
    assert_eq!(state.fallback(), "AL");
    assert_eq!(state.accessible_label(), "Ada Lovelace");
}

#[test]
fn avatar_accessible_label_can_be_explicit_for_source_and_fallback_avatars() {
    let fallback = Avatar::new("fallback-avatar", "Ada Lovelace")
        .accessible_label("Current user")
        .state();
    let source = Avatar::new("source-avatar", "Ada Lovelace")
        .source("asset://avatars/ada.png")
        .accessible_label("Ada profile photo")
        .state();

    assert_eq!(fallback.accessible_label(), "Current user");
    assert_eq!(source.accessible_label(), "Ada profile photo");
}

#[test]
fn avatar_size_metrics_and_token_intents_are_stable() {
    let tokens = custom_tokens();
    let small = Avatar::new("small-avatar", "Ada")
        .small()
        .tokens(tokens)
        .state();
    let medium = Avatar::new("medium-avatar", "Ada").tokens(tokens).state();
    let large = Avatar::new("large-avatar", "Ada")
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(small.size(), Size::Small);
    assert_eq!(small.metrics().diameter(), ui_px(28.0));
    assert_eq!(small.metrics().text_size(), ui_px(11.0));

    assert_eq!(medium.metrics().diameter(), ui_px(32.0));
    assert_eq!(medium.metrics().radius(), ui_px(16.0));

    assert_eq!(large.metrics().diameter(), ui_px(40.0));
    assert_eq!(large.metrics().text_size(), ui_px(14.0));
    assert_eq!(large.colors().background().token(), tokens.surface_muted);
    assert_eq!(large.colors().foreground().token(), tokens.text);
    assert_eq!(large.colors().border().token(), tokens.border);
}

#[test]
fn avatar_group_state_tracks_visible_and_hidden_counts() {
    let group = AvatarGroup::new("team")
        .avatar(Avatar::new("ada", "Ada Lovelace"))
        .avatar(Avatar::new("grace", "Grace Hopper"))
        .avatar(Avatar::new("katherine", "Katherine Johnson"))
        .avatar(Avatar::new("margaret", "Margaret Hamilton"))
        .max_visible(3)
        .tokens(custom_tokens());
    let state = group.state();

    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.total_count(), 4);
    assert_eq!(state.visible_count(), 3);
    assert_eq!(state.hidden_count(), 1);

    let count = AvatarGroupCount::new("team-count", state.hidden_count())
        .with_size(state.size())
        .tokens(custom_tokens());
    let count_state = count.state();

    assert_eq!(count_state.count(), 1);
    assert_eq!(count_state.size(), Size::Medium);
    assert_eq!(count_state.role(), Role::Label);
}

#[open_gpui::test]
fn avatar_renders_stable_debug_selector(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Avatar::new("runtime-avatar", "Ada Lovelace"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert!(cx.debug_bounds("avatar:runtime-avatar:root").is_some());
}

#[test]
fn separator_state_exposes_orientation_role_and_decorative_mode() {
    let horizontal = Separator::new("section-separator").state();
    let vertical = Separator::new("panel-separator").vertical().large().state();
    let decorative = Separator::new("decorative-separator")
        .decorative(true)
        .state();

    assert_eq!(horizontal.orientation(), Orientation::Horizontal);
    assert_eq!(horizontal.role(), Some(Role::Separator));
    assert_eq!(horizontal.metrics().thickness(), ui_px(1.0));
    assert_eq!(horizontal.colors().line().token(), semantic::BORDER);

    assert_eq!(vertical.orientation(), Orientation::Vertical);
    assert_eq!(vertical.role(), Some(Role::Separator));
    assert_eq!(vertical.metrics().thickness(), ui_px(2.0));

    assert!(decorative.decorative());
    assert_eq!(decorative.role(), None);
}

#[test]
fn kbd_state_is_display_only_with_muted_token_intents() {
    let tokens = custom_tokens();
    let state = Kbd::new("command-shortcut", "Ctrl+K")
        .small()
        .tokens(tokens)
        .state();

    assert_eq!(state.label(), "Ctrl+K");
    assert_eq!(state.size(), Size::Small);
    assert!(state.display_only());
    assert_eq!(state.metrics().min_width(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
    assert_eq!(state.colors().foreground().token(), tokens.text_muted);
    assert_eq!(state.colors().border().token(), tokens.border);
}

#[test]
fn progress_state_clamps_values_and_preserves_indeterminate_mode() {
    let full = Progress::new("upload-progress", "Upload")
        .value(142.0)
        .large()
        .state();
    let empty = Progress::new("empty-progress", "Empty")
        .value(f32::NAN)
        .state();
    let indeterminate = Progress::new("pending-progress", "Pending")
        .indeterminate()
        .state();

    assert_eq!(full.role(), Role::ProgressIndicator);
    assert_eq!(full.value_percent(), Some(100.0));
    assert_eq!(full.normalized_value(), Some(1.0));
    assert_eq!(
        full.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 1.0
        }
    );
    assert_eq!(full.indicator_start_fraction(), 0.0);
    assert_eq!(full.indicator_fraction(), 1.0);
    assert_eq!(full.metrics().height(), ui_px(10.0));
    assert_eq!(full.colors().track().token(), semantic::SURFACE_MUTED);
    assert_eq!(full.colors().indicator().token(), semantic::ACCENT);

    assert_eq!(empty.value_percent(), Some(0.0));
    assert_eq!(empty.normalized_value(), Some(0.0));
    assert_eq!(
        empty.visual_mode(),
        ProgressVisualMode::Determinate {
            normalized_value: 0.0
        }
    );
    assert!(indeterminate.indeterminate());
    assert_eq!(indeterminate.value_percent(), None);
    assert_eq!(indeterminate.normalized_value(), None);
    assert_eq!(
        indeterminate.visual_mode(),
        ProgressVisualMode::Indeterminate
    );
    assert!(
        indeterminate.indicator_start_fraction() > 0.0,
        "indeterminate progress should not look like a left-anchored determinate fill"
    );
    assert!(
        indeterminate.indicator_fraction() > 0.0 && indeterminate.indicator_fraction() < 0.5,
        "indeterminate progress should render as a short segment, not as a fixed percentage value"
    );
}

#[test]
fn skeleton_state_is_noninteractive_placeholder_with_stable_metrics() {
    let tokens = custom_tokens();
    let state = Skeleton::new("loading-line")
        .subtle(true)
        .large()
        .tokens(tokens)
        .state();

    assert_eq!(state.size(), Size::Large);
    assert!(state.subtle());
    assert!(state.display_only());
    assert_eq!(state.metrics().width(), ui_px(224.0));
    assert_eq!(state.metrics().height(), ui_px(20.0));
    assert_eq!(state.colors().background().token(), tokens.surface_muted);
}

#[open_gpui::test]
fn low_state_primitives_render_stable_debug_selectors(cx: &mut open_gpui::TestAppContext) {
    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(Separator::new("runtime-separator"))
                .child(Kbd::new("runtime-kbd", "Ctrl+K"))
                .child(Progress::new("runtime-progress", "Loading").value(40.0))
                .child(Progress::new("runtime-progress-indeterminate", "Indexing").indeterminate())
                .child(Skeleton::new("runtime-skeleton"))
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TestView);
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    for selector in [
        "separator:runtime-separator:root",
        "kbd:runtime-kbd:root",
        "progress:runtime-progress:root",
        "progress:runtime-progress:indicator",
        "progress:runtime-progress-indeterminate:root",
        "progress:runtime-progress-indeterminate:indicator",
        "skeleton:runtime-skeleton:root",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should be rendered"
        );
    }

    let determinate_root = cx
        .debug_bounds("progress:runtime-progress:root")
        .expect("determinate progress root should render");
    let determinate_indicator = cx
        .debug_bounds("progress:runtime-progress:indicator")
        .expect("determinate progress indicator should render");
    let indeterminate_root = cx
        .debug_bounds("progress:runtime-progress-indeterminate:root")
        .expect("indeterminate progress root should render");
    let indeterminate_indicator = cx
        .debug_bounds("progress:runtime-progress-indeterminate:indicator")
        .expect("indeterminate progress indicator should render");

    let determinate_width =
        determinate_indicator.size.width.as_f32() / determinate_root.size.width.as_f32();
    let indeterminate_start = (indeterminate_indicator.left().as_f32()
        - indeterminate_root.left().as_f32())
        / indeterminate_root.size.width.as_f32();
    let indeterminate_width =
        indeterminate_indicator.size.width.as_f32() / indeterminate_root.size.width.as_f32();

    assert!(
        (determinate_width - 0.4).abs() < 0.02,
        "determinate progress indicator should match the provided value"
    );
    assert!(
        indeterminate_start > 0.25,
        "indeterminate progress indicator should not be left-anchored"
    );
    assert!(
        indeterminate_width > 0.25 && indeterminate_width < 0.45,
        "indeterminate progress indicator should be a short segment"
    );
}

#[test]
fn button_accepts_custom_token_bundle() {
    let tokens = custom_tokens();
    let state = Button::new("outline", "Outline")
        .variant(ButtonVariant::Outline)
        .tokens(tokens)
        .state();

    assert_eq!(state.colors().border().token(), tokens.border);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
}

#[test]
fn theme_resolver_keeps_token_intent_and_resolves_fallback_color() {
    let tokens = custom_tokens();
    let state = Button::new("default", "Default").tokens(tokens).state();
    let background = state.colors().background();

    assert_eq!(background.token(), tokens.accent);
    assert_eq!(background.state(), ColorState::Default);
    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(u32::from(ThemeResolver::resolve(background)), 0x1f7a66ff);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            background,
            ThemeSnapshot::dark()
        )),
        0x1f7a66ff
    );
}

#[test]
fn theme_resolver_prefers_runtime_theme_table_for_known_tokens() {
    let state = Button::new("default", "Default").state();
    let background = state.colors().background();
    let custom_colors = [ThemeColor::new(
        semantic::ACCENT,
        ColorState::Default,
        0x123456,
    )];
    let snapshot = ThemeSnapshot::new(ThemeMode::Light, 42, &custom_colors);

    assert_eq!(background.fallback_rgb(), 0x1f7a66);
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(background, snapshot)),
        0x123456ff
    );
    assert_eq!(snapshot.mode(), ThemeMode::Light);
    assert_eq!(snapshot.revision(), 42);
}

#[test]
fn default_theme_snapshots_expose_distinct_modes_and_revisions() {
    let light = ThemeSnapshot::light();
    let dark = ThemeSnapshot::dark();
    let high_contrast = ThemeSnapshot::high_contrast();

    assert_eq!(light.mode().as_str(), "light");
    assert_eq!(dark.mode().as_str(), "dark");
    assert_eq!(high_contrast.mode().as_str(), "high-contrast");
    assert!(light.revision() < dark.revision());
    assert!(dark.revision() < high_contrast.revision());
    assert_ne!(
        light.color_rgb(semantic::SURFACE, ColorState::Default),
        dark.color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_ne!(
        dark.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible),
        high_contrast.color_rgb(semantic::FOCUS_RING, ColorState::FocusVisible)
    );
}

#[test]
fn theme_registry_preloads_builtin_snapshots_without_global_theme_state() {
    let registry = ThemeRegistry::with_builtins();

    assert_eq!(
        registry
            .entries()
            .iter()
            .map(|entry| (
                entry.id(),
                entry.snapshot().mode(),
                entry.snapshot().revision()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("light", ThemeMode::Light, ThemeSnapshot::light().revision()),
            ("dark", ThemeMode::Dark, ThemeSnapshot::dark().revision()),
            (
                "high-contrast",
                ThemeMode::HighContrast,
                ThemeSnapshot::high_contrast().revision()
            ),
        ]
    );
    assert_eq!(
        registry
            .snapshot("dark")
            .and_then(|snapshot| snapshot.color_rgb(semantic::SURFACE, ColorState::Default)),
        ThemeSnapshot::dark().color_rgb(semantic::SURFACE, ColorState::Default)
    );
}

#[test]
fn theme_registry_registers_user_definition_with_fallback_diagnostics() {
    let mut registry = ThemeRegistry::with_builtins();
    let entry = registry
        .register(
            ThemeDefinition::new("forest", "Forest", ThemeMode::Dark, 9001)
                .fallback_mode(ThemeMode::Light)
                .color(ThemeColor::new(
                    semantic::ACCENT,
                    ColorState::Default,
                    0x227755,
                ))
                .color(ThemeColor::new(
                    semantic::ACCENT,
                    ColorState::Hover,
                    0x1b6044,
                )),
        )
        .expect("valid user theme definition should register");
    let snapshot = entry.snapshot();

    assert_eq!(entry.id(), "forest");
    assert_eq!(entry.label(), "Forest");
    assert_eq!(snapshot.mode(), ThemeMode::Dark);
    assert_eq!(snapshot.revision(), 9001);
    assert_eq!(
        entry.diagnostics().fallback_mode(),
        ThemeMode::Light,
        "the registry should record which built-in table filled omitted optional tokens"
    );
    assert!(
        entry.diagnostics().fallback_color_count() > 0,
        "omitted optional token/state entries should be filled from the fallback snapshot"
    );
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x227755)
    );
    assert_eq!(
        snapshot.color_rgb(semantic::SURFACE, ColorState::Default),
        ThemeSnapshot::light().color_rgb(semantic::SURFACE, ColorState::Default)
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            ColorIntent::new(semantic::ACCENT, 0x1f7a66),
            snapshot
        )),
        0x227755ff
    );
}

#[test]
fn theme_registry_rejects_missing_required_identity_fields() {
    let mut registry = ThemeRegistry::new();

    assert_eq!(
        registry.register(ThemeDefinition::draft()).unwrap_err(),
        ThemeValidationError::MissingId
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("  "))
            .unwrap_err(),
        ThemeValidationError::MissingId
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("brand"))
            .unwrap_err(),
        ThemeValidationError::MissingLabel
    );
    assert_eq!(
        registry
            .register(ThemeDefinition::draft().id("brand").label("Brand"))
            .unwrap_err(),
        ThemeValidationError::MissingMode
    );
    assert_eq!(
        registry
            .register(
                ThemeDefinition::draft()
                    .id("brand")
                    .label("Brand")
                    .mode(ThemeMode::Light)
            )
            .unwrap_err(),
        ThemeValidationError::MissingRevision
    );
}

#[test]
fn theme_registry_replaces_existing_definition_by_stable_id() {
    let mut registry = ThemeRegistry::new();

    registry
        .register(
            ThemeDefinition::new("brand", "Brand", ThemeMode::Light, 1).color(ThemeColor::new(
                semantic::ACCENT,
                ColorState::Default,
                0x111111,
            )),
        )
        .expect("initial theme should register");
    registry
        .register(
            ThemeDefinition::new("brand", "Brand refreshed", ThemeMode::Light, 2).color(
                ThemeColor::new(semantic::ACCENT, ColorState::Default, 0x222222),
            ),
        )
        .expect("theme refresh should replace by id");

    assert_eq!(registry.entries().len(), 1);
    let snapshot = registry
        .snapshot("brand")
        .expect("brand snapshot should exist");
    assert_eq!(snapshot.revision(), 2);
    assert_eq!(
        snapshot.color_rgb(semantic::ACCENT, ColorState::Default),
        Some(0x222222)
    );
}

#[test]
fn theme_registry_types_are_exported_from_root_and_prelude() {
    use open_gpui_ui_components::{self as root, prelude};

    let mut root_registry: root::ThemeRegistry = root::ThemeRegistry::with_builtins();
    let root_definition: root::ThemeDefinition =
        root::ThemeDefinition::new("root-brand", "Root brand", root::ThemeMode::Light, 7);
    let root_entry: root::ThemeRegistryEntry = root_registry
        .register(root_definition)
        .expect("root ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let root_diagnostics: root::ThemeRegistrationDiagnostics = root_entry.diagnostics();
    let root_error: root::ThemeValidationError = root::ThemeValidationError::MissingId;

    let mut prelude_registry: prelude::ThemeRegistry = prelude::ThemeRegistry::with_builtins();
    let prelude_definition: prelude::ThemeDefinition = prelude::ThemeDefinition::new(
        "prelude-brand",
        "Prelude brand",
        prelude::ThemeMode::Dark,
        8,
    );
    let prelude_entry: prelude::ThemeRegistryEntry = prelude_registry
        .register(prelude_definition)
        .expect("prelude ThemeRegistry should register exported ThemeDefinition")
        .clone();
    let prelude_diagnostics: prelude::ThemeRegistrationDiagnostics = prelude_entry.diagnostics();
    let prelude_error: prelude::ThemeValidationError = prelude::ThemeValidationError::MissingLabel;

    assert_eq!(root_entry.snapshot().revision(), 7);
    assert_eq!(prelude_entry.snapshot().revision(), 8);
    assert_eq!(root_diagnostics.fallback_mode(), root::ThemeMode::Light);
    assert!(root_diagnostics.fallback_color_count() > 0);
    assert_eq!(
        prelude_diagnostics.fallback_mode(),
        prelude::ThemeMode::Dark
    );
    assert_eq!(root_error, root::ThemeValidationError::MissingId);
    assert_eq!(prelude_error, prelude::ThemeValidationError::MissingLabel);
}

#[test]
fn default_theme_resolves_all_current_component_color_intents() {
    let theme = [
        ThemeSnapshot::light(),
        ThemeSnapshot::dark(),
        ThemeSnapshot::high_contrast(),
    ];
    let buttons = [
        Button::new("default", "Default").state(),
        Button::new("secondary", "Secondary")
            .variant(ButtonVariant::Secondary)
            .state(),
        Button::new("outline", "Outline")
            .variant(ButtonVariant::Outline)
            .state(),
        Button::new("ghost", "Ghost")
            .variant(ButtonVariant::Ghost)
            .state(),
        Button::new("destructive", "Destructive")
            .variant(ButtonVariant::Destructive)
            .state(),
        Button::new("selected", "Selected").selected(true).state(),
    ];
    let badges = [
        Badge::new("default-badge", "Default").state(),
        Badge::new("secondary-badge", "Secondary")
            .variant(BadgeVariant::Secondary)
            .state(),
        Badge::new("destructive-badge", "Destructive")
            .variant(BadgeVariant::Destructive)
            .state(),
        Badge::new("outline-badge", "Outline")
            .variant(BadgeVariant::Outline)
            .state(),
    ];
    let avatars = [
        Avatar::new("avatar", "Ada Lovelace").state(),
        Avatar::new("source-avatar", "Ada Lovelace")
            .source("asset://avatars/ada.png")
            .state(),
    ];
    let status_cues = [
        StatusCue::new("status-neutral", "Neutral").state(),
        StatusCue::new("status-info", "Info")
            .intent(FeedbackIntent::Info)
            .state(),
        StatusCue::new("status-success", "Success")
            .intent(FeedbackIntent::Success)
            .state(),
        StatusCue::new("status-warning", "Warning")
            .intent(FeedbackIntent::Warning)
            .state(),
        StatusCue::new("status-danger", "Danger")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let empty_states = [
        EmptyState::new("empty-neutral", "Neutral").state(),
        EmptyState::new("empty-danger", "Danger")
            .description("Needs action")
            .intent(FeedbackIntent::Danger)
            .state(),
    ];
    let icon_buttons = [
        IconButton::new("search", "?", "Search").state(),
        IconButton::new("outline-icon", "+", "Add")
            .variant(ButtonVariant::Outline)
            .state(),
        IconButton::new("danger-icon", "!", "Delete")
            .variant(ButtonVariant::Destructive)
            .state(),
    ];
    let switches = [
        Switch::new("off").state(),
        Switch::new("on").checked(true).state(),
    ];
    let checkboxes = [
        Checkbox::new("unchecked").state(),
        Checkbox::new("checked").checked(true).state(),
        Checkbox::new("mixed").indeterminate(true).state(),
        Checkbox::new("invalid").invalid(true).state(),
    ];
    let radio_groups = [
        RadioGroup::new("plan")
            .default_selected("team")
            .item(RadioItem::new("personal", "Personal"))
            .item(RadioItem::new("team", "Team"))
            .state(),
        RadioGroup::new("disabled-plan")
            .disabled(true)
            .item(RadioItem::new("personal", "Personal"))
            .state(),
    ];
    let toggles = [
        Toggle::new("ghost-off", "Ghost off").state(),
        Toggle::new("ghost-on", "Ghost on").pressed(true).state(),
        Toggle::new("outline-on", "Outline on")
            .variant(ToggleVariant::Outline)
            .pressed(true)
            .state(),
    ];
    let text_inputs = [
        TextInput::new("default", "Default").state(),
        TextInput::new("disabled", "Disabled")
            .disabled(true)
            .state(),
        TextInput::new("readonly", "Read only")
            .read_only(true)
            .state(),
        TextInput::new("invalid", "Invalid").invalid(true).state(),
    ];
    let fields = [
        Field::new("field", "control", "Field").state(),
        Field::new("required", "control", "Required")
            .required(true)
            .state(),
        Field::new("disabled", "control", "Disabled")
            .disabled(true)
            .state(),
        Field::new("invalid", "control", "Invalid")
            .invalid(true)
            .state(),
    ];
    let labels = [
        Label::new("label", "Label").state(),
        Label::new("required-label", "Required")
            .required(true)
            .state(),
        Label::new("disabled-label", "Disabled")
            .disabled(true)
            .state(),
    ];
    let separators = [
        Separator::new("separator").state(),
        Separator::new("vertical-separator").vertical().state(),
    ];
    let kbds = [
        Kbd::new("kbd", "Ctrl+K").state(),
        Kbd::new("large-kbd", "Enter").large().state(),
    ];
    let progress = [
        Progress::new("progress", "Progress").value(50.0).state(),
        Progress::new("indeterminate-progress", "Progress")
            .indeterminate()
            .state(),
    ];
    let skeletons = [
        Skeleton::new("skeleton").state(),
        Skeleton::new("subtle-skeleton").subtle(true).state(),
    ];
    let menus = [
        Menu::new("menu", "Menu")
            .open(true)
            .item(MenuItem::action("open", "Open"))
            .state(),
        Menu::new("closed-menu", "Closed")
            .item(MenuItem::action("open", "Open"))
            .state(),
    ];
    let alert_dialogs = [
        AlertDialog::new(
            "alert",
            "Open",
            "Confirm",
            "Continue with changes.",
            "Continue",
        )
        .open(true)
        .state(),
        AlertDialog::new(
            "danger-alert",
            "Delete",
            "Delete item?",
            "This removes it.",
            "Delete",
        )
        .intent(AlertDialogIntent::Destructive)
        .open(true)
        .state(),
    ];
    let sheets = [
        Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content")
            .open(true)
            .state(),
        Sheet::new("closed-sheet", "Closed sheet", "Closed", "Closed content").state(),
    ];
    let hover_cards = [
        HoverCard::new("hover-card", "Profile", "Profile details")
            .open(true)
            .state(),
        HoverCard::element("closed-hover-card", "Details", div().child("Rich")).state(),
    ];
    let listboxes = [
        Listbox::new("listbox", "Choices")
            .selected("one")
            .option(ListboxOption::new("one", "One"))
            .option(ListboxOption::new("two", "Two").disabled(true))
            .state(),
        Listbox::new("empty-listbox", "Empty").state(),
    ];
    let selects = [
        Select::new("select", "Choice")
            .open(true)
            .selected("one")
            .option(ListboxOption::new("one", "One"))
            .state(),
        Select::new("closed-select", "Choice").state(),
    ];
    let comboboxes = [
        Combobox::new("combobox", "Search")
            .open(true)
            .default_query("one")
            .option(ComboboxOption::new("one", "One"))
            .state(),
        Combobox::new("closed-combobox", "Search").state(),
    ];
    let commands = [
        Command::new("command", "Commands")
            .open(true)
            .default_query("open")
            .item(CommandItem::new("open", "Open"))
            .state(),
        Command::new("closed-command", "Commands").state(),
    ];
    let table_toolbars = [
        TableToolbar::new("table-toolbar", "Filters")
            .summary("2 filtered")
            .state(),
        TableToolbar::new("small-table-toolbar", "Filters")
            .small()
            .state(),
    ];

    for state in buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in badges {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in avatars {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in status_cues {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in empty_states {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.marker(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in icon_buttons {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in switches {
        let colors = state.colors();
        for intent in [
            colors.track(),
            colors.thumb(),
            colors.border(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in checkboxes {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.hover_background(),
            colors.border(),
            colors.indicator(),
            colors.label(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in radio_groups {
        let colors = state.colors();
        for intent in [
            colors.control_background(),
            colors.control_background_selected(),
            colors.control_border(),
            colors.control_border_selected(),
            colors.indicator(),
            colors.label(),
            colors.label_muted(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in toggles {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.border(),
            colors.hover_background(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in text_inputs {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.placeholder(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in fields {
        let colors = state.colors();
        for intent in [colors.label(), colors.message(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in labels {
        let colors = state.colors();
        for intent in [colors.text(), colors.required_marker()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in separators {
        let colors = state.colors();
        for intent in [colors.line()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in kbds {
        let colors = state.colors();
        for intent in [colors.background(), colors.foreground(), colors.border()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in progress {
        let colors = state.colors();
        for intent in [colors.track(), colors.indicator()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in skeletons {
        let colors = state.colors();
        for intent in [colors.background()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in menus {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.border(),
            colors.item_background(),
            colors.item_hover_background(),
            colors.item_focus_background(),
            colors.item_disabled_foreground(),
            colors.separator(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in alert_dialogs {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.action_background(),
            colors.action_hover_background(),
            colors.action_foreground(),
            colors.action_border(),
            colors.cancel_background(),
            colors.cancel_hover_background(),
            colors.cancel_foreground(),
            colors.cancel_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in sheets {
        let colors = state.colors();
        for intent in [
            colors.barrier(),
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.close_background(),
            colors.close_hover_background(),
            colors.close_foreground(),
            colors.close_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in hover_cards {
        let colors = state.colors();
        for intent in [
            colors.background(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in listboxes {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.option_background(),
            colors.option_hover_background(),
            colors.option_active_background(),
            colors.option_selected_background(),
            colors.option_disabled_foreground(),
            colors.separator(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in selects {
        let colors = state.colors();
        for intent in [
            colors.trigger_background(),
            colors.trigger_hover_background(),
            colors.trigger_foreground(),
            colors.trigger_placeholder_foreground(),
            colors.trigger_border(),
            colors.content_background(),
            colors.content_foreground(),
            colors.content_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in comboboxes {
        let colors = state.colors();
        for intent in [
            colors.popup_background(),
            colors.popup_foreground(),
            colors.popup_border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in commands {
        let colors = state.colors();
        for intent in [
            colors.surface(),
            colors.foreground(),
            colors.muted_foreground(),
            colors.border(),
            colors.focus_ring(),
        ] {
            assert_theme_has_exact_color(theme, intent);
        }
    }

    for state in table_toolbars {
        let colors = state.colors();
        for intent in [colors.foreground(), colors.muted_foreground()] {
            assert_theme_has_exact_color(theme, intent);
        }
    }
}

fn assert_theme_has_exact_color(
    themes: [ThemeSnapshot<'_>; 3],
    intent: open_gpui_ui_components::ColorIntent,
) {
    for theme in themes {
        assert!(
            theme
                .colors()
                .iter()
                .any(|entry| entry.token() == intent.token() && entry.state() == intent.state()),
            "missing {} theme color for {} / {}",
            theme.mode().as_str(),
            intent.token(),
            intent.state().as_str()
        );
    }
}

#[test]
fn theme_snapshots_resolve_state_specific_component_tokens() {
    let button = Button::new("secondary", "Secondary")
        .variant(ButtonVariant::Secondary)
        .state();
    let selected_switch = Switch::new("feature").checked(true).state();
    let mixed_checkbox = Checkbox::new("permissions").indeterminate(true).state();
    let disabled_input = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .state();
    let invalid_input = TextInput::new("email", "Email").invalid(true).state();
    let required_field = Field::new("email-field", "email", "Email")
        .required(true)
        .state();
    let theme = ThemeSnapshot::light();

    assert_eq!(
        button.colors().hover_background().state(),
        ColorState::Hover
    );
    assert_eq!(
        selected_switch.colors().track().state(),
        ColorState::Selected
    );
    assert_eq!(
        mixed_checkbox.colors().background().state(),
        ColorState::Selected
    );
    assert_eq!(
        disabled_input.colors().background().state(),
        ColorState::Disabled
    );
    assert_eq!(invalid_input.colors().border().state(), ColorState::Invalid);
    assert_eq!(
        invalid_input.colors().focus_ring().state(),
        ColorState::FocusVisible
    );
    assert_eq!(
        required_field.colors().required_marker().state(),
        ColorState::Required
    );
    assert_eq!(
        Label::new("required-label", "Required")
            .required(true)
            .state()
            .colors()
            .required_marker()
            .state(),
        ColorState::Required
    );

    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            button.colors().hover_background(),
            theme
        )),
        0xdfe6dcff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            disabled_input.colors().background(),
            theme
        )),
        0xf1f5eeff
    );
    assert_eq!(
        u32::from(ThemeResolver::resolve_with(
            invalid_input.colors().focus_ring(),
            theme
        )),
        0x2f80edff
    );
}

#[test]
fn switch_label_uses_theme_text_token() {
    let tokens = custom_tokens();
    let state = Switch::new("feature").tokens(tokens).state();

    assert_eq!(state.colors().label().token(), tokens.text);
}

#[test]
fn checked_switch_maps_to_true_toggled_state() {
    let state = Switch::new("feature").checked(true).state();

    assert!(state.checked());
    assert_eq!(state.role(), Role::Switch);
    assert_eq!(state.toggled(), Toggled::True);
    assert_eq!(state.colors().track().token(), semantic::ACCENT);
    assert_eq!(state.focus_ring().color().token(), semantic::FOCUS_RING);
    assert!(!state.focus_ring().changes_layout());
    assert!(state.activation_enabled());
}

#[test]
fn unchecked_switch_maps_to_false_toggled_state() {
    let state = Switch::new("feature").state();

    assert!(!state.checked());
    assert_eq!(state.toggled(), Toggled::False);
    assert_eq!(state.colors().track().token(), semantic::SURFACE_MUTED);
}

#[test]
fn disabled_switch_keeps_role_but_blocks_activation_metadata() {
    let state = Switch::new("feature").disabled(true).state();

    assert_eq!(state.role(), Role::Switch);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
}

#[test]
fn switch_size_metrics_are_deterministic() {
    let state = Switch::new("feature").small().state();
    let metrics = state.metrics();

    assert_eq!(state.size(), Size::Small);
    assert_eq!(metrics.track_width(), ui_px(32.0));
    assert_eq!(metrics.track_height(), ui_px(18.0));
    assert_eq!(metrics.thumb_size(), ui_px(14.0));
    assert_eq!(metrics.checked_thumb_x(), ui_px(16.0));
}

#[open_gpui::test]
fn switch_runtime_click_emits_on_change_with_next_checked(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        checked: Rc<RefCell<bool>>,
        changes: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let checked = *self.checked.borrow();
            let next_checked = self.checked.clone();
            let changes = self.changes.clone();
            let disabled_changes = self.changes.clone();

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Switch::new("runtime-switch")
                        .label("Runtime switch")
                        .checked(checked)
                        .on_change(move |checked, _, _, _| {
                            *next_checked.borrow_mut() = checked;
                            changes.borrow_mut().push(checked);
                        }),
                )
                .child(
                    Switch::new("disabled-runtime-switch")
                        .label("Disabled runtime switch")
                        .disabled(true)
                        .on_change(move |checked, _, _, _| {
                            disabled_changes.borrow_mut().push(checked);
                        }),
                )
        }
    }

    let checked = Rc::new(RefCell::new(false));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        checked: checked.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let disabled_switch = cx
        .debug_bounds("switch:disabled-runtime-switch:root")
        .expect("disabled switch should expose a stable debug selector");
    cx.simulate_click(disabled_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert!(
        changes.borrow().is_empty(),
        "disabled switch click should not emit on_change"
    );

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should expose a stable debug selector");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), true);
    assert_eq!(changes.borrow().as_slice(), &[true]);

    let runtime_switch = cx
        .debug_bounds("switch:runtime-switch:root")
        .expect("runtime switch should remain rendered after controlled update");
    cx.simulate_click(runtime_switch.center(), Default::default());
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
    assert_eq!(*checked.borrow(), false);
    assert_eq!(changes.borrow().as_slice(), &[true, false]);
}

#[test]
fn checkbox_states_map_to_checkbox_role_and_toggled_values() {
    let unchecked = Checkbox::new("unchecked").state();
    let checked = Checkbox::new("checked").checked(true).state();
    let mixed = Checkbox::new("mixed").indeterminate(true).state();

    assert_eq!(unchecked.role(), Role::CheckBox);
    assert_eq!(unchecked.toggled(), Toggled::False);
    assert!(!unchecked.checked());
    assert!(!unchecked.indeterminate());

    assert_eq!(checked.role(), Role::CheckBox);
    assert_eq!(checked.toggled(), Toggled::True);
    assert!(checked.checked());
    assert!(!checked.indeterminate());

    assert_eq!(mixed.role(), Role::CheckBox);
    assert_eq!(mixed.toggled(), Toggled::Mixed);
    assert!(!mixed.checked());
    assert!(mixed.indeterminate());
}

#[test]
fn disabled_checkbox_blocks_activation_metadata() {
    let state = Checkbox::new("disabled").disabled(true).state();

    assert_eq!(state.role(), Role::CheckBox);
    assert!(state.disabled());
    assert!(!state.activation_enabled());
    assert!(!state.tab_stop_enabled());
    assert_eq!(state.colors().background().state(), ColorState::Disabled);
}

#[test]
fn invalid_and_required_checkbox_expose_state_and_token_intents() {
    let tokens = custom_tokens();
    let state = Checkbox::new("terms")
        .checked(true)
        .required(true)
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.required());
    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().border().state(), ColorState::Invalid);
    assert_eq!(state.colors().background().token(), tokens.accent);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
}

#[test]
fn checkbox_checked_state_builder_accepts_mixed() {
    let state = Checkbox::new("bulk").checked_state(Toggled::Mixed).state();

    assert_eq!(state.toggled(), Toggled::Mixed);
    assert!(state.indeterminate());
    assert!(!state.checked());
}

#[test]
fn label_state_records_control_association_and_required_marker() {
    let tokens = custom_tokens();
    let state = Label::new("email-label", "Email")
        .for_control("email-input")
        .required(true)
        .tokens(tokens)
        .state();

    assert_eq!(state.role(), Role::Label);
    assert_eq!(state.text(), "Email");
    assert_eq!(state.control_id(), Some("email-input"));
    assert!(state.associated());
    assert!(state.required());
    assert_eq!(state.colors().text().token(), tokens.text);
    assert_eq!(state.colors().required_marker().token(), tokens.destructive);
}

#[test]
fn disabled_label_uses_muted_text_intent() {
    let tokens = custom_tokens();
    let state = Label::new("disabled-label", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().text().token(), tokens.text_muted);
    assert_eq!(state.colors().text().state(), ColorState::Disabled);
}

#[test]
fn default_text_input_state_uses_text_input_role_and_placeholder_display() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(state.metrics().height(), Size::Medium.input_h());
    assert_eq!(state.metrics().padding_x(), Size::Medium.input_px());
    assert!(!state.has_value());
    assert_eq!(state.display_text().as_ref(), "Email address");
    assert!(state.displaying_placeholder());
    assert!(state.editable());
}

#[test]
fn filled_text_input_reports_value_state() {
    let state = TextInput::new("email", "Email")
        .placeholder("Email address")
        .value("hello@example.com")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "hello@example.com");
    assert_eq!(state.display_text().as_ref(), "hello@example.com");
    assert!(!state.displaying_placeholder());
}

#[test]
fn text_input_state_normalizes_static_values_with_single_line_policy() {
    let state = TextInput::new("query", "Search")
        .value("alpha\r\nbeta\ngamma")
        .state();

    assert_eq!(state.value(), "alpha  beta gamma");
    assert_eq!(state.display_text().as_ref(), "alpha  beta gamma");
}

#[test]
fn password_text_input_masks_display_without_hiding_value() {
    let state = TextInput::new("password", "Password")
        .placeholder("Password")
        .value("a🙂中")
        .display_mode(TextInputDisplayMode::Password)
        .state();

    assert_eq!(state.value(), "a🙂中");
    assert_eq!(state.display_mode(), TextInputDisplayMode::Password);
    assert_eq!(state.display_text().as_ref(), "•••");
    assert!(state.display_mode().masks_value());
    assert!(!state.displaying_placeholder());
}

#[test]
fn controlled_text_input_on_change_marks_input_controller_driven() {
    let state = TextInput::new("email", "Email")
        .value("hello@example.com")
        .on_change(|_, _, _| {})
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
    assert_eq!(state.value(), "hello@example.com");
}

#[test]
fn disabled_and_read_only_text_inputs_block_editability() {
    let tokens = custom_tokens();
    let disabled = TextInput::new("disabled", "Disabled")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = TextInput::new("readonly", "Read only")
        .read_only(true)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(!disabled.activation_enabled());
    assert_eq!(disabled.colors().background().token(), tokens.surface_muted);
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(!read_only.activation_enabled());
    assert_eq!(
        read_only.colors().background().token(),
        ThemeTokens::default().surface_muted
    );
    assert_eq!(read_only.role(), Role::TextInput);
}

#[test]
fn invalid_text_input_uses_destructive_border_token() {
    let tokens = custom_tokens();
    let state = TextInput::new("email", "Email")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.colors().border().token(), tokens.destructive);
    assert_eq!(state.colors().focus_ring().token(), tokens.focus_ring);
    assert_eq!(state.focus_ring().color().token(), tokens.focus_ring);
    assert!(!state.focus_ring().changes_layout());
    assert_eq!(state.colors().placeholder().token(), tokens.text_muted);
}

#[test]
fn focus_ring_preserves_token_intent_without_layout_shift() {
    let ring = FocusRing::from_color(Button::new("save", "Save").state().colors().focus_ring());
    let shadow = focus_ring_shadow(ring);

    assert_eq!(ring.color().token(), semantic::FOCUS_RING);
    assert_eq!(ring.width(), DEFAULT_FOCUS_RING_WIDTH);
    assert!(!ring.changes_layout());
    assert_eq!(shadow[0].spread_radius, px(2.0));
    assert_eq!(shadow[0].blur_radius, px(0.0));
    assert!(!shadow[0].inset);
}

#[test]
fn text_input_size_helpers_apply_input_metrics() {
    let state = TextInput::new("query", "Search").large().state();

    assert_eq!(state.size(), Size::Large);
    assert_eq!(state.metrics().height(), ui_px(36.0));
    assert_eq!(state.metrics().text_size(), Size::Large.control_text_px());
}

#[open_gpui::test]
fn text_input_controller_converts_utf16_ranges_and_replaces_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a🙂中", cx));

    cx.update_entity(&controller, |controller, cx| {
        let mut adjusted = None;

        assert_eq!(
            controller
                .text_for_range_utf16(1..3, &mut adjusted)
                .as_deref(),
            Some("🙂")
        );
        assert_eq!(adjusted, Some(1..3));

        controller.select_range(1.."a🙂".len(), cx);
        controller.replace_text_in_range_utf16(None, "b\nc", cx);

        assert_eq!(controller.value(), "ab c中");
        assert_eq!(controller.selected_range(), 4..4);
        assert_eq!(controller.selected_range_utf16(), 4..4);
    });
}

#[open_gpui::test]
fn text_input_controller_updates_marked_text_and_commits_composition(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(TextInputController::new);

    cx.update_entity(&controller, |controller, cx| {
        controller.replace_and_mark_text_in_range_utf16(None, "ni", Some(1..2), cx);

        assert_eq!(controller.value(), "ni");
        assert_eq!(controller.marked_range_utf16(), Some(0..2));
        assert_eq!(controller.selected_range_utf16(), 1..2);

        controller.replace_text_in_range_utf16(None, "你", cx);

        assert_eq!(controller.value(), "你");
        assert_eq!(controller.marked_range_utf16(), None);
        assert_eq!(controller.selected_range_utf16(), 1..1);
    });
}

#[open_gpui::test]
fn text_input_controller_delete_commands_respect_grapheme_boundaries(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("a👨‍👩‍👧‍👦b", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.move_to_offset("a👨‍👩‍👧‍👦".len(), cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "ab");

        controller.move_to_offset(1, cx);
        controller.delete_forward(cx);

        assert_eq!(controller.value(), "a");
    });
}

#[open_gpui::test]
fn text_input_controller_rejects_editing_when_disabled_or_read_only(
    cx: &mut open_gpui::TestAppContext,
) {
    let controller = cx.new(|cx| TextInputController::with_value("locked", cx));

    cx.update_entity(&controller, |controller, cx| {
        controller.set_read_only(true, cx);
        controller.select_range(0..controller.value().len(), cx);
        controller.replace_text_in_range_utf16(None, "changed", cx);

        assert_eq!(controller.value(), "locked");

        controller.set_read_only(false, cx);
        controller.set_disabled(true, cx);
        controller.delete_backward(cx);

        assert_eq!(controller.value(), "locked");
        assert!(!controller.accepts_editing());
    });
}

#[open_gpui::test]
fn text_input_runtime_accepts_controller_backed_simulated_input(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        controller: open_gpui::Entity<TextInputController>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                TextInput::new("runtime-text-input", "Runtime text input")
                    .controller(self.controller.clone())
                    .placeholder("Type here"),
            )
        }
    }

    cx.update(init_text_input);
    let controller = cx.new(TextInputController::new);
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        controller: controller.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:runtime-text-input:root")
        .expect("standalone text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    cx.update_entity(&controller, |controller, _| {
        assert_eq!(controller.value(), "hello world");
        assert_eq!(
            controller.selected_range(),
            controller.value().len()..controller.value().len()
        );
    });
}

#[open_gpui::test]
fn controlled_text_input_on_change_accepts_input_without_supplied_controller(
    cx: &mut open_gpui::TestAppContext,
) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                TextInput::new("controlled-text-input", "Controlled text input")
                    .value(value)
                    .placeholder("Type here")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    cx.update(init_text_input);
    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("text-input:controlled-text-input:root")
        .expect("controlled text input should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("hello\nworld");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "hello world");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("hello world")
    );
}

#[open_gpui::test]
fn text_input_state_marks_controller_driven_inputs(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("editable", "Editable")
        .controller(controller)
        .state();

    assert!(state.controller_driven());
    assert!(state.editable());
}

#[open_gpui::test]
fn controller_driven_text_input_state_marks_disabled_editing(cx: &mut open_gpui::TestAppContext) {
    let controller = cx.new(TextInputController::new);
    let state = TextInput::new("disabled", "Disabled")
        .controller(controller)
        .disabled(true)
        .state();

    assert!(state.controller_driven());
    assert!(state.disabled());
    assert!(!state.editable());
}

#[test]
fn default_textarea_state_uses_text_input_role_and_rows() {
    let state = Textarea::new("notes", "Notes")
        .placeholder("Release notes")
        .rows(4)
        .state();

    assert_eq!(state.role(), Role::TextInput);
    assert_eq!(state.rows(), 4);
    assert_eq!(state.metrics().rows(), 4);
    assert!(state.placeholder_visible());
    assert_eq!(state.display_text(), "Release notes");
    assert!(state.editable());
    assert!(!state.controller_driven());
}

#[test]
fn filled_textarea_preserves_newlines_in_state() {
    let state = Textarea::new("notes", "Notes")
        .value("Line 1\r\nLine 2")
        .placeholder("Release notes")
        .state();

    assert!(state.has_value());
    assert_eq!(state.value(), "Line 1\nLine 2");
    assert_eq!(state.display_text(), "Line 1\nLine 2");
    assert!(!state.displaying_placeholder());
}

#[test]
fn disabled_read_only_and_invalid_textareas_expose_control_state() {
    let tokens = custom_tokens();
    let disabled = Textarea::new("disabled-notes", "Disabled notes")
        .disabled(true)
        .tokens(tokens)
        .state();
    let read_only = Textarea::new("readonly-notes", "Read-only notes")
        .read_only(true)
        .state();
    let invalid = Textarea::new("invalid-notes", "Invalid notes")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(disabled.disabled());
    assert!(!disabled.editable());
    assert!(read_only.read_only());
    assert!(!read_only.editable());
    assert!(invalid.invalid());
    assert_eq!(invalid.colors().border().token(), tokens.destructive);
}

#[open_gpui::test]
fn controlled_textarea_on_change_preserves_newline_input(cx: &mut open_gpui::TestAppContext) {
    struct TestView {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();

            div().size_full().child(
                Textarea::new("controlled-textarea", "Controlled textarea")
                    .value(value)
                    .placeholder("Type notes")
                    .on_change(move |value, _, _| {
                        *next_value.borrow_mut() = value.clone();
                        changes.borrow_mut().push(value);
                    }),
            )
        }
    }

    let value = Rc::new(RefCell::new(String::new()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| TestView {
        value: value.clone(),
        changes: changes.clone(),
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let input = cx
        .debug_bounds("textarea:controlled-textarea:root")
        .expect("controlled textarea should expose a stable debug selector");
    cx.simulate_click(input.center(), Default::default());
    cx.simulate_input("Line 1\nLine 2");
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    assert_eq!(value.borrow().as_str(), "Line 1\nLine 2");
    assert_eq!(
        changes.borrow().last().map(String::as_str),
        Some("Line 1\nLine 2")
    );
}

#[test]
fn default_field_state_exposes_label_help_and_metrics() {
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .state();

    assert_eq!(state.label(), "Email");
    assert_eq!(state.help().unwrap(), "Use a work address.");
    assert_eq!(state.support_text().unwrap(), "Use a work address.");
    assert!(!state.support_is_error());
    assert_eq!(state.size(), Size::Medium);
    assert_eq!(
        state.metrics().label_text_size(),
        Size::Medium.control_text_px()
    );
}

#[test]
fn required_field_exposes_required_metadata() {
    let state = Field::new("email-field", "email", "Email")
        .required(true)
        .state();

    assert!(state.required());
    assert_eq!(
        state.colors().required_marker().token(),
        semantic::DESTRUCTIVE
    );
}

#[test]
fn invalid_field_prefers_error_support_text() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .help("Use a work address.")
        .error("Enter a valid email.")
        .invalid(true)
        .tokens(tokens)
        .state();

    assert!(state.invalid());
    assert_eq!(state.support_text().unwrap(), "Enter a valid email.");
    assert!(state.support_is_error());
    assert_eq!(state.colors().message().token(), tokens.destructive);
}

#[test]
fn disabled_field_uses_muted_label_intent() {
    let tokens = custom_tokens();
    let state = Field::new("email-field", "email", "Email")
        .disabled(true)
        .tokens(tokens)
        .state();

    assert!(state.disabled());
    assert_eq!(state.colors().message().token(), tokens.text_muted);
}
