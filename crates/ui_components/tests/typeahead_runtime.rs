use std::{cell::RefCell, rc::Rc, time::Duration};

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyView, AppContext, Context, Entity, FocusHandle, InputEvent, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement, Styled,
    SubtreePresentation, SubtreePresentationExt, VisualContext, VisualTestContext, Window, canvas,
    div, point, px, size,
};
use open_gpui_ui_components::{
    ContextMenu, Listbox, ListboxGroup, Menu, MenuItem, ScrollArea, Select, Tree,
    TreeItemDescriptor, VirtualizedList, VirtualizedListItemDescriptor,
    VirtualizedListSelectionMode, listbox::ListboxOption,
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
        point(root.left() + px(8.0), root.top() + px(2.0)),
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

#[derive(Clone, Copy, Default)]
enum VirtualizedTreeTargetMutation {
    #[default]
    None,
    Reordered,
    Disabled,
}

struct VirtualizedTreeFocusView {
    target_mutation: VirtualizedTreeTargetMutation,
}

impl VirtualizedTreeFocusView {
    fn items(&self) -> Vec<TreeItemDescriptor> {
        let mut indices = (0..100).collect::<Vec<_>>();
        if matches!(
            self.target_mutation,
            VirtualizedTreeTargetMutation::Reordered
        ) {
            let target = indices.pop().expect("test tree should contain a target");
            indices.insert(10, target);
        }

        indices
            .into_iter()
            .map(|index| {
                let descriptor =
                    TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"));
                if index == 99
                    && matches!(
                        self.target_mutation,
                        VirtualizedTreeTargetMutation::Disabled
                    )
                {
                    descriptor.disabled(true)
                } else {
                    descriptor
                }
            })
            .collect()
    }
}

impl Render for VirtualizedTreeFocusView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div().w(px(280.0)).h(px(112.0)).child(
                Tree::new("virtual-focus-tree", "Virtual focus tree", self.items())
                    .small()
                    .virtualized(true)
                    .viewport_item_count(4)
                    .overscan_count(0)
                    .default_focused("item-0000"),
            ),
        )
    }
}

struct NestedVirtualizedTreeWheelView {
    outer_scroll_handle: ScrollHandle,
    virtualized: bool,
}

impl Render for NestedVirtualizedTreeWheelView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let items = (0..100).map(|index| {
            TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"))
        });

        div().size_full().child(
            div().w(px(240.0)).h(px(180.0)).child(
                ScrollArea::new(
                    "tree-wheel-fence-outer",
                    div().relative().w(px(640.0)).h(px(640.0)).child(
                        div().absolute().w(px(180.0)).h(px(112.0)).child(
                            Tree::new("tree-wheel-fence", "Tree wheel fence", items)
                                .small()
                                .virtualized(self.virtualized)
                                .viewport_item_count(4)
                                .overscan_count(0)
                                .default_focused("item-0000"),
                        ),
                    ),
                )
                .vertical()
                .scroll_handle(&self.outer_scroll_handle),
            ),
        )
    }
}

struct LateTreeFocusClaimView {
    focus: FocusHandle,
    armed: bool,
}

impl Render for LateTreeFocusClaimView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.armed {
            self.armed = false;
            self.focus.focus(window, cx);
        }

        div()
            .id("late-tree-focus-claim")
            .debug_selector(|| "late-tree-focus-claim".into())
            .w(px(80.0))
            .h(px(32.0))
            .focusable()
            .track_focus(&self.focus)
    }
}

struct VirtualizedTreeLateFocusView {
    late_focus_claim: Entity<LateTreeFocusClaimView>,
}

impl Render for VirtualizedTreeLateFocusView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let items = (0..100).map(|index| {
            TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"))
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().w(px(280.0)).h(px(112.0)).child(
                    Tree::new("tree-late-focus-claim", "Tree late focus claim", items)
                        .small()
                        .virtualized(true)
                        .viewport_item_count(4)
                        .overscan_count(0)
                        .default_focused("item-0000"),
                ),
            )
            .child(AnyView::from(self.late_focus_claim.clone()))
    }
}

struct LateTreePrepaintFocusClaimView {
    focus: FocusHandle,
    armed: bool,
    focus_stable: bool,
}

