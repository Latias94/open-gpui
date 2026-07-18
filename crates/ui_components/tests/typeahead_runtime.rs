use std::{cell::RefCell, rc::Rc, time::Duration};

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    Context, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    ParentElement, Render, Styled, VisualTestContext, Window, div, point, px, size,
};
use open_gpui_ui_components::{
    ContextMenu, Listbox, ListboxGroup, Menu, MenuItem, Select, Tree, TreeItemDescriptor,
    VirtualizedList, VirtualizedListItemDescriptor, VirtualizedListSelectionMode,
    listbox::ListboxOption,
};
use open_gpui_ui_core::{Sizable, ui_px};

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });
}

fn dispatch_key(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).expect("test keystroke should parse");
    cx.update(|window, cx| {
        window.dispatch_keystroke(keystroke, cx);
    });
}

fn key_down_event(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    }
}

fn committed_character_event(physical_key: &str, character: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: physical_key.to_owned(),
            key_char: Some(character.to_owned()),
        },
        is_held: false,
        prefer_character_input: true,
    }
}

fn key_up_event(key: &str) -> KeyUpEvent {
    KeyUpEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: key.to_owned(),
            key_char: None,
        },
    }
}

fn nested_listbox_option_selector(
    cx: &mut VisualTestContext,
    owner_id: &str,
    value: &str,
) -> String {
    let suffix = format!(":option:{value}");
    let matches = cx
        .debug_selectors_with_prefix("listbox:")
        .into_iter()
        .filter(|selector| selector.contains(owner_id) && selector.ends_with(&suffix))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "nested Listbox option selector should be unique for {owner_id:?}/{value:?}: {matches:?}"
    );
    matches.into_iter().next().unwrap()
}

fn focus_tree(cx: &mut VisualTestContext, id: &str) {
    let root = cx
        .debug_bounds(&format!("tree:{id}:root"))
        .expect("tree root should render");
    cx.simulate_click(
        point(root.left() + px(2.0), root.top() + px(2.0)),
        Modifiers::none(),
    );
    draw(cx);
}

fn focus_virtualized_list(cx: &mut VisualTestContext, id: &str) {
    let root = cx
        .debug_bounds(&format!("virtualized-list:{id}:root"))
        .expect("virtualized list root should render");
    cx.simulate_click(root.center(), Modifiers::none());
    draw(cx);
}

struct StaticTreeView {
    id: &'static str,
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for StaticTreeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().child(
            div().w(px(280.0)).h(px(180.0)).child(
                Tree::new(
                    self.id,
                    "Deterministic typeahead tree",
                    [
                        TreeItemDescriptor::new("alpha", "Alpha"),
                        TreeItemDescriptor::new("alpine", "Alpine"),
                        TreeItemDescriptor::new("amber", "Amber"),
                        TreeItemDescriptor::new("north", "North"),
                        TreeItemDescriptor::new("notes", "Notes"),
                        TreeItemDescriptor::new("ocean", "Ocean"),
                    ],
                )
                .small()
                .default_focused("alpha")
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                }),
            ),
        )
    }
}

#[open_gpui::test]
fn tree_repeated_character_cycles_without_waiting_for_a_redraw(cx: &mut open_gpui::TestAppContext) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| StaticTreeView {
        id: "same-frame-cycle-tree",
        selections: selections.clone(),
    });
    draw(cx);
    focus_tree(cx, "same-frame-cycle-tree");

    dispatch_key(cx, "a");
    assert!(cx.debug_selector_is_focused("tree:same-frame-cycle-tree:item:alpine"));
    dispatch_key(cx, "a");
    assert!(cx.debug_selector_is_focused("tree:same-frame-cycle-tree:item:amber"));
    dispatch_key(cx, "a");
    assert!(cx.debug_selector_is_focused("tree:same-frame-cycle-tree:item:alpha"));
    dispatch_key(cx, "a");
    assert!(cx.debug_selector_is_focused("tree:same-frame-cycle-tree:item:alpine"));
    assert!(
        selections.borrow().is_empty(),
        "typeahead focus movement must not select a Tree item"
    );
}

