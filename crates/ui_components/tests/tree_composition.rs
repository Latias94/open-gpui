#[path = "support/a11y.rs"]
mod a11y_support;

use std::{cell::RefCell, collections::BTreeMap, collections::BTreeSet, rc::Rc};

use open_gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, VisualTestContext, Window, accesskit, div, point, px,
};
use open_gpui_ui_components::gpui_adapter::{TextInputController, init_text_input};
use open_gpui_ui_components::theme::{ThemeContext, ThemeMode, ThemeResolver, ThemeScope};
use open_gpui_ui_components::tree::{
    Tree, TreeChildrenLoadState, TreeDropPosition, TreeItemDescriptor, TreeItemState, TreeMove,
    TreeRowAction, TreeRowAppearance, TreeSelection, TreeToggle, apply_tree_move,
};
use open_gpui_ui_components::{ActivationKey, ActivationSource, Button, ColorIntent, TextInput};
use open_gpui_ui_core::{Sizable, ui_px};

use a11y_support::node_with_label;

fn objects() -> Vec<TreeItemDescriptor> {
    vec![
        TreeItemDescriptor::new("alpha", "Alpha").child(TreeItemDescriptor::new("child", "Child")),
        TreeItemDescriptor::new("beta", "Beta"),
    ]
}

#[test]
fn controlled_values_override_defaults_including_empty_selection_and_expansion() {
    let tree = Tree::new("controlled", "Objects", objects())
        .default_selected("alpha")
        .selected(Some("beta".to_owned()))
        .default_selected("alpha")
        .default_expanded_values(["alpha"])
        .expanded_values(std::iter::empty::<String>())
        .default_expanded_values(["alpha"]);
    let state = tree.state();
    assert_eq!(state.selected_value(), Some("beta"));
    assert!(!state.item_by_value("alpha").unwrap().expanded());
    assert!(state.item_by_value("child").is_none());

    let empty = Tree::new("empty", "Objects", objects())
        .selected(None)
        .default_selected("alpha")
        .state();
    assert_eq!(empty.selected_value(), None);
    assert_eq!(empty.focused_value(), Some("alpha"));

    let explicitly_collapsed = Tree::new(
        "collapsed",
        "Objects",
        [TreeItemDescriptor::new("alpha", "Alpha")
            .expanded(true)
            .child(TreeItemDescriptor::new("child", "Child"))],
    )
    .expanded_values(std::iter::empty::<String>())
    .state();
    assert_eq!(explicitly_collapsed.items().len(), 1);
    assert!(!explicitly_collapsed.items()[0].expanded());
}

#[test]
fn lazy_children_and_reordered_rows_keep_accepted_values_and_render_identity() {
    let lazy = Tree::new(
        "lazy",
        "Objects",
        [TreeItemDescriptor::new("alpha", "Alpha").with_children_unloaded()],
    )
    .selected(Some("grandchild".to_owned()))
    .expanded_values(["alpha", "child"])
    .state();
    let toggle = TreeToggle::from_item(&lazy.items()[0]).unwrap();
    assert!(!toggle.expanded());
    assert_eq!(
        toggle.children_load_state(),
        &TreeChildrenLoadState::Unloaded
    );

    let loaded = Tree::new(
        "lazy",
        "Objects",
        [TreeItemDescriptor::new("alpha", "Alpha").child(
            TreeItemDescriptor::new("child", "Child")
                .child(TreeItemDescriptor::new("grandchild", "Grandchild")),
        )],
    )
    .selected(Some("grandchild".to_owned()))
    .expanded_values(["alpha", "child"])
    .behavior_snapshot(ui_px(0.0), ui_px(300.0));
    assert_eq!(loaded.state().selected_value(), Some("grandchild"));
    let accepted_key = loaded.rows()[2].render_key();

    let reordered = Tree::new(
        "lazy",
        "Objects",
        [
            TreeItemDescriptor::new("beta", "Beta"),
            TreeItemDescriptor::new("alpha", "Alpha").child(
                TreeItemDescriptor::new("child", "Child")
                    .child(TreeItemDescriptor::new("grandchild", "Grandchild")),
            ),
        ],
    )
    .selected(Some("grandchild".to_owned()))
    .expanded_values(["alpha", "child"])
    .behavior_snapshot(ui_px(0.0), ui_px(300.0));
    assert_eq!(reordered.state().selected_value(), Some("grandchild"));
    assert_eq!(reordered.rows()[3].render_key(), accepted_key);
}

