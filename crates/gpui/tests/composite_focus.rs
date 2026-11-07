#![cfg(feature = "test-support")]

//! Public consumers of Zed's composite accessibility and keyboard continuity mechanisms
//! (83d4847462 and 2268045a11), at fixed source 635026fabc38440fce5e1767d125026a17b12012.

use open_gpui::{
    AppContext as _, ClickEvent, Context, Empty, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBinding, KeyDownEvent, KeyUpEvent, Keystroke, ParentElement as _, QuitMode,
    Render, Role, StatefulInteractiveElement as _, Styled as _, Subscription, TargetedEvent,
    TestAppContext, VisualTestContext, Window, WindowHandle,
    accesskit::{Node, NodeId, TreeUpdate},
    actions, div, px, size,
};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

actions!(
    composite_focus,
    [NextItem, PreviousItem, AcceptItem, CancelMenu, AdvanceTab]
);

const ITEMS: [&str; 3] = ["Open document", "Save document", "Close document"];

struct MenuSource {
    selected: usize,
    revision: usize,
    items_visible: bool,
}

#[derive(Default)]
struct Observations {
    opens: Cell<usize>,
    accepts: Cell<usize>,
    last_accepted: Cell<Option<usize>>,
    renders: Cell<usize>,
    releases: Cell<usize>,
}

struct CompositeMenu {
    trigger: FocusHandle,
    container: FocusHandle,
    outside: FocusHandle,
    source: Entity<MenuSource>,
    open: bool,
    observed: Rc<Observations>,
    _source_subscription: Subscription,
}

impl CompositeMenu {
    fn new(source: Entity<MenuSource>, observed: Rc<Observations>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&source, |_, _, cx| cx.notify());
        cx.on_release({
            let observed = Rc::clone(&observed);
            move |_, _| observed.releases.set(observed.releases.get() + 1)
        })
        .detach();
        Self {
            trigger: cx.focus_handle(),
            container: cx.focus_handle(),
            outside: cx.focus_handle(),
            source,
            open: false,
            observed,
            _source_subscription: subscription,
        }
    }

    fn select(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.source.update(cx, |source, cx| {
            source.selected = if forward {
                (source.selected + 1) % ITEMS.len()
            } else {
                (source.selected + ITEMS.len() - 1) % ITEMS.len()
            };
            cx.notify();
        });
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.trigger.focus(window, cx);
        cx.notify();
    }
}

impl Render for CompositeMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let source = self.source.read(cx);
        let selected = source.selected;
        let revision = source.revision;
        let items_visible = source.items_visible;
        let mut root = div()
            .id("composite-root")
            .role(Role::Group)
            .aria_label("Document controls")
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("menu-trigger")
                    .debug_selector(|| "composite:trigger".to_owned())
                    .role(Role::Button)
                    .aria_label("Document menu trigger")
                    .track_focus(&self.trigger)
                    .w(px(200.0))
                    .h(px(28.0))
                    .on_click(cx.listener(
                        |this, event: &TargetedEvent<ClickEvent>, window, cx| {
                            assert!(event.window_event().is_keyboard());
                            this.observed.opens.set(this.observed.opens.get() + 1);
                            this.open = true;
                            this.container.focus(window, cx);
                            cx.notify();
                        },
                    )),
            );
        if self.open {
            let mut menu = div()
                .id("document-menu")
                .debug_selector(|| "composite:container".to_owned())
                .role(Role::Menu)
                .aria_label("Document menu")
                .track_focus(&self.container)
                .key_context("CompositeMenu")
                .w(px(220.0))
                .min_h(px(28.0))
                .flex()
                .flex_col()
                .on_action(cx.listener(|this, _: &NextItem, _, cx| this.select(true, cx)))
                .on_action(cx.listener(|this, _: &PreviousItem, _, cx| this.select(false, cx)))
                .on_action(cx.listener(|this, _: &AcceptItem, window, cx| {
                    this.observed.accepts.set(this.observed.accepts.get() + 1);
                    this.observed
                        .last_accepted
                        .set(Some(this.source.read(cx).selected));
                    this.close(window, cx);
                }))
                .on_action(cx.listener(|this, _: &CancelMenu, window, cx| this.close(window, cx)));
            if items_visible {
                for (index, label) in ITEMS.into_iter().enumerate() {
                    let mut item = div()
                        .id(("menu-item", index))
                        .role(Role::MenuItem)
                        .aria_label(label)
                        .w(px(200.0))
                        .h(px(28.0))
                        .a11y_synthetic_children(move |builder| {
                            let id = builder.synthetic_node_id("detail");
                            let mut detail = Node::new(Role::Label);
                            detail.set_label(format!("{label}: source revision {revision}"));
                            assert!(builder.push_child(id, detail));
                        });
                    if index == selected {
                        item = item.aria_active_descendant();
                    }
                    menu = menu.child(item);
                }
            }
            root = root.child(menu);
        }
        root.child(
            div()
                .id("outside-control")
                .debug_selector(|| "composite:outside".to_owned())
                .role(Role::Button)
                .aria_label("Outside control")
                .track_focus(&self.outside)
                .w(px(200.0))
                .h(px(28.0)),
        )
    }
}