#[open_gpui::test]
fn tree_typeahead_navigation_and_activation_use_the_event_time_value(
    cx: &mut open_gpui::TestAppContext,
) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| StaticTreeView {
        id: "event-time-tree",
        selections: selections.clone(),
    });
    draw(cx);
    focus_tree(cx, "event-time-tree");

    dispatch_key(cx, "a");
    dispatch_key(cx, "down");
    dispatch_key(cx, "enter");

    assert_eq!(
        selections.borrow().as_slice(),
        ["amber"],
        "Down and Enter must resolve from the runtime value written by typeahead without a redraw"
    );
}

#[open_gpui::test]
fn tree_fake_clock_keeps_exact_boundary_and_resets_after_timeout(
    cx: &mut open_gpui::TestAppContext,
) {
    let (_, cx) = cx.add_window_view(|_, _| StaticTreeView {
        id: "fake-clock-tree",
        selections: Rc::new(RefCell::new(Vec::new())),
    });
    draw(cx);
    focus_tree(cx, "fake-clock-tree");

    dispatch_key(cx, "n");
    assert!(cx.debug_selector_is_focused("tree:fake-clock-tree:item:north"));

    cx.executor().advance_clock(Duration::from_millis(700));
    dispatch_key(cx, "o");
    assert!(
        cx.debug_selector_is_focused("tree:fake-clock-tree:item:north"),
        "the exact timeout boundary should refine `n` to `no` and include the current match"
    );

    cx.executor().advance_clock(Duration::from_millis(701));
    dispatch_key(cx, "o");
    assert!(
        cx.debug_selector_is_focused("tree:fake-clock-tree:item:ocean"),
        "input after the timeout should start a new one-character cycle"
    );
}

#[open_gpui::test]
fn rejected_collection_input_propagates_without_mutating_the_session(
    cx: &mut open_gpui::TestAppContext,
) {
    let (_, cx) = cx.add_window_view(|_, _| StaticTreeView {
        id: "filtered-input-tree",
        selections: Rc::new(RefCell::new(Vec::new())),
    });
    draw(cx);
    focus_tree(cx, "filtered-input-tree");

    let incomplete_ime = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: "a".to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: true,
    });
    assert!(incomplete_ime.propagated());
    assert!(!incomplete_ime.default_prevented());

    let control_modified = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Modifiers::none()
            },
            key: "a".to_owned(),
            key_char: Some("a".to_owned()),
        },
        is_held: false,
        prefer_character_input: false,
    });
    assert!(control_modified.propagated());
    assert!(!control_modified.default_prevented());
    assert!(cx.debug_selector_is_focused("tree:filtered-input-tree:item:alpha"));

    let committed = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: "a".to_owned(),
            key_char: Some("a".to_owned()),
        },
        is_held: false,
        prefer_character_input: false,
    });
    assert!(!committed.propagated());
    assert!(committed.default_prevented());
    assert!(
        cx.debug_selector_is_focused("tree:filtered-input-tree:item:alpine"),
        "rejected input must not prepend characters or refresh the deadline"
    );
}

struct DynamicTreeView {
    phase: usize,
    mounted: bool,
    selections: Rc<RefCell<Vec<String>>>,
}

impl DynamicTreeView {
    fn items(&self) -> Vec<TreeItemDescriptor> {
        match self.phase {
            0 => vec![
                TreeItemDescriptor::new("alpha", "Alpha"),
                TreeItemDescriptor::new("alpine", "Alpine"),
                TreeItemDescriptor::new("amber", "Amber"),
            ],
            1 => vec![
                TreeItemDescriptor::new("alpine", "Alpine"),
                TreeItemDescriptor::new("alpha", "Alpha"),
                TreeItemDescriptor::new("amber", "Amber"),
            ],
            2 => vec![
                TreeItemDescriptor::new("alpine", "Alpine"),
                TreeItemDescriptor::new("amber", "Amber"),
            ],
            _ => vec![
                TreeItemDescriptor::new("alpha", "Alpha first"),
                TreeItemDescriptor::new("alpha", "Alpha second"),
                TreeItemDescriptor::new("amber", "Amber"),
            ],
        }
    }
}