struct CompositionProbe {
    items: Vec<TreeItemDescriptor>,
    selected: Option<String>,
    expanded: BTreeSet<String>,
    controlled: bool,
    accept_requests: bool,
    virtualized: bool,
    selections: Rc<RefCell<Vec<String>>>,
    toggles: Rc<RefCell<Vec<TreeToggle>>>,
    moves: Rc<RefCell<Vec<TreeMove>>>,
    actions: Rc<RefCell<Vec<ActivationSource>>>,
    rendered: Rc<RefCell<BTreeMap<String, TreeItemState>>>,
    themes: Rc<RefCell<Vec<ThemeMode>>>,
}

impl CompositionProbe {
    fn new(virtualized: bool) -> Self {
        Self {
            items: objects(),
            selected: Some("alpha".to_owned()),
            expanded: BTreeSet::new(),
            controlled: true,
            accept_requests: false,
            virtualized,
            selections: Rc::default(),
            toggles: Rc::default(),
            moves: Rc::default(),
            actions: Rc::default(),
            rendered: Rc::default(),
            themes: Rc::default(),
        }
    }
}

impl Render for CompositionProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered.borrow_mut().clear();
        let rendered = self.rendered.clone();
        let actions = self.actions.clone();
        let themes = self.themes.clone();
        let moves = self.moves.clone();
        let mut tree = Tree::new("composition", "Objects", self.items.clone())
            .small()
            .default_focused("alpha")
            .virtualized(self.virtualized)
            .viewport_item_count(4)
            .overscan_count(1)
            .draggable(true)
            .on_select(cx.processor(|probe, selection: TreeSelection, _, cx| {
                probe
                    .selections
                    .borrow_mut()
                    .push(selection.value().to_owned());
                if probe.accept_requests {
                    probe.selected = Some(selection.value().to_owned());
                    cx.notify();
                }
            }))
            .on_toggle(cx.processor(|probe, toggle: TreeToggle, _, cx| {
                probe.toggles.borrow_mut().push(toggle.clone());
                if probe.accept_requests {
                    if toggle.expanded() {
                        probe.expanded.insert(toggle.value().to_owned());
                    } else {
                        probe.expanded.remove(toggle.value());
                    }
                    cx.notify();
                }
            }))
            .on_move(move |movement, _, _| moves.borrow_mut().push(movement))
            .row_content(move |item, _, _| {
                rendered
                    .borrow_mut()
                    .insert(item.value().to_owned(), item.clone());
                let actions = actions.clone();
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(format!("Object: {}", item.label())))
                    .child(TreeRowAction::new(
                        format!("action:{}", item.value()),
                        Button::new(
                            format!("inspect:{}", item.value()),
                            format!("Inspect {}", item.label()),
                        )
                        .small()
                        .disabled(item.disabled())
                        .on_activate(move |activation, _, _| {
                            actions.borrow_mut().push(activation.source());
                        }),
                    ))
            })
            .row_appearance(move |item, appearance, window, cx| {
                themes
                    .borrow_mut()
                    .push(ThemeResolver::current(window, cx).mode());
                if item.selected() {
                    appearance.with_foreground(ColorIntent::new(
                        open_gpui_ui_core::semantic::ACCENT,
                        0x123456,
                    ))
                } else {
                    appearance
                }
            });
        if self.controlled {
            tree = tree
                .selected(self.selected.clone())
                .expanded_values(self.expanded.clone());
        } else {
            tree = tree.default_expanded_values(self.expanded.clone());
            if let Some(selected) = self.selected.as_ref() {
                tree = tree.default_selected(selected.clone());
            }
        }

        div().size_full().child(ThemeScope::new(
            "composition-theme",
            ThemeContext::dark(),
            div().w(px(360.0)).h(px(180.0)).child(tree),
        ))
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = cx
        .debug_bounds(selector)
        .expect("composed tree element should render");
    cx.simulate_click(bounds.center(), Default::default());
    draw(cx);
}

fn press_key(cx: &mut VisualTestContext, key: &'static str) {
    let keystroke = Keystroke::parse(key).expect("valid test key");
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
    draw(cx);
}