impl Render for LateTreePrepaintFocusClaimView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let request_focus = std::mem::take(&mut self.armed);
        let focus = self.focus.clone();
        let focus_stable = self.focus_stable;

        div()
            .id("late-tree-prepaint-focus-claim")
            .debug_selector(|| "late-tree-prepaint-focus-claim".into())
            .w(px(80.0))
            .h(px(32.0))
            .focusable()
            .track_focus(&self.focus)
            .child(
                canvas(
                    move |_, window, _| {
                        if request_focus {
                            let focus = focus.clone();
                            if focus_stable {
                                window.record_prepaint_focus_stable_commit(move |_, window, cx| {
                                    focus.focus(window, cx);
                                });
                            } else {
                                window.record_prepaint_window_commit(move |_, window, cx| {
                                    focus.focus(window, cx);
                                });
                            }
                        }
                    },
                    |_, _, _, _| {},
                )
                .size_full(),
            )
    }
}

struct VirtualizedTreeLatePrepaintFocusView {
    late_focus_claim: Entity<LateTreePrepaintFocusClaimView>,
}

impl Render for VirtualizedTreeLatePrepaintFocusView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let items = (0..100).map(|index| {
            TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"))
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().w(px(280.0)).h(px(112.0)).child(
                    Tree::new(
                        "tree-late-prepaint-focus-claim",
                        "Tree late prepaint focus claim",
                        items,
                    )
                    .small()
                    .virtualized(true)
                    .viewport_item_count(4)
                    .overscan_count(0)
                    .default_focused("item-0000"),
                ),
            )
            .child(AnyView::from(self.late_focus_claim.clone()))
    }
}

struct VirtualizedTreeRejectedFocusRetryView {
    presentation: SubtreePresentation,
    virtualized: bool,
    external_focus: FocusHandle,
}

impl Render for VirtualizedTreeRejectedFocusRetryView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let items = (0..100).map(|index| {
            TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"))
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .w(px(280.0))
                    .h(px(112.0))
                    .child(
                        Tree::new(
                            "tree-rejected-focus-retry",
                            "Tree rejected focus retry",
                            items,
                        )
                        .small()
                        .virtualized(self.virtualized)
                        .viewport_item_count(4)
                        .overscan_count(0)
                        .default_focused("item-0000"),
                    )
                    .with_subtree_presentation(self.presentation),
            )
            .child(
                div()
                    .id("tree-rejected-focus-retry-external")
                    .debug_selector(|| "tree-rejected-focus-retry-external".into())
                    .w(px(80.0))
                    .h(px(32.0))
                    .focusable()
                    .track_focus(&self.external_focus),
            )
    }
}

#[open_gpui::test]
fn virtualized_tree_re_resolves_a_keyboard_focus_target_after_reorder(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, _| VirtualizedTreeFocusView {
        target_mutation: VirtualizedTreeTargetMutation::None,
    });
    draw(cx);
    focus_tree(cx, "virtual-focus-tree");

    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        view.update(cx, |view, cx| {
            view.target_mutation = VirtualizedTreeTargetMutation::Reordered;
            cx.notify();
        });
    });
    draw(cx);

    assert!(
        cx.debug_selector_is_focused("tree:virtual-focus-tree:item:item-0099"),
        "the logical target must be focused at its current index"
    );
}

#[open_gpui::test]
fn virtualized_tree_cancels_a_keyboard_focus_target_that_becomes_disabled(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, _| VirtualizedTreeFocusView {
        target_mutation: VirtualizedTreeTargetMutation::None,
    });
    draw(cx);
    focus_tree(cx, "virtual-focus-tree");

    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        view.update(cx, |view, cx| {
            view.target_mutation = VirtualizedTreeTargetMutation::Disabled;
            cx.notify();
        });
    });
    draw(cx);

    assert!(
        cx.debug_selector_is_focused("tree:virtual-focus-tree:item:item-0000"),
        "an unavailable logical target must not move or strand focus"
    );
    assert!(
        cx.debug_bounds("tree:virtual-focus-tree:item:item-0099")
            .is_none(),
        "an unavailable logical target must not materialize its stale index"
    );
}