impl Render for DynamicTreeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().when(self.mounted, |root| {
            root.child(
                div().w(px(280.0)).h(px(180.0)).child(
                    Tree::new("dynamic-tree", "Dynamic tree", self.items())
                        .small()
                        .default_focused("alpha")
                        .on_select(move |selection, _, _| {
                            selections.borrow_mut().push(selection.value().to_owned());
                        }),
                ),
            )
        })
    }
}

#[open_gpui::test]
fn tree_reorder_remove_and_remount_use_stable_instance_local_state(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, _| DynamicTreeView {
        phase: 0,
        mounted: true,
        selections: Rc::new(RefCell::new(Vec::new())),
    });
    draw(cx);
    focus_tree(cx, "dynamic-tree");

    dispatch_key(cx, "a");
    assert!(cx.debug_selector_is_focused("tree:dynamic-tree:item:alpine"));

    view.update(cx, |view, cx| {
        view.phase = 1;
        cx.notify();
    });
    draw(cx);
    assert!(
        cx.debug_selector_is_focused("tree:dynamic-tree:item:alpine"),
        "reorder must preserve the focused stable value"
    );
    dispatch_key(cx, "a");
    assert!(
        cx.debug_selector_is_focused("tree:dynamic-tree:item:alpha"),
        "cycling after reorder must scan from the stable value's new position"
    );

    view.update(cx, |view, cx| {
        view.phase = 2;
        cx.notify();
    });
    draw(cx);
    focus_tree(cx, "dynamic-tree");
    assert!(cx.debug_selector_is_focused("tree:dynamic-tree:item:alpine"));
    dispatch_key(cx, "a");
    assert!(
        cx.debug_selector_is_focused("tree:dynamic-tree:item:amber"),
        "removing the active key must fall back safely without a stale index"
    );

    view.update(cx, |view, cx| {
        view.mounted = false;
        view.phase = 0;
        cx.notify();
    });
    draw(cx);
    view.update(cx, |view, cx| {
        view.mounted = true;
        cx.notify();
    });
    draw(cx);
    focus_tree(cx, "dynamic-tree");
    dispatch_key(cx, "m");
    assert!(
        cx.debug_selector_is_focused("tree:dynamic-tree:item:alpha"),
        "a remounted instance must not retain the previous instance's `a` buffer"
    );
}

#[open_gpui::test]
fn tree_removes_focus_authority_when_a_unique_value_becomes_ambiguous(
    cx: &mut open_gpui::TestAppContext,
) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| DynamicTreeView {
        phase: 0,
        mounted: true,
        selections: selections.clone(),
    });
    draw(cx);
    focus_tree(cx, "dynamic-tree");
    assert!(cx.debug_selector_is_focused("tree:dynamic-tree:item:alpha"));

    view.update(cx, |view, cx| {
        view.phase = 3;
        cx.notify();
    });
    draw(cx);

    assert!(
        !cx.debug_selector_is_focused("tree:dynamic-tree:item:alpha"),
        "ambiguous disabled rows must not retain the former unique value's focus handle"
    );
    focus_tree(cx, "dynamic-tree");
    assert!(cx.debug_selector_is_focused("tree:dynamic-tree:item:amber"));
    dispatch_key(cx, "enter");
    assert_eq!(selections.borrow().as_slice(), ["amber"]);
}

struct TwoTreeView;

impl Render for TwoTreeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tree = |id| {
            Tree::new(
                id,
                id,
                [
                    TreeItemDescriptor::new("alpha", "Alpha"),
                    TreeItemDescriptor::new("north", "North"),
                    TreeItemDescriptor::new("ocean", "Ocean"),
                ],
            )
            .small()
            .default_focused("alpha")
        };

        div()
            .size_full()
            .flex()
            .gap_2()
            .child(div().w(px(260.0)).h(px(160.0)).child(tree("first-tree")))
            .child(div().w(px(260.0)).h(px(160.0)).child(tree("second-tree")))
    }
}