fn dispatch(cx: &mut VisualTestContext, label: &str, action: accesskit::Action) {
    let update = cx
        .latest_accessibility_tree_update()
        .expect("tree should publish semantics");
    let (target_node, _) = node_with_label(&update, label);
    assert!(cx.dispatch_accessibility_action(accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node,
        data: None,
    }));
    draw(cx);
}

#[open_gpui::test]
fn controlled_selection_rejects_pointer_keyboard_and_accessibility_until_owner_accepts(
    cx: &mut open_gpui::TestAppContext,
) {
    for virtualized in [false, true] {
        let probe = CompositionProbe::new(virtualized);
        let selections = probe.selections.clone();
        let rendered = probe.rendered.clone();
        let (view, cx) = cx.add_window_view(|_, _| probe);
        assert!(cx.activate_accessibility());
        draw(cx);
        let initial = cx.latest_accessibility_tree_update().unwrap();
        let (beta_id, beta) = node_with_label(&initial, "Beta");
        assert_eq!(beta.role(), accesskit::Role::TreeItem);
        assert_eq!(beta.is_selected(), Some(false));
        assert!(beta.supports_action(accesskit::Action::Click));

        click(cx, "tree:composition:item:beta");
        assert!(rendered.borrow()["alpha"].selected());
        assert!(!rendered.borrow()["beta"].selected());
        assert!(cx.debug_selector_is_focused("tree:composition:item:beta"));

        cx.simulate_keystrokes("enter");
        draw(cx);
        dispatch(cx, "Beta", accesskit::Action::Click);
        assert_eq!(selections.borrow().as_slice(), &["beta", "beta", "beta"]);
        assert!(rendered.borrow()["alpha"].selected());
        assert!(!rendered.borrow()["beta"].selected());

        view.update(cx, |probe, cx| {
            probe.accept_requests = true;
            cx.notify();
        });
        draw(cx);
        dispatch(cx, "Beta", accesskit::Action::Click);
        assert_eq!(
            view.read_with(cx, |probe, _| probe.selected.clone()),
            Some("beta".to_owned())
        );
        assert!(rendered.borrow()["beta"].selected());
        let accepted = cx.latest_accessibility_tree_update().unwrap();
        let (accepted_beta_id, accepted_beta) = node_with_label(&accepted, "Beta");
        assert_eq!(accepted_beta_id, beta_id);
        assert_eq!(accepted_beta.is_selected(), Some(true));

        view.update(cx, |probe, cx| {
            probe.selected = None;
            cx.notify();
        });
        draw(cx);
        assert!(rendered.borrow().values().all(|item| !item.selected()));
        assert!(cx.debug_selector_is_focused("tree:composition:item:beta"));
    }
}

#[open_gpui::test]
fn controlled_expansion_rejection_and_lazy_loading_keep_application_authority(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut probe = CompositionProbe::new(true);
    probe.items = vec![TreeItemDescriptor::new("alpha", "Alpha").with_children_unloaded()];
    probe.selected = Some("grandchild".to_owned());
    probe.expanded.insert("child".to_owned());
    let toggles = probe.toggles.clone();
    let selections = probe.selections.clone();
    let rendered = probe.rendered.clone();
    let (view, cx) = cx.add_window_view(|_, _| probe);
    assert!(cx.activate_accessibility());
    draw(cx);

    click(cx, "tree:composition:toggle:alpha");
    cx.simulate_keystrokes("right");
    draw(cx);
    dispatch(cx, "Expand Alpha", accesskit::Action::Click);
    assert_eq!(toggles.borrow().len(), 3);
    assert!(toggles.borrow().iter().all(|toggle| toggle.expanded()));
    assert!(
        toggles
            .borrow()
            .iter()
            .all(|toggle| toggle.children_load_state().is_unloaded())
    );
    assert!(!rendered.borrow()["alpha"].expanded());
    assert!(selections.borrow().is_empty());

    view.update(cx, |probe, cx| {
        probe.expanded.insert("alpha".to_owned());
        probe.items =
            vec![TreeItemDescriptor::new("alpha", "Alpha").with_children_loading("Loading")];
        cx.notify();
    });
    draw(cx);
    let loading = cx.latest_accessibility_tree_update().unwrap();
    let (alpha_id, alpha) = node_with_label(&loading, "Alpha");
    assert!(alpha.is_busy());
    let (_, disclosure) = node_with_label(&loading, "Loading Alpha");
    assert!(!disclosure.supports_action(accesskit::Action::Click));
    cx.simulate_keystrokes("left");
    draw(cx);
    assert_eq!(toggles.borrow().len(), 3);

    view.update(cx, |probe, cx| {
        probe.items = vec![
            TreeItemDescriptor::new("alpha", "Alpha").child(
                TreeItemDescriptor::new("child", "Child")
                    .child(TreeItemDescriptor::new("grandchild", "Grandchild")),
            ),
        ];
        cx.notify();
    });
    draw(cx);
    assert!(rendered.borrow()["child"].expanded());
    assert!(rendered.borrow()["grandchild"].selected());
    let loaded = cx.latest_accessibility_tree_update().unwrap();
    assert_eq!(node_with_label(&loaded, "Alpha").0, alpha_id);

    view.update(cx, |probe, cx| {
        probe.expanded.remove("alpha");
        cx.notify();
    });
    draw(cx);
    assert!(!rendered.borrow().contains_key("grandchild"));
    view.update(cx, |probe, cx| {
        probe.expanded.insert("alpha".to_owned());
        cx.notify();
    });
    draw(cx);
    assert!(rendered.borrow()["grandchild"].selected());
    assert!(selections.borrow().is_empty());
}