fn open_consumer(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<CompositeMenu>,
    VisualTestContext,
    Entity<MenuSource>,
    Rc<Observations>,
) {
    cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.bind_keys([
            KeyBinding::new("down", NextItem, Some("CompositeMenu")),
            KeyBinding::new("up", PreviousItem, Some("CompositeMenu")),
            KeyBinding::new("enter", AcceptItem, Some("CompositeMenu")),
            KeyBinding::new("space", AcceptItem, Some("CompositeMenu")),
            KeyBinding::new("escape", CancelMenu, Some("CompositeMenu")),
        ]);
    });
    let source = cx.new(|_| MenuSource {
        selected: 0,
        revision: 0,
        items_visible: true,
    });
    let observed = Rc::new(Observations::default());
    let handle = cx.open_window(size(px(320.0), px(240.0)), |_, cx| {
        CompositeMenu::new(source.clone(), Rc::clone(&observed), cx)
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    handle
        .update(&mut visual, |view, window, cx| {
            view.trigger.focus(window, cx)
        })
        .unwrap();
    visual.run_until_parked();
    assert!(visual.activate_accessibility());
    assert!(visual.debug_selector_is_focused("composite:trigger"));
    (handle, visual, source, observed)
}

fn key_down(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
}

fn key_up(visual: &mut VisualTestContext, key: &str) {
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse(key).unwrap(),
    });
}

fn press_keys(visual: &mut VisualTestContext, keys: &str) {
    for key in keys.split_whitespace() {
        key_down(visual, key);
        key_up(visual, key);
    }
}

fn labelled<'a>(tree: &'a TreeUpdate, label: &str) -> (NodeId, &'a Node) {
    let mut matches = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label));
    let (id, node) = matches
        .next()
        .unwrap_or_else(|| panic!("missing node {label:?}"));
    assert!(matches.next().is_none(), "duplicate node {label:?}");
    (*id, node)
}

fn exported_key(json: &Value, id: NodeId) -> &str {
    let raw = id.0.to_string();
    json["nodes"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, node)| node["accesskit_id"].as_str() == Some(raw.as_str()))
        .map(|(key, _)| key.as_str())
        .unwrap_or_else(|| panic!("missing exported node {id:?}"))
}

fn assert_focus(
    visual: &mut VisualTestContext,
    actual_label: &str,
    selected_label: Option<&str>,
) -> TreeUpdate {
    visual.run_until_parked();
    let tree = visual
        .latest_accessibility_tree_update()
        .expect("accepted platform tree");
    let json = visual.update(|window, _| {
        serde_json::from_str::<Value>(&window.debug_a11y_tree_json().unwrap()).unwrap()
    });
    let actual = labelled(&tree, actual_label).0;
    assert_eq!(json["gpui_focus"], exported_key(&json, actual));
    match selected_label {
        Some(label) => {
            let selected = labelled(&tree, label).0;
            assert_eq!(tree.focus, selected);
            assert_eq!(
                json["active_descendant_focus"],
                exported_key(&json, selected)
            );
            assert_ne!(
                actual, selected,
                "the actual focus must remain on the container"
            );
        }
        None => {
            assert_eq!(tree.focus, actual);
            assert!(json["active_descendant_focus"].is_null());
        }
    }
    tree
}