#[open_gpui::test]
fn tree_instances_in_one_window_never_share_a_buffer(cx: &mut open_gpui::TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| TwoTreeView);
    draw(cx);

    focus_tree(cx, "first-tree");
    dispatch_key(cx, "n");
    assert!(cx.debug_selector_is_focused("tree:first-tree:item:north"));

    focus_tree(cx, "second-tree");
    dispatch_key(cx, "o");
    assert!(
        cx.debug_selector_is_focused("tree:second-tree:item:ocean"),
        "the second instance must start with `o`, not inherit the first instance's `n`"
    );
}

#[open_gpui::test]
fn equal_tree_ids_in_different_windows_never_share_a_buffer(cx: &mut open_gpui::TestAppContext) {
    let first = cx
        .open_window(size(px(320.0), px(220.0)), |_, _| StaticTreeView {
            id: "shared-window-tree",
            selections: Rc::new(RefCell::new(Vec::new())),
        })
        .into();
    let second = cx
        .open_window(size(px(320.0), px(220.0)), |_, _| StaticTreeView {
            id: "shared-window-tree",
            selections: Rc::new(RefCell::new(Vec::new())),
        })
        .into();

    {
        let mut first_cx = VisualTestContext::from_window(first, cx);
        draw(&mut first_cx);
        focus_tree(&mut first_cx, "shared-window-tree");
        dispatch_key(&mut first_cx, "n");
        assert!(first_cx.debug_selector_is_focused("tree:shared-window-tree:item:north"));
    }
    {
        let mut second_cx = VisualTestContext::from_window(second, cx);
        draw(&mut second_cx);
        focus_tree(&mut second_cx, "shared-window-tree");
        dispatch_key(&mut second_cx, "o");
        assert!(
            second_cx.debug_selector_is_focused("tree:shared-window-tree:item:ocean"),
            "window-local runtime state must not inherit another window's `n` buffer"
        );
    }

    assert!(cx.debug_selector_is_focused_in_window(first, "tree:shared-window-tree:item:north"));
    assert!(cx.debug_selector_is_focused_in_window(second, "tree:shared-window-tree:item:ocean"));
}

struct VirtualizedListProbe {
    phase: usize,
    activations: Rc<RefCell<Vec<String>>>,
    selection_changes: Rc<RefCell<Vec<Vec<String>>>>,
}

impl VirtualizedListProbe {
    fn items(&self) -> Vec<VirtualizedListItemDescriptor> {
        match self.phase {
            0 => vec![
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::section("section", "Albatross section"),
                VirtualizedListItemDescriptor::new("disabled", "Almond disabled").disabled(true),
                VirtualizedListItemDescriptor::separator("separator"),
                VirtualizedListItemDescriptor::loading("loading", "Amber loading"),
                VirtualizedListItemDescriptor::new("alpine", "Alpine"),
                VirtualizedListItemDescriptor::new("amber", "Amber"),
            ],
            1 => vec![
                VirtualizedListItemDescriptor::new("alpine", "Alpine"),
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::new("amber", "Amber"),
            ],
            _ => vec![
                VirtualizedListItemDescriptor::new("alpine", "Alpine"),
                VirtualizedListItemDescriptor::new("amber", "Amber"),
            ],
        }
    }
}