#[open_gpui::test]
fn uncontrolled_defaults_seed_once_and_hidden_selection_survives_collapse_and_loading(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut probe = CompositionProbe::new(false);
    probe.controlled = false;
    probe.selected = Some("child".to_owned());
    probe.expanded.insert("alpha".to_owned());
    let rendered = probe.rendered.clone();
    let (view, cx) = cx.add_window_view(|_, _| probe);
    draw(cx);
    assert!(rendered.borrow()["child"].selected());

    click(cx, "tree:composition:toggle:alpha");
    assert!(!rendered.borrow().contains_key("child"));
    view.update(cx, |probe, cx| {
        probe.selected = Some("beta".to_owned());
        probe.expanded.clear();
        cx.notify();
    });
    draw(cx);
    click(cx, "tree:composition:toggle:alpha");
    assert!(rendered.borrow()["child"].selected());

    view.update(cx, |probe, cx| {
        probe.items =
            vec![TreeItemDescriptor::new("alpha", "Alpha").with_children_loading("Loading")];
        cx.notify();
    });
    draw(cx);
    view.update(cx, |probe, cx| {
        probe.items = objects();
        cx.notify();
    });
    draw(cx);
    assert!(rendered.borrow()["child"].selected());
    assert!(rendered.borrow()["alpha"].expanded());

    click(cx, "tree:composition:item:beta");
    view.update(cx, |probe, cx| {
        probe.selected = Some("alpha".to_owned());
        cx.notify();
    });
    draw(cx);
    assert!(rendered.borrow()["beta"].selected());
}

#[open_gpui::test]
fn row_actions_preserve_button_activation_without_selecting_dragging_or_navigating_tree(
    cx: &mut open_gpui::TestAppContext,
) {
    for virtualized in [false, true] {
        let probe = CompositionProbe::new(virtualized);
        let selections = probe.selections.clone();
        let moves = probe.moves.clone();
        let actions = probe.actions.clone();
        let themes = probe.themes.clone();
        let (_, cx) = cx.add_window_view(|_, _| probe);
        assert!(cx.activate_accessibility());
        draw(cx);

        click(cx, "button:inspect:beta:root");
        assert_eq!(actions.borrow().as_slice(), &[ActivationSource::Pointer]);
        assert!(cx.debug_selector_is_focused("button:inspect:beta:root"));
        press_key(cx, "enter");
        assert_eq!(actions.borrow().len(), 2);
        assert_eq!(
            actions.borrow().last(),
            Some(&ActivationSource::Keyboard(ActivationKey::Enter))
        );
        press_key(cx, "space");
        assert_eq!(actions.borrow().len(), 3);
        assert_eq!(
            actions.borrow().last(),
            Some(&ActivationSource::Keyboard(ActivationKey::Space))
        );
        cx.simulate_keystrokes("home");
        draw(cx);
        assert!(cx.debug_selector_is_focused("button:inspect:beta:root"));
        assert_eq!(actions.borrow().len(), 3);
        dispatch(cx, "Inspect Beta", accesskit::Action::Click);
        assert_eq!(
            actions.borrow().last(),
            Some(&ActivationSource::Accessibility)
        );
        assert!(selections.borrow().is_empty());

        let action = cx
            .debug_bounds("button:inspect:beta:root")
            .unwrap()
            .center();
        cx.simulate_mouse_down(action, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            point(action.x + px(20.0), action.y),
            MouseButton::Left,
            Default::default(),
        );
        assert!(!cx.update(|_, cx| cx.has_active_drag()));
        cx.simulate_mouse_up(action, MouseButton::Left, Default::default());
        draw(cx);
        assert!(moves.borrow().is_empty());
        assert!(selections.borrow().is_empty());
        assert!(!themes.borrow().is_empty());
        assert!(themes.borrow().iter().all(|mode| *mode == ThemeMode::Dark));

        let update = cx.latest_accessibility_tree_update().unwrap();
        let (_, row) = node_with_label(&update, "Beta");
        let (action_id, action) = node_with_label(&update, "Inspect Beta");
        assert_eq!(row.role(), accesskit::Role::TreeItem);
        assert_eq!(action.role(), accesskit::Role::Button);
        assert!(row.children().contains(&action_id));
        click(cx, "tree:composition:item:beta");
        assert_eq!(selections.borrow().as_slice(), &["beta"]);
    }
}