#[open_gpui::test]
fn public_composite_navigation_and_source_notifications_keep_container_keyboard_focus(
    cx: &mut TestAppContext,
) {
    let (_, mut visual, source, _) = open_consumer(cx);
    press_keys(&mut visual, "enter");
    let initial = assert_focus(&mut visual, "Document menu", Some(ITEMS[0]));
    let menu_id = labelled(&initial, "Document menu").0;
    let item_ids = ITEMS.map(|label| labelled(&initial, label).0);
    assert_eq!(labelled(&initial, "Document menu").1.children(), item_ids);
    for (label, id) in ITEMS.into_iter().zip(item_ids) {
        let detail = labelled(&initial, &format!("{label}: source revision 0")).0;
        assert_eq!(labelled(&initial, label).1.children(), [detail]);
        assert_ne!(initial.focus, detail);
        assert_ne!(id, detail);
    }

    for (key, selected) in [("down", 1), ("down", 2), ("up", 1)] {
        press_keys(&mut visual, key);
        let tree = assert_focus(&mut visual, "Document menu", Some(ITEMS[selected]));
        assert_eq!(labelled(&tree, "Document menu").0, menu_id);
        assert_eq!(ITEMS.map(|label| labelled(&tree, label).0), item_ids);
        assert!(visual.debug_selector_is_focused("composite:container"));
    }

    let history = visual.accessibility_tree_update_history().len();
    source.update(&mut visual, |source, cx| {
        source.selected = 2;
        source.revision += 1;
        cx.notify();
    });
    let changed = assert_focus(&mut visual, "Document menu", Some(ITEMS[2]));
    assert!(visual.accessibility_tree_update_history().len() > history);
    assert_eq!(ITEMS.map(|label| labelled(&changed, label).0), item_ids);
    for label in ITEMS {
        labelled(&changed, &format!("{label}: source revision 1"));
    }
    assert!(visual.debug_selector_is_focused("composite:container"));
}

#[open_gpui::test]
fn public_composite_declarations_are_ignored_while_an_outsider_has_actual_focus(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, source, _) = open_consumer(cx);
    press_keys(&mut visual, "enter");
    assert_focus(&mut visual, "Document menu", Some(ITEMS[0]));
    handle
        .update(&mut visual, |view, window, cx| {
            view.outside.focus(window, cx)
        })
        .unwrap();
    assert_focus(&mut visual, "Outside control", None);
    source.update(&mut visual, |source, cx| {
        source.selected = 1;
        cx.notify();
    });
    let tree = assert_focus(&mut visual, "Outside control", None);
    labelled(&tree, ITEMS[1]);
    assert!(visual.debug_selector_is_focused("composite:outside"));
    handle
        .update(&mut visual, |view, window, cx| {
            view.container.focus(window, cx)
        })
        .unwrap();
    assert_focus(&mut visual, "Document menu", Some(ITEMS[1]));
}

struct DisplayOnlyMenu {
    display: FocusHandle,
    outside: FocusHandle,
}

impl Render for DisplayOnlyMenu {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("display-only-menu")
                    .role(Role::Menu)
                    .aria_label("Display only menu")
                    .track_accessibility_focus(&self.display)
                    .w(px(160.0))
                    .h(px(40.0))
                    .child(
                        div()
                            .id("display-only-selected")
                            .role(Role::MenuItem)
                            .aria_label("Display only selection")
                            .aria_active_descendant()
                            .w(px(140.0))
                            .h(px(24.0)),
                    ),
            )
            .child(
                div()
                    .id("display-only-outside")
                    .role(Role::Button)
                    .aria_label("Display override outside control")
                    .track_focus(&self.outside)
                    .w(px(160.0))
                    .h(px(24.0)),
            )
    }
}