impl Render for VirtualizedListProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let activations = self.activations.clone();
        let selection_changes = self.selection_changes.clone();
        div().size_full().child(
            div().w(px(280.0)).h(px(112.0)).child(
                VirtualizedList::new("typeahead-list", "Typeahead list", self.items())
                    .small()
                    .row_height(ui_px(28.0))
                    .viewport_item_count(4)
                    .overscan(0)
                    .default_active_key("alpha")
                    .selection_mode(VirtualizedListSelectionMode::Multiple)
                    .on_activate(move |activation, _, _| {
                        activations.borrow_mut().push(activation.key().to_owned());
                    })
                    .on_selection_change(move |change, _, _| {
                        selection_changes.borrow_mut().push(
                            change
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

#[open_gpui::test]
fn virtualized_list_typeahead_skips_structural_rows_and_never_selects(
    cx: &mut open_gpui::TestAppContext,
) {
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| VirtualizedListProbe {
        phase: 0,
        activations: activations.clone(),
        selection_changes: selection_changes.clone(),
    });
    draw(cx);
    focus_virtualized_list(cx, "typeahead-list");

    dispatch_key(cx, "a");
    draw(cx);
    assert!(
        cx.debug_bounds("virtualized-list:typeahead-list:row:alpine")
            .is_some(),
        "section, disabled, separator, and loading rows must be skipped before Alpine"
    );
    assert!(selection_changes.borrow().is_empty());

    dispatch_key(cx, "enter");
    assert_eq!(activations.borrow().as_slice(), ["alpine"]);
    assert!(
        selection_changes.borrow().is_empty(),
        "typeahead and multiple-mode activation must not imply selection"
    );
}

#[open_gpui::test]
fn virtualized_list_reorder_and_remove_cycle_from_the_latest_stable_key(
    cx: &mut open_gpui::TestAppContext,
) {
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| VirtualizedListProbe {
        phase: 0,
        activations: activations.clone(),
        selection_changes: selection_changes.clone(),
    });
    draw(cx);
    focus_virtualized_list(cx, "typeahead-list");

    dispatch_key(cx, "a");
    view.update(cx, |view, cx| {
        view.phase = 1;
        cx.notify();
    });
    draw(cx);
    dispatch_key(cx, "a");
    draw(cx);
    dispatch_key(cx, "enter");
    assert_eq!(
        activations.borrow().as_slice(),
        ["alpha"],
        "reorder must scan from Alpine's new stable-key position"
    );

    activations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.phase = 2;
        cx.notify();
    });
    draw(cx);
    dispatch_key(cx, "a");
    draw(cx);
    dispatch_key(cx, "enter");
    assert_eq!(
        activations.borrow().as_slice(),
        ["amber"],
        "removing Alpha must resolve the fallback key without retaining its old index"
    );
    assert!(selection_changes.borrow().is_empty());
}

#[open_gpui::test]
fn virtualized_list_navigation_and_activation_use_the_event_time_key(
    cx: &mut open_gpui::TestAppContext,
) {
    let activations = Rc::new(RefCell::new(Vec::new()));
    let selection_changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| VirtualizedListProbe {
        phase: 0,
        activations: activations.clone(),
        selection_changes,
    });
    draw(cx);
    focus_virtualized_list(cx, "typeahead-list");

    dispatch_key(cx, "a");
    dispatch_key(cx, "down");
    dispatch_key(cx, "enter");

    assert_eq!(
        activations.borrow().as_slice(),
        ["amber"],
        "Down and Enter must resolve from the runtime key written by typeahead without a redraw"
    );
}

struct ListboxTypeaheadView {
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for ListboxTypeaheadView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().child(
            Listbox::new("typeahead-listbox", "Typeahead listbox")
                .default_selected("alpha")
                .default_active("alpha")
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::separator("separator"))
                .option(ListboxOption::new("aardvark", "Aardvark").disabled(true))
                .group(
                    ListboxGroup::new("group", "Group")
                        .option(ListboxOption::new("alpine", "Alpine"))
                        .option(ListboxOption::new("amber", "Amber")),
                )
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                }),
        )
    }
}