#[open_gpui::test]
fn row_editors_preserve_text_composition_and_cursor_keys_without_tree_requests(
    cx: &mut open_gpui::TestAppContext,
) {
    struct EditorProbe {
        editor: Entity<TextInputController>,
        virtualized: bool,
        selections: Rc<RefCell<Vec<TreeSelection>>>,
        toggles: Rc<RefCell<Vec<TreeToggle>>>,
    }

    impl Render for EditorProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let editor = self.editor.clone();
            let selections = self.selections.clone();
            let toggles = self.toggles.clone();
            div().size_full().child(
                div().w(px(360.0)).h(px(180.0)).child(
                    Tree::new("editor-composition", "Editable objects", objects())
                        .small()
                        .virtualized(self.virtualized)
                        .viewport_item_count(4)
                        .default_selected("alpha")
                        .default_focused("alpha")
                        .on_select(move |selection, _, _| selections.borrow_mut().push(selection))
                        .on_toggle(move |toggle, _, _| toggles.borrow_mut().push(toggle))
                        .row_content(move |item, _, _| {
                            if item.value() == "beta" {
                                TreeRowAction::new(
                                    "row-editor",
                                    TextInput::new("row-name", "Object name")
                                        .small()
                                        .controller(editor.clone()),
                                )
                                .into_any_element()
                            } else {
                                div().child(item.label().to_owned()).into_any_element()
                            }
                        }),
                ),
            )
        }
    }

    cx.update(init_text_input);
    for virtualized in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| EditorProbe {
            editor: cx.new(TextInputController::new),
            virtualized,
            selections: Rc::default(),
            toggles: Rc::default(),
        });
        draw(cx);
        let editor = view.read_with(cx, |probe, _| probe.editor.clone());
        click(cx, "text-input:row-name:root");
        assert!(cx.update(|window, cx| editor.focus_handle(cx).is_focused(window)));
        cx.simulate_input("ab");
        draw(cx);
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.value().to_owned()),
            "ab"
        );
        press_key(cx, "left");
        cx.simulate_input("x");
        draw(cx);
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.value().to_owned()),
            "axb"
        );
        cx.simulate_marked_text(None, "に", Some(1..1));
        draw(cx);
        cx.simulate_input("界");
        draw(cx);
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.value().to_owned()),
            "ax界b"
        );
        assert!(cx.update(|window, cx| editor.focus_handle(cx).is_focused(window)));
        assert!(view.read_with(cx, |probe, _| probe.selections.borrow().is_empty()));
        assert!(view.read_with(cx, |probe, _| probe.toggles.borrow().is_empty()));
    }
}

struct ExternalTreeFocusProbe {
    focus: FocusHandle,
    claim_on_render: bool,
}

impl Render for ExternalTreeFocusProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.claim_on_render {
            self.claim_on_render = false;
            self.focus.focus(window, cx);
        }
        div()
            .id("external-tree-focus")
            .debug_selector(|| "external-tree-focus".to_owned())
            .w(px(100.0))
            .h(px(30.0))
            .focusable()
            .track_focus(&self.focus)
    }
}