#[open_gpui::test]
fn virtualized_tree_wheel_before_the_next_draw_cancels_materialization(
    cx: &mut open_gpui::TestAppContext,
) {
    let outer_scroll_handle = ScrollHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| NestedVirtualizedTreeWheelView {
        outer_scroll_handle: outer_scroll_handle.clone(),
        virtualized: true,
    });
    draw(cx);
    focus_tree(cx, "tree-wheel-fence");

    let outer_viewport = cx
        .debug_bounds("scroll-area:tree-wheel-fence-outer")
        .expect("the outer scroll area should be mounted");
    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        window.dispatch_event(
            ScrollWheelEvent {
                position: point(
                    outer_viewport.left() + px(220.0),
                    outer_viewport.top() + px(8.0),
                ),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-16.0))),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    });
    let outer_after_wheel = outer_scroll_handle.offset().y;
    let first_row_after_wheel = cx
        .debug_bounds("tree:tree-wheel-fence:item:item-0001")
        .expect("a visible Tree row should remain mounted after wheel input");
    assert_ne!(
        outer_after_wheel,
        px(0.0),
        "the outer wheel must move the ancestor viewport"
    );

    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_bounds("tree:tree-wheel-fence:item:item-0099")
            .is_none(),
        "a wheel observed before prepaint must cancel stale Tree materialization"
    );
    assert_eq!(
        outer_scroll_handle.offset().y,
        outer_after_wheel,
        "the cancelled request must not overwrite the outer wheel position"
    );
    assert_eq!(
        cx.debug_bounds("tree:tree-wheel-fence:item:item-0001"),
        Some(first_row_after_wheel),
        "the cancelled request must not move the Tree's own virtual viewport"
    );
}

#[open_gpui::test]
fn virtualized_tree_static_handoff_retains_the_interrupted_scroll_fence(
    cx: &mut open_gpui::TestAppContext,
) {
    let outer_scroll_handle = ScrollHandle::new();
    let (view, cx) = cx.add_window_view(|_, _| NestedVirtualizedTreeWheelView {
        outer_scroll_handle: outer_scroll_handle.clone(),
        virtualized: true,
    });
    draw(cx);
    focus_tree(cx, "tree-wheel-fence");

    let outer_viewport = cx
        .debug_bounds("scroll-area:tree-wheel-fence-outer")
        .expect("the outer scroll area should be mounted");
    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        window.dispatch_event(
            ScrollWheelEvent {
                position: point(
                    outer_viewport.left() + px(220.0),
                    outer_viewport.top() + px(8.0),
                ),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-16.0))),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    });
    let outer_after_wheel = outer_scroll_handle.offset().y;
    view.update(cx, |view, cx| {
        view.virtualized = false;
        cx.notify();
    });

    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    let target = cx
        .debug_bounds("tree:tree-wheel-fence:item:item-0099")
        .expect("static Tree handoff should mount every row");
    assert_eq!(
        outer_scroll_handle.offset().y,
        outer_after_wheel,
        "the static handoff must not overwrite the outer wheel position"
    );
    assert!(
        target.top() > outer_viewport.bottom(),
        "the interrupted fence must prevent automatic focus reveal after static handoff; target={target:?}, outer={outer_viewport:?}"
    );
}

#[open_gpui::test]
fn virtualized_tree_late_sibling_focus_claim_cancels_materialization(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let external_focus = cx.focus_handle();
        VirtualizedTreeLateFocusView {
            late_focus_claim: cx.new(move |_| LateTreeFocusClaimView {
                focus: external_focus,
                armed: false,
            }),
        }
    });
    draw(cx);
    focus_tree(cx, "tree-late-focus-claim");
    let first_row_before = cx
        .debug_bounds("tree:tree-late-focus-claim:item:item-0000")
        .expect("the first Tree row should initially be mounted");
    let late_focus_claim =
        cx.update_window_entity(&view, |view, _, _| view.late_focus_claim.clone());

    late_focus_claim.update(cx, |late_focus_claim, cx| {
        late_focus_claim.armed = true;
        cx.notify();
    });
    dispatch_key(cx, "end");
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("late-tree-focus-claim"),
        "the sibling's later render-time claim must win the candidate frame"
    );
    assert!(
        cx.debug_bounds("tree:tree-late-focus-claim:item:item-0099")
            .is_none(),
        "a losing Tree claim must not materialize its former target"
    );
    assert_eq!(
        cx.debug_bounds("tree:tree-late-focus-claim:item:item-0000"),
        Some(first_row_before),
        "a losing Tree claim must preserve the virtual viewport"
    );
}