#[open_gpui::test]
fn standalone_listbox_cycles_without_selecting_and_skips_disabled_structure(
    cx: &mut open_gpui::TestAppContext,
) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| ListboxTypeaheadView {
        selections: selections.clone(),
    });
    draw(cx);

    let alpha = cx
        .debug_bounds("listbox:typeahead-listbox:option:alpha")
        .expect("Alpha option should render");
    cx.simulate_click(alpha.center(), Modifiers::none());
    draw(cx);
    selections.borrow_mut().clear();

    let preferred_space =
        cx.simulate_event_with_dispatch_snapshot(committed_character_event("space", " "));
    cx.simulate_event(key_up_event("space"));
    assert!(preferred_space.propagated());
    assert!(!preferred_space.default_prevented());
    assert!(
        selections.borrow().is_empty(),
        "character-preferred whitespace must not arm semantic Space activation"
    );

    let preferred_a =
        cx.simulate_event_with_dispatch_snapshot(committed_character_event("down", "a"));
    assert!(!preferred_a.propagated());
    assert!(preferred_a.default_prevented());
    dispatch_key(cx, "a");
    assert!(
        selections.borrow().is_empty(),
        "typeahead must only move the active option"
    );

    cx.simulate_event(key_down_event("enter"));
    cx.simulate_event(key_up_event("enter"));
    assert_eq!(
        selections.borrow().as_slice(),
        ["amber"],
        "separator and disabled Aardvark must be skipped while cycling Alpha -> Alpine -> Amber"
    );
}

struct MenuTypeaheadView {
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for MenuTypeaheadView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().child(
            Menu::new("typeahead-menu", "Typeahead menu")
                .default_focused_value("alpha")
                .item(MenuItem::action("alpha", "Alpha"))
                .item(MenuItem::separator("separator"))
                .item(MenuItem::action("aardvark", "Aardvark").disabled(true))
                .item(MenuItem::action("alpine", "Alpine"))
                .item(MenuItem::action("amber", "Amber"))
                .item(MenuItem::action("north", "North"))
                .item(MenuItem::action("ocean", "Ocean"))
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                }),
        )
    }
}

#[open_gpui::test]
fn menu_typeahead_cycles_and_reopen_starts_a_new_session(cx: &mut open_gpui::TestAppContext) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| MenuTypeaheadView {
        selections: selections.clone(),
    });
    draw(cx);

    let trigger = cx
        .debug_bounds("menu:typeahead-menu:trigger")
        .expect("menu trigger should render");
    cx.simulate_click(trigger.center(), Modifiers::none());
    draw(cx);

    dispatch_key(cx, "a");
    dispatch_key(cx, "a");
    assert!(selections.borrow().is_empty());

    dispatch_key(cx, "enter");
    draw(cx);
    assert_eq!(selections.borrow().as_slice(), ["amber"]);
    assert!(cx.debug_bounds("menu:typeahead-menu:content").is_none());

    let trigger = cx
        .debug_bounds("menu:typeahead-menu:trigger")
        .expect("menu trigger should remain reusable");
    cx.simulate_click(trigger.center(), Modifiers::none());
    draw(cx);
    let preferred_o =
        cx.simulate_event_with_dispatch_snapshot(committed_character_event("down", "o"));
    assert!(!preferred_o.propagated());
    assert!(preferred_o.default_prevented());
    dispatch_key(cx, "enter");
    draw(cx);
    assert_eq!(
        selections.borrow().as_slice(),
        ["amber", "ocean"],
        "reopen must clear the previous `a` session before accepting `o`"
    );
}

struct ContextMenuTypeaheadView {
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for ContextMenuTypeaheadView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().child(
            ContextMenu::new("typeahead-context-menu", "Typeahead context menu")
                .default_open(true)
                .default_focused_value("alpha")
                .item(MenuItem::action("alpha", "Alpha"))
                .item(MenuItem::action("alpine", "Alpine"))
                .item(MenuItem::action("amber", "Amber"))
                .item(MenuItem::action("ocean", "Ocean"))
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                }),
        )
    }
}