struct TreeFocusProbe {
    items: Vec<TreeItemDescriptor>,
    expanded: BTreeSet<String>,
    virtualized: bool,
    external: Entity<ExternalTreeFocusProbe>,
}

impl TreeFocusProbe {
    fn new(virtualized: bool, cx: &mut Context<Self>) -> Self {
        Self {
            items: vec![
                TreeItemDescriptor::new("before", "Before"),
                TreeItemDescriptor::new("alpha", "Alpha").child(
                    TreeItemDescriptor::new("child", "Child")
                        .child(TreeItemDescriptor::new("grandchild", "Grandchild")),
                ),
                TreeItemDescriptor::new("beta", "Beta"),
            ],
            expanded: BTreeSet::from(["alpha".to_owned(), "child".to_owned()]),
            virtualized,
            external: cx.new(|cx| ExternalTreeFocusProbe {
                focus: cx.focus_handle(),
                claim_on_render: false,
            }),
        }
    }
}

impl Render for TreeFocusProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div().w(px(360.0)).h(px(180.0)).child(
                    Tree::new("focus-composition", "Objects", self.items.clone())
                        .small()
                        .selected(Some("grandchild".to_owned()))
                        .expanded_values(self.expanded.clone())
                        .default_focused("before")
                        .virtualized(self.virtualized)
                        .viewport_item_count(4)
                        .overscan_count(1),
                ),
            )
            .child(self.external.clone())
    }
}

#[open_gpui::test]
fn controlled_collapse_and_lazy_unload_restore_focus_to_the_nearest_visible_ancestor(
    cx: &mut open_gpui::TestAppContext,
) {
    for virtualized in [false, true] {
        let (view, cx) = cx.add_window_view(|_, cx| TreeFocusProbe::new(virtualized, cx));
        assert!(cx.activate_accessibility());
        draw(cx);
        dispatch(cx, "Grandchild", accesskit::Action::Focus);
        assert!(cx.debug_selector_is_focused("tree:focus-composition:item:grandchild"));
        view.update(cx, |probe, cx| {
            probe.expanded.remove("child");
            cx.notify();
        });
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_selector_is_focused("tree:focus-composition:item:child"));
        let update = cx
            .latest_accessibility_tree_update()
            .expect("recovered tree");
        assert_eq!(update.focus, node_with_label(&update, "Child").0);
        press_key(cx, "down");
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_selector_is_focused("tree:focus-composition:item:beta"));

        view.update(cx, |probe, cx| {
            probe.expanded.insert("child".to_owned());
            cx.notify();
        });
        draw(cx);
        dispatch(cx, "Grandchild", accesskit::Action::Focus);
        view.update(cx, |probe, cx| {
            probe.items[1] = TreeItemDescriptor::new("alpha", "Alpha")
                .with_children_loading("Refreshing children");
            cx.notify();
        });
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_selector_is_focused("tree:focus-composition:item:alpha"));
        press_key(cx, "down");
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_selector_is_focused("tree:focus-composition:item:beta"));
    }
}

#[open_gpui::test]
fn controlled_collapse_preserves_external_focus_and_yields_to_a_later_sibling_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    for virtualized in [false, true] {
        for late_claim in [false, true] {
            let (view, cx) = cx.add_window_view(|_, cx| TreeFocusProbe::new(virtualized, cx));
            assert!(cx.activate_accessibility());
            draw(cx);
            dispatch(cx, "Grandchild", accesskit::Action::Focus);
            let external = view.read_with(cx, |probe, _| probe.external.clone());
            if late_claim {
                external.update(cx, |external, cx| {
                    external.claim_on_render = true;
                    cx.notify();
                });
            } else {
                let focus = external.read_with(cx, |external, _| external.focus.clone());
                cx.update(|window, cx| focus.focus(window, cx));
                draw(cx);
            }
            view.update(cx, |probe, cx| {
                probe.expanded.remove("child");
                cx.notify();
            });
            for _ in 0..3 {
                draw(cx);
            }
            assert!(
                cx.debug_selector_is_focused("external-tree-focus"),
                "Tree must not replace external focus; virtualized={virtualized}, late={late_claim}"
            );
        }
    }
}