#[open_gpui::test]
fn virtualized_tree_late_prepaint_focus_claim_cancels_materialization(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let external_focus = cx.focus_handle();
        VirtualizedTreeLatePrepaintFocusView {
            late_focus_claim: cx.new(move |_| LateTreePrepaintFocusClaimView {
                focus: external_focus,
                armed: false,
                focus_stable: false,
            }),
        }
    });
    draw(cx);
    focus_tree(cx, "tree-late-prepaint-focus-claim");
    let first_row_before = cx
        .debug_bounds("tree:tree-late-prepaint-focus-claim:item:item-0000")
        .expect("the first Tree row should initially be mounted");
    let late_focus_claim =
        cx.update_window_entity(&view, |view, _, _| view.late_focus_claim.clone());

    late_focus_claim.update(cx, |late_focus_claim, cx| {
        late_focus_claim.armed = true;
        cx.notify();
    });
    dispatch_key(cx, "end");
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("late-tree-prepaint-focus-claim"),
        "the sibling's later prepaint-commit claim must win the candidate frame"
    );
    assert!(
        cx.debug_bounds("tree:tree-late-prepaint-focus-claim:item:item-0099")
            .is_none(),
        "a prepaint-commit focus override must prevent stale Tree materialization"
    );
    assert_eq!(
        cx.debug_bounds("tree:tree-late-prepaint-focus-claim:item:item-0000"),
        Some(first_row_before),
        "a prepaint-commit focus override must preserve the virtual viewport"
    );
}

#[open_gpui::test]
fn virtualized_tree_focus_stable_commit_rejects_late_focus_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let external_focus = cx.focus_handle();
        VirtualizedTreeLatePrepaintFocusView {
            late_focus_claim: cx.new(move |_| LateTreePrepaintFocusClaimView {
                focus: external_focus,
                armed: false,
                focus_stable: true,
            }),
        }
    });
    draw(cx);
    focus_tree(cx, "tree-late-prepaint-focus-claim");
    let late_focus_claim =
        cx.update_window_entity(&view, |view, _, _| view.late_focus_claim.clone());

    late_focus_claim.update(cx, |late_focus_claim, cx| {
        late_focus_claim.armed = true;
        cx.notify();
    });
    dispatch_key(cx, "end");
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("tree:tree-late-prepaint-focus-claim:item:item-0099"),
        "a focus-stable commit must reject a late competing focus claim"
    );
    assert!(
        cx.debug_bounds("tree:tree-late-prepaint-focus-claim:item:item-0099")
            .is_some(),
        "the focus-stable phase must retain the Tree materialization it validated"
    );
    assert!(
        !cx.debug_selector_is_focused("late-tree-prepaint-focus-claim"),
        "the late focus-stable commit must not take focus"
    );
}

#[open_gpui::test]
fn virtualized_tree_retries_a_rejected_static_handoff_claim_when_interactive_again(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| VirtualizedTreeRejectedFocusRetryView {
        presentation: SubtreePresentation::Visible,
        virtualized: true,
        external_focus: cx.focus_handle(),
    });
    draw(cx);
    focus_tree(cx, "tree-rejected-focus-retry");

    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        view.update(cx, |view, cx| {
            view.virtualized = false;
            view.presentation = SubtreePresentation::Inert;
            cx.notify();
        });
    });
    draw(cx);

    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("tree:tree-rejected-focus-retry:item:item-0099"),
        "a rejected static handoff claim must retry after its target becomes interactive again"
    );
}

#[open_gpui::test]
fn virtualized_tree_does_not_retry_a_rejected_static_handoff_after_a_new_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| VirtualizedTreeRejectedFocusRetryView {
        presentation: SubtreePresentation::Visible,
        virtualized: true,
        external_focus: cx.focus_handle(),
    });
    draw(cx);
    focus_tree(cx, "tree-rejected-focus-retry");
    let external_focus = cx.update_window_entity(&view, |view, _, _| view.external_focus.clone());

    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        view.update(cx, |view, cx| {
            view.virtualized = false;
            view.presentation = SubtreePresentation::Inert;
            cx.notify();
        });
    });
    draw(cx);

    cx.update(|window, cx| {
        external_focus.focus(window, cx);
        view.update(cx, |view, cx| {
            view.presentation = SubtreePresentation::Visible;
            cx.notify();
        });
    });
    cx.run_until_parked();
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("tree-rejected-focus-retry-external"),
        "a newer focus claim must prevent a rejected Tree handoff from retrying or reclaiming focus"
    );
    assert!(
        !cx.debug_selector_is_focused("tree:tree-rejected-focus-retry:item:item-0099"),
        "the rejected Tree handoff must not regain focus after a newer claim"
    );
}