#[open_gpui::test]
fn context_menu_typeahead_resets_when_reopened_at_a_new_generation(
    cx: &mut open_gpui::TestAppContext,
) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| ContextMenuTypeaheadView {
        selections: selections.clone(),
    });
    draw(cx);
    assert!(
        cx.debug_bounds("context-menu:typeahead-context-menu:surface")
            .is_some()
    );

    dispatch_key(cx, "a");
    dispatch_key(cx, "a");
    dispatch_key(cx, "enter");
    draw(cx);
    assert_eq!(selections.borrow().as_slice(), ["amber"]);
    assert!(
        cx.debug_bounds("context-menu:typeahead-context-menu:surface")
            .is_none()
    );

    let hotspot = cx
        .debug_bounds("context-menu:typeahead-context-menu:hotspot")
        .expect("context menu hotspot should remain mounted");
    cx.simulate_mouse_down(hotspot.center(), MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(hotspot.center(), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    draw(cx);
    let preferred_o =
        cx.simulate_event_with_dispatch_snapshot(committed_character_event("enter", "o"));
    assert!(!preferred_o.propagated());
    assert!(preferred_o.default_prevented());
    dispatch_key(cx, "enter");
    draw(cx);
    assert_eq!(
        selections.borrow().as_slice(),
        ["amber", "ocean"],
        "a new opening generation must not retain the previous `a` buffer"
    );
}

struct SelectTypeaheadView {
    selections: Rc<RefCell<Vec<String>>>,
}

impl Render for SelectTypeaheadView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        div().size_full().child(
            Select::new("typeahead-select", "Typeahead select")
                .default_active("alpha")
                .option(ListboxOption::new("alpha", "Alpha"))
                .option(ListboxOption::new("alpine", "Alpine"))
                .option(ListboxOption::new("amber", "Amber"))
                .option(ListboxOption::new("north", "North"))
                .option(ListboxOption::new("ocean", "Ocean"))
                .on_select(move |selection, _, _| {
                    selections.borrow_mut().push(selection.value().to_owned());
                }),
        )
    }
}

#[open_gpui::test]
fn select_typeahead_exists_only_in_the_open_popup_and_resets_on_reopen(
    cx: &mut open_gpui::TestAppContext,
) {
    let selections = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| SelectTypeaheadView {
        selections: selections.clone(),
    });
    draw(cx);

    let trigger = cx
        .debug_bounds("select:typeahead-select:trigger")
        .expect("Select trigger should render");
    cx.simulate_click(trigger.center(), Modifiers::none());
    draw(cx);
    dispatch_key(cx, "a");
    draw(cx);
    let alpine = nested_listbox_option_selector(cx, "typeahead-select", "alpine");
    assert!(cx.debug_selector_is_focused(&alpine));
    assert!(selections.borrow().is_empty());

    dispatch_key(cx, "escape");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_selector_is_focused("select:typeahead-select:trigger"));

    dispatch_key(cx, "n");
    draw(cx);
    assert!(
        cx.debug_selectors_with_prefix("listbox:")
            .into_iter()
            .all(|selector| !selector.contains("typeahead-select")),
        "printable input on the closed trigger must not open or select"
    );
    assert!(selections.borrow().is_empty());

    let preferred_down =
        cx.simulate_event_with_dispatch_snapshot(committed_character_event("down", "@"));
    assert!(preferred_down.propagated());
    assert!(!preferred_down.default_prevented());
    let modified_down = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Modifiers::none()
            },
            key: "down".to_owned(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });
    assert!(modified_down.propagated());
    assert!(!modified_down.default_prevented());
    assert!(
        cx.debug_selectors_with_prefix("listbox:")
            .into_iter()
            .all(|selector| !selector.contains("typeahead-select")),
        "character-preferred and modified command keys must keep a closed Select inert"
    );

    let trigger = cx
        .debug_bounds("select:typeahead-select:trigger")
        .expect("Select trigger should remain reusable");
    cx.simulate_click(trigger.center(), Modifiers::none());
    draw(cx);
    dispatch_key(cx, "o");
    draw(cx);
    let ocean = nested_listbox_option_selector(cx, "typeahead-select", "ocean");
    assert!(
        cx.debug_selector_is_focused(&ocean),
        "reopened popup must begin with a fresh `o` session"
    );
    assert!(selections.borrow().is_empty());
}