#[open_gpui::test]
fn accessibility_positions_count_siblings_including_disabled_items(
    cx: &mut open_gpui::TestAppContext,
) {
    for virtualized in [false, true] {
        let mut probe = CompositionProbe::new(virtualized);
        probe.items = vec![
            TreeItemDescriptor::new("alpha", "Alpha")
                .child(TreeItemDescriptor::new("disabled-child", "Disabled child").disabled(true))
                .child(TreeItemDescriptor::new("child", "Child")),
            TreeItemDescriptor::new("disabled-root", "Disabled root").disabled(true),
            TreeItemDescriptor::new("beta", "Beta"),
        ];
        probe.expanded.insert("alpha".to_owned());
        let (_, cx) = cx.add_window_view(|_, _| probe);
        assert!(cx.activate_accessibility());
        draw(cx);
        let update = cx
            .latest_accessibility_tree_update()
            .expect("tree semantics");
        for (label, position, count, level) in [
            ("Alpha", 1, 3, 1),
            ("Disabled child", 1, 2, 2),
            ("Child", 2, 2, 2),
            ("Disabled root", 2, 3, 1),
            ("Beta", 3, 3, 1),
        ] {
            let (_, item) = node_with_label(&update, label);
            assert_eq!(item.position_in_set(), Some(position), "{label}");
            assert_eq!(item.size_of_set(), Some(count), "{label}");
            assert_eq!(item.level(), Some(level), "{label}");
        }
        let (_, disabled) = node_with_label(&update, "Disabled child");
        assert!(disabled.is_disabled());
        assert!(!disabled.supports_action(accesskit::Action::Focus));
        dispatch(cx, "Alpha", accesskit::Action::Focus);
        press_key(cx, "down");
        for _ in 0..3 {
            draw(cx);
        }
        assert!(cx.debug_selector_is_focused("tree:composition:item:child"));
    }
}

#[open_gpui::test]
fn rejected_drag_move_preserves_model_and_selection_while_reorder_keeps_row_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut probe = CompositionProbe::new(true);
    probe.expanded.insert("alpha".to_owned());
    probe.selected = Some("child".to_owned());
    let moves = probe.moves.clone();
    let selections = probe.selections.clone();
    let rendered = probe.rendered.clone();
    let (view, cx) = cx.add_window_view(|_, _| probe);
    assert!(cx.activate_accessibility());
    let _ = cx.update(|window, _| window.activate_window());
    draw(cx);
    let initial = cx.latest_accessibility_tree_update().unwrap();
    let child_id = node_with_label(&initial, "Child").0;
    let source = cx
        .debug_bounds("tree:composition:item:child")
        .unwrap()
        .center();
    let target = cx
        .debug_bounds("tree:composition:drop:before:beta")
        .unwrap()
        .center();
    cx.simulate_mouse_down(source, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(
        point(source.x + px(18.0), source.y),
        MouseButton::Left,
        Default::default(),
    );
    cx.simulate_mouse_move(target, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(target, MouseButton::Left, Default::default());
    draw(cx);
    assert_eq!(moves.borrow().len(), 1);
    assert!(selections.borrow().is_empty());
    assert!(rendered.borrow()["child"].selected());
    assert_eq!(rendered.borrow()["child"].parent_value(), Some("alpha"));
    assert_eq!(moves.borrow()[0].position(), TreeDropPosition::Before);

    let movement = moves.borrow()[0].clone();
    view.update(cx, |probe, cx| {
        probe.items = apply_tree_move(probe.items.clone(), &movement).unwrap();
        cx.notify();
    });
    draw(cx);
    assert!(rendered.borrow()["child"].selected());
    assert_eq!(rendered.borrow()["child"].parent_value(), None);
    let reordered = cx.latest_accessibility_tree_update().unwrap();
    assert_eq!(node_with_label(&reordered, "Child").0, child_id);

    let state = view.read_with(cx, |probe, _| {
        Tree::new("validation", "Objects", probe.items.clone()).state()
    });
    assert!(
        state
            .move_for_drop("child", "child", TreeDropPosition::Inside)
            .is_none()
    );
    let appearance = TreeRowAppearance::from_item(&rendered.borrow()["child"]);
    assert_eq!(
        appearance.background().state(),
        open_gpui_ui_components::ColorState::Selected
    );
}