#[open_gpui::test]
fn public_display_override_cannot_authorize_active_descendants_without_real_input_ancestry(
    cx: &mut TestAppContext,
) {
    let handle = cx.open_window(size(px(200.0), px(120.0)), |_, cx| DisplayOnlyMenu {
        display: cx.focus_handle(),
        outside: cx.focus_handle(),
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    assert!(visual.activate_accessibility());
    assert!(visual.update(|window, app| window.focused(app).is_none()));
    let initial = assert_focus(&mut visual, "Display only menu", None);
    assert_ne!(
        initial.focus,
        labelled(&initial, "Display only selection").0
    );

    handle
        .update(&mut visual, |view, window, cx| {
            view.outside.focus(window, cx)
        })
        .unwrap();
    handle
        .update(&mut visual, |view, window, _| {
            assert!(view.outside.is_focused(window))
        })
        .unwrap();
    let outside = assert_focus(&mut visual, "Display only menu", None);
    assert_ne!(
        outside.focus,
        labelled(&outside, "Display only selection").0
    );
}

#[open_gpui::test]
fn public_composite_removal_and_rebuild_clear_and_restore_active_descendant(
    cx: &mut TestAppContext,
) {
    let (_, mut visual, source, _) = open_consumer(cx);
    press_keys(&mut visual, "enter down");
    let initial = assert_focus(&mut visual, "Document menu", Some(ITEMS[1]));
    let selected_id = initial.focus;
    source.update(&mut visual, |source, cx| {
        source.items_visible = false;
        cx.notify();
    });
    let removed = assert_focus(&mut visual, "Document menu", None);
    assert!(labelled(&removed, "Document menu").1.children().is_empty());
    assert!(removed.nodes.iter().all(|(id, _)| *id != selected_id));
    assert!(visual.debug_selector_is_focused("composite:container"));

    source.update(&mut visual, |source, cx| {
        source.items_visible = true;
        source.selected = 2;
        source.revision += 1;
        cx.notify();
    });
    let rebuilt = assert_focus(&mut visual, "Document menu", Some(ITEMS[2]));
    assert_eq!(labelled(&rebuilt, ITEMS[1]).0, selected_id);
    labelled(&rebuilt, &format!("{}: source revision 1", ITEMS[2]));
    press_keys(&mut visual, "up");
    assert_focus(&mut visual, "Document menu", Some(ITEMS[1]));
}

#[open_gpui::test]
fn public_keyboard_click_requires_uninterrupted_activation_press_and_release(
    cx: &mut TestAppContext,
) {
    let (_, mut visual, _, observed) = open_consumer(cx);
    for (key, other) in [("enter", "space"), ("space", "enter")] {
        let before = observed.opens.get();
        key_up(&mut visual, key);
        assert_eq!(
            observed.opens.get(),
            before,
            "orphan keyup must not activate"
        );

        key_down(&mut visual, key);
        key_up(&mut visual, other);
        assert_eq!(
            observed.opens.get(),
            before + 1,
            "activation keys share the official arm"
        );
        assert_focus(&mut visual, "Document menu", Some(ITEMS[0]));
        key_up(&mut visual, key);
        assert_eq!(
            observed.opens.get(),
            before + 1,
            "an arm is consumed exactly once"
        );
        visual.dispatch_action(CancelMenu);
        let before = observed.opens.get();

        key_down(&mut visual, key);
        key_down(&mut visual, "a");
        key_up(&mut visual, key);
        assert_eq!(observed.opens.get(), before, "another keydown cancels");
        key_up(&mut visual, "a");

        key_down(&mut visual, key);
        key_up(&mut visual, "a");
        key_up(&mut visual, key);
        assert_eq!(observed.opens.get(), before, "another keyup cancels");
        assert_focus(&mut visual, "Document menu trigger", None);

        key_down(&mut visual, key);
        assert_eq!(observed.opens.get(), before, "keydown alone must not click");
        key_up(&mut visual, key);
        assert_eq!(observed.opens.get(), before + 1);
        assert_focus(&mut visual, "Document menu", Some(ITEMS[0]));
        visual.dispatch_action(CancelMenu);
        key_up(&mut visual, key);
        assert_eq!(
            observed.opens.get(),
            before + 1,
            "a press can activate only once"
        );
        assert_focus(&mut visual, "Document menu trigger", None);
    }
}

#[open_gpui::test]
fn public_keyboard_click_rejects_focus_aba_but_accepts_same_focus_reassertion(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, _, observed) = open_consumer(cx);
    for key in ["enter", "space"] {
        let before = observed.opens.get();
        key_down(&mut visual, key);
        handle
            .update(&mut visual, |view, window, cx| {
                view.outside.focus(window, cx);
                view.trigger.focus(window, cx);
            })
            .unwrap();
        assert!(visual.debug_selector_is_focused("composite:trigger"));
        key_up(&mut visual, key);
        assert_eq!(
            observed.opens.get(),
            before,
            "focus ABA must cancel a held activation"
        );

        key_down(&mut visual, key);
        handle
            .update(&mut visual, |view, window, cx| {
                view.trigger.focus(window, cx)
            })
            .unwrap();
        key_up(&mut visual, key);
        assert_eq!(observed.opens.get(), before + 1);
        assert_focus(&mut visual, "Document menu", Some(ITEMS[0]));
        visual.dispatch_action(CancelMenu);
    }
}

#[open_gpui::test]
fn public_menu_accept_action_does_not_reopen_trigger_when_keyup_returns(cx: &mut TestAppContext) {
    let (_, mut visual, _, observed) = open_consumer(cx);
    for (index, key) in ["enter", "space"].into_iter().enumerate() {
        press_keys(&mut visual, "enter down");
        assert_eq!(observed.opens.get(), index + 1);
        assert_focus(&mut visual, "Document menu", Some(ITEMS[index + 1]));
        key_down(&mut visual, key);
        assert_eq!(
            observed.accepts.get(),
            index + 1,
            "real keymap action runs on keydown"
        );
        assert_eq!(observed.last_accepted.get(), Some(index + 1));
        assert!(visual.debug_selector_is_focused("composite:trigger"));
        assert_focus(&mut visual, "Document menu trigger", None);
        key_up(&mut visual, key);
        assert_eq!(
            observed.opens.get(),
            index + 1,
            "the returned keyup must not reopen the menu"
        );
        assert_eq!(observed.accepts.get(), index + 1);
        assert_focus(&mut visual, "Document menu trigger", None);
    }
}

#[open_gpui::test]
fn public_composite_root_replacement_and_close_release_exact_weak_owners(cx: &mut TestAppContext) {
    let (handle, mut visual, source, observed) = open_consumer(cx);
    let weak_root = handle.entity(&visual).unwrap().downgrade();
    press_keys(&mut visual, "enter down");
    assert_focus(&mut visual, "Document menu", Some(ITEMS[1]));
    visual.update(|window, cx| window.replace_root(cx, |_, _| Empty));
    visual.run_until_parked();
    assert!(weak_root.upgrade().is_none());
    assert_eq!(observed.releases.get(), 1);
    let renders = observed.renders.get();
    source.update(&mut visual, |source, cx| {
        source.revision += 1;
        cx.notify();
    });
    visual.run_until_parked();
    assert_eq!(
        observed.renders.get(),
        renders,
        "the removed root must not observe its source"
    );

    let replacement = visual.update(|window, app| {
        window.replace_root(app, |_, cx| {
            CompositeMenu::new(source.clone(), Rc::clone(&observed), cx)
        })
    });
    let weak_replacement = replacement.downgrade();
    visual.update(|window, app| {
        replacement.update(app, |view, cx| view.trigger.focus(window, cx));
    });
    press_keys(&mut visual, "enter");
    assert_focus(&mut visual, "Document menu", Some(ITEMS[1]));
    drop(replacement);
    assert_eq!(observed.releases.get(), 1);
    assert!(visual.simulate_close());
    assert!(weak_replacement.upgrade().is_none());
    assert_eq!(observed.releases.get(), 2);
    let renders = observed.renders.get();
    source.update(&mut visual, |source, cx| {
        source.revision += 1;
        cx.notify();
    });
    visual.run_until_parked();
    assert_eq!(observed.renders.get(), renders);
    assert_eq!(observed.releases.get(), 2);
}

struct TabGroups {
    containers: [FocusHandle; 2],
    children: [[FocusHandle; 2]; 2],
}

impl Render for TabGroups {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("tab-groups")
            .key_context("CompositeTabs")
            .on_action(|_: &AdvanceTab, window, cx| window.focus_next(cx))
            .size_full()
            .flex()
            .flex_col();
        for group in 0..2 {
            let mut container = div()
                .id(("tab-group", group))
                .track_focus(&self.containers[group])
                .tab_group()
                .tab_stop(false)
                .flex()
                .flex_col();
            for child in 0..2 {
                container = container.child(
                    div()
                        .id(("tab-child", child))
                        .debug_selector(move || format!("composite-tabs:{group}:{child}"))
                        .track_focus(&self.children[group][child])
                        .tab_index(0)
                        .w(px(80.0))
                        .h(px(24.0)),
                );
            }
            root = root.child(container);
        }
        root
    }
}

#[open_gpui::test]
fn public_tab_from_second_non_tab_stop_container_enters_its_own_first_child(
    cx: &mut TestAppContext,
) {
    cx.update(|app| {
        app.bind_keys([KeyBinding::new("tab", AdvanceTab, Some("CompositeTabs"))]);
    });
    let handle = cx.open_window(size(px(200.0), px(160.0)), |_, cx| TabGroups {
        containers: std::array::from_fn(|_| cx.focus_handle()),
        children: std::array::from_fn(|_| std::array::from_fn(|_| cx.focus_handle())),
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    handle
        .update(&mut visual, |view, window, cx| {
            view.containers[1].focus(window, cx)
        })
        .unwrap();
    press_keys(&mut visual, "tab");
    assert!(visual.debug_selector_is_focused("composite-tabs:1:0"));
    assert!(!visual.debug_selector_is_focused("composite-tabs:0:0"));
    press_keys(&mut visual, "tab");
    assert!(visual.debug_selector_is_focused("composite-tabs:1:1"));
}