struct VirtualizedTreeFocusHandoffView {
    external_focus: FocusHandle,
    focus_external_on_select: bool,
    selection_count: Rc<RefCell<usize>>,
}

impl Render for VirtualizedTreeFocusHandoffView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut tree = Tree::new(
            "focus-handoff-tree",
            "Focus handoff tree",
            (0..100).map(|index| {
                TreeItemDescriptor::new(format!("item-{index:04}"), format!("Item {index:04}"))
            }),
        )
        .small()
        .virtualized(true)
        .viewport_item_count(4)
        .overscan_count(0)
        .default_focused("item-0000");
        if self.focus_external_on_select {
            let external_focus = self.external_focus.clone();
            let selection_count = self.selection_count.clone();
            tree = tree.on_select(move |_, window, cx| {
                *selection_count.borrow_mut() += 1;
                external_focus.focus(window, cx);
            });
        }

        div()
            .size_full()
            .flex()
            .gap_2()
            .child(div().w(px(280.0)).h(px(112.0)).child(tree))
            .child(
                div()
                    .id("focus-handoff-target")
                    .debug_selector(|| "tree-focus-handoff-target".into())
                    .w(px(80.0))
                    .h(px(40.0))
                    .focusable()
                    .track_focus(&self.external_focus),
            )
    }
}

#[open_gpui::test]
fn virtualized_tree_losing_same_turn_focus_claim_does_not_materialize_or_reclaim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| VirtualizedTreeFocusHandoffView {
        external_focus: cx.focus_handle(),
        focus_external_on_select: false,
        selection_count: Rc::new(RefCell::new(0)),
    });
    draw(cx);
    focus_tree(cx, "focus-handoff-tree");
    assert!(cx.debug_selector_is_focused("tree:focus-handoff-tree:item:item-0000"));
    let first_row_before = cx
        .debug_bounds("tree:focus-handoff-tree:item:item-0000")
        .expect("the first row should initially be mounted");

    let external_focus = cx.update_window_entity(&view, |view, _, _| view.external_focus.clone());
    cx.update(|window, cx| {
        window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        external_focus.focus(window, cx);
    });
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert!(
        cx.debug_selector_is_focused("tree-focus-handoff-target"),
        "the later same-turn focus request must remain the winning claim"
    );
    assert!(
        cx.debug_bounds("tree:focus-handoff-tree:item:item-0099")
            .is_none(),
        "a superseded Tree claim must not materialize its former target"
    );
    assert_eq!(
        cx.debug_bounds("tree:focus-handoff-tree:item:item-0000"),
        Some(first_row_before),
        "a superseded Tree claim must not move the virtual viewport"
    );
}

#[open_gpui::test]
fn virtualized_tree_on_select_focus_handoff_cancels_the_pending_materialization(
    cx: &mut open_gpui::TestAppContext,
) {
    let selection_count = Rc::new(RefCell::new(0));
    let (_, cx) = cx.add_window_view(|_, cx| VirtualizedTreeFocusHandoffView {
        external_focus: cx.focus_handle(),
        focus_external_on_select: true,
        selection_count: selection_count.clone(),
    });
    draw(cx);
    focus_tree(cx, "focus-handoff-tree");
    let first_row_before = cx
        .debug_bounds("tree:focus-handoff-tree:item:item-0000")
        .expect("the first row should initially be mounted");

    dispatch_key(cx, "end");
    dispatch_key(cx, "enter");
    for _ in 0..3 {
        draw(cx);
        cx.run_until_parked();
    }

    assert_eq!(*selection_count.borrow(), 1);
    assert!(
        cx.debug_selector_is_focused("tree-focus-handoff-target"),
        "the callback focus request must supersede the Tree claim"
    );
    assert!(
        cx.debug_bounds("tree:focus-handoff-tree:item:item-0099")
            .is_none(),
        "the callback focus request must cancel stale target materialization"
    );
    assert_eq!(
        cx.debug_bounds("tree:focus-handoff-tree:item:item-0000"),
        Some(first_row_before),
        "the callback focus request must preserve the virtual viewport"
    );
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
