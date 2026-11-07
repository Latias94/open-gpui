#![cfg(feature = "test-support")]

use open_gpui::{
    AppContext as _, Context, Empty, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, QuitMode, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, TestAppContext, VisualTestContext, Window, WindowHandle,
    accesskit::{Node, NodeId, TreeUpdate},
    div, px, size,
};
use serde_json::Value;
use std::{cell::Cell, collections::HashSet, rc::Rc};

const SPIN_AUTHOR: &str = "settings.editor.font-size.完整标识符 \"α\"\nprimary";
const UPDATED_SPIN_AUTHOR: &str = "settings.editor.font-size.更新标识符 \"β\"\nsecondary";
const SPIN_SHORTCUTS: &str = "Control+Alt+ArrowUp Control+Alt+ArrowDown";
const UPDATED_SPIN_SHORTCUTS: &str = "Meta+Shift+ArrowUp Meta+Shift+ArrowDown";
const MENU_AUTHOR: &str = "settings.editor.font-menu.完整标识符 \"菜单\"";
const UPDATED_MENU_AUTHOR: &str = "settings.editor.font-menu.更新标识符 \"字体\"";
const MENU_SHORTCUTS: &str = "Control+Shift+F10 Alt+M";
const UPDATED_MENU_SHORTCUTS: &str = "Meta+Shift+F10 Alt+Shift+M";
const MENU_ITEMS: [&str; 3] = ["Small text", "Medium text", "Large text"];

#[derive(Default)]
struct Observations {
    renders: Cell<usize>,
    notifications: Cell<usize>,
    subscriptions: Cell<usize>,
    selections: Cell<usize>,
    semantic_frames: Cell<usize>,
    releases: Cell<usize>,
}

impl Observations {
    fn counts(&self) -> [usize; 6] {
        [
            self.renders.get(),
            self.notifications.get(),
            self.subscriptions.get(),
            self.selections.get(),
            self.semantic_frames.get(),
            self.releases.get(),
        ]
    }
}

#[derive(Clone)]
struct PropertySource {
    spin_author: Option<SharedString>,
    spin_shortcuts: Option<SharedString>,
    step: Option<f64>,
    value: f64,
    menu_author: Option<SharedString>,
    menu_shortcuts: Option<SharedString>,
    show_item: bool,
}

impl Default for PropertySource {
    fn default() -> Self {
        Self {
            spin_author: Some(SPIN_AUTHOR.into()),
            spin_shortcuts: Some(SPIN_SHORTCUTS.into()),
            step: Some(0.25),
            value: 14.5,
            menu_author: Some(MENU_AUTHOR.into()),
            menu_shortcuts: Some(MENU_SHORTCUTS.into()),
            show_item: true,
        }
    }
}

struct PropertyConsumer {
    source: Entity<PropertySource>,
    observed: Rc<Observations>,
    _subscription: Subscription,
}

impl Render for PropertyConsumer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        if window.is_a11y_active() {
            self.observed
                .semantic_frames
                .set(self.observed.semantic_frames.get() + 1);
        }
        let source = self.source.read(cx).clone();
        let mut spin = div()
            .id("font-size-element")
            .role(Role::SpinButton)
            .aria_label("Editor font size")
            .aria_numeric_value(source.value)
            .aria_min_numeric_value(6.0)
            .aria_max_numeric_value(72.0)
            .w(px(240.0))
            .h(px(28.0));
        if let Some(author) = source.spin_author {
            spin = spin.accessibility_id(author);
        }
        if let Some(shortcuts) = source.spin_shortcuts {
            spin = spin.aria_keyshortcuts(shortcuts);
        }
        if let Some(step) = source.step {
            spin = spin.aria_numeric_value_step(step);
        }
        let mut menu = div()
            .id("font-menu-element")
            .role(Role::Menu)
            .aria_label("Font size commands")
            .w(px(240.0))
            .min_h(px(28.0));
        if let Some(author) = source.menu_author {
            menu = menu.accessibility_id(author);
        }
        if let Some(shortcuts) = source.menu_shortcuts {
            menu = menu.aria_keyshortcuts(shortcuts);
        }
        if source.show_item {
            menu = menu.child(
                div()
                    .id("apply-font-size-element")
                    .role(Role::MenuItem)
                    .aria_label("Apply font size")
                    .accessibility_id("settings.editor.apply-font-size")
                    .aria_keyshortcuts("Control+Enter")
                    .h(px(28.0)),
            );
        }
        div()
            .id("font-settings-element")
            .role(Role::Group)
            .aria_label("Font settings")
            .size_full()
            .flex()
            .flex_col()
            .child(spin)
            .child(menu)
    }
}

fn open_property_consumer(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<PropertyConsumer>,
    VisualTestContext,
    Entity<PropertySource>,
    Rc<Observations>,
) {
    let source = cx.new(|_| PropertySource::default());
    let observed = Rc::new(Observations::default());
    let handle = cx.open_window(size(px(320.0), px(240.0)), |_, cx| {
        let subscription = cx.observe(&source, |this: &mut PropertyConsumer, _, cx| {
            this.observed
                .notifications
                .set(this.observed.notifications.get() + 1);
            cx.notify();
        });
        observed.subscriptions.set(1);
        PropertyConsumer {
            source: source.clone(),
            observed: Rc::clone(&observed),
            _subscription: subscription,
        }
    });
    let visual = VisualTestContext::from_window(handle.into(), cx);
    visual.run_until_parked();
    (handle, visual, source, observed)
}

fn labelled<'a>(tree: &'a TreeUpdate, label: &str) -> (NodeId, &'a Node) {
    let mut matches = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label));
    let (id, node) = matches
        .next()
        .unwrap_or_else(|| panic!("missing node labelled {label:?}"));
    assert!(matches.next().is_none(), "duplicate label {label:?}");
    (*id, node)
}

fn exported_node(json: &Value, id: NodeId) -> (&str, &Value) {
    let raw = id.0.to_string();
    json["nodes"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, node)| node["accesskit_id"].as_str() == Some(raw.as_str()))
        .map(|(key, node)| (key.as_str(), node))
        .unwrap_or_else(|| panic!("missing exported node {id:?}"))
}

fn published_snapshot(visual: &mut VisualTestContext) -> (TreeUpdate, Value) {
    visual.run_until_parked();
    let tree = visual
        .latest_accessibility_tree_update()
        .expect("accepted platform accessibility tree");
    let (json, accepted) = visual.update(|window, _| {
        assert!(window.is_a11y_enabled());
        assert!(window.is_a11y_active());
        (
            serde_json::from_str::<Value>(
                &window.debug_a11y_tree_json().expect("accepted raw tree"),
            )
            .unwrap(),
            window
                .presentation_facts()
                .frame_accepted_generation
                .expect("accepted frame generation"),
        )
    });
    assert_eq!(json["frame"]["frame_generation"].as_u64(), Some(accepted));
    assert_eq!(json["nodes"].as_object().unwrap().len(), tree.nodes.len());
    let ids = tree.nodes.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
    assert_eq!(ids.len(), tree.nodes.len());
    assert!(ids.contains(&tree.focus));
    let root = tree.tree.as_ref().expect("complete tree").root;
    assert_eq!(json["root"], exported_node(&json, root).0);
    for (id, node) in &tree.nodes {
        let children = node
            .children()
            .iter()
            .map(|child| {
                assert!(ids.contains(child), "dangling child of {id:?}");
                exported_node(&json, *child).0
            })
            .collect::<Vec<_>>();
        let exported = exported_node(&json, *id).1;
        if children.is_empty() {
            assert!(exported.get("children").is_none());
        } else {
            assert_eq!(exported["children"], serde_json::json!(children));
        }
    }
    (tree, json)
}

fn assert_properties(
    tree: &TreeUpdate,
    json: &Value,
    label: &str,
    author: Option<&str>,
    shortcuts: Option<&str>,
    step: Option<f64>,
) -> NodeId {
    let (id, node) = labelled(tree, label);
    assert_eq!(node.author_id(), author);
    assert_eq!(node.keyboard_shortcut(), shortcuts);
    assert_eq!(node.numeric_value_step(), step);
    let aria = &exported_node(json, id).1["aria"];
    for (property, expected) in [("author_id", author), ("keyboard_shortcut", shortcuts)] {
        assert_eq!(aria.get(property), expected.map(Value::from).as_ref());
    }
    assert_eq!(
        aria.get("numeric_value_step"),
        step.map(Value::from).as_ref()
    );
    id
}

fn assert_passive_queries(
    visual: &mut VisualTestContext,
    observed: &Observations,
    enabled: bool,
    active: bool,
) {
    visual.run_until_parked();
    let counts = observed.counts();
    let history = visual.accessibility_tree_update_history();
    let scheduled = visual.frame_scheduled();
    let (facts, focus, json) = visual.update(|window, app| {
        (
            window.presentation_facts(),
            window.focused(app),
            window.debug_a11y_tree_json(),
        )
    });
    for _ in 0..3 {
        visual.update(|window, app| {
            assert_eq!(window.is_a11y_enabled(), enabled);
            assert_eq!(window.is_a11y_active(), active);
            assert_eq!(window.presentation_facts(), facts);
            assert_eq!(window.focused(app), focus);
            assert_eq!(window.debug_a11y_tree_json(), json);
        });
    }
    visual.run_until_parked();
    assert_eq!(observed.counts(), counts);
    assert_eq!(visual.accessibility_tree_update_history(), history);
    assert_eq!(visual.frame_scheduled(), scheduled);
    visual.update(|window, app| {
        assert_eq!(window.presentation_facts(), facts);
        assert_eq!(window.focused(app), focus);
        assert_eq!(window.debug_a11y_tree_json(), json);
    });
}

#[open_gpui::test]
fn public_a11y_properties_follow_source_notifications_and_remove_stale_properties(
    cx: &mut TestAppContext,
) {
    let (_, mut visual, source, observed) = open_property_consumer(cx);
    assert!(visual.activate_accessibility());
    let (initial, json) = published_snapshot(&mut visual);
    let spin = assert_properties(
        &initial,
        &json,
        "Editor font size",
        Some(SPIN_AUTHOR),
        Some(SPIN_SHORTCUTS),
        Some(0.25),
    );
    let menu = assert_properties(
        &initial,
        &json,
        "Font size commands",
        Some(MENU_AUTHOR),
        Some(MENU_SHORTCUTS),
        None,
    );
    let item = assert_properties(
        &initial,
        &json,
        "Apply font size",
        Some("settings.editor.apply-font-size"),
        Some("Control+Enter"),
        None,
    );
    assert_eq!(HashSet::from([spin, menu, item]).len(), 3);
    assert_eq!(
        labelled(&initial, "Editor font size").1.role(),
        Role::SpinButton
    );
    assert_eq!(
        labelled(&initial, "Font size commands").1.role(),
        Role::Menu
    );
    assert_eq!(
        labelled(&initial, "Font size commands").1.children(),
        [item]
    );
    #[cfg(debug_assertions)]
    {
        let spin_element = &exported_node(&json, spin).1["element_id"];
        let menu_element = &exported_node(&json, menu).1["element_id"];
        let item_element = &exported_node(&json, item).1["element_id"];
        assert!(spin_element.as_str().unwrap().contains("font-size-element"));
        assert!(menu_element.as_str().unwrap().contains("font-menu-element"));
        assert_eq!(
            HashSet::from([
                spin_element.as_str().unwrap(),
                menu_element.as_str().unwrap(),
                item_element.as_str().unwrap(),
            ])
            .len(),
            3
        );
        assert_ne!(spin_element, SPIN_AUTHOR);
        assert_ne!(menu_element, MENU_AUTHOR);
    }
    let notifications = observed.notifications.get();
    let history = visual.accessibility_tree_update_history().len();
    source.update(&mut visual, |source, cx| {
        source.spin_author = Some(String::from(UPDATED_SPIN_AUTHOR).into());
        source.spin_shortcuts = Some(String::from(UPDATED_SPIN_SHORTCUTS).into());
        source.step = Some(1.5);
        source.value = 19.0;
        source.menu_author = Some(UPDATED_MENU_AUTHOR.into());
        source.menu_shortcuts = Some(UPDATED_MENU_SHORTCUTS.into());
        cx.notify();
    });
    let (updated, updated_json) = published_snapshot(&mut visual);
    assert_eq!(observed.notifications.get(), notifications + 1);
    assert!(visual.accessibility_tree_update_history().len() > history);
    assert_eq!(
        assert_properties(
            &updated,
            &updated_json,
            "Editor font size",
            Some(UPDATED_SPIN_AUTHOR),
            Some(UPDATED_SPIN_SHORTCUTS),
            Some(1.5),
        ),
        spin
    );
    assert_eq!(
        assert_properties(
            &updated,
            &updated_json,
            "Font size commands",
            Some(UPDATED_MENU_AUTHOR),
            Some(UPDATED_MENU_SHORTCUTS),
            None,
        ),
        menu
    );
    assert_eq!(labelled(&updated, "Apply font size").0, item);
    let spin_node = labelled(&updated, "Editor font size").1;
    assert_eq!(spin_node.numeric_value(), Some(19.0));
    assert_eq!(spin_node.min_numeric_value(), Some(6.0));
    assert_eq!(spin_node.max_numeric_value(), Some(72.0));
    assert_eq!(
        exported_node(&updated_json, spin).1["aria"]["numeric_value"],
        19.0
    );
    #[cfg(debug_assertions)]
    for id in [spin, menu] {
        assert_eq!(
            exported_node(&updated_json, id).1["element_id"],
            exported_node(&json, id).1["element_id"]
        );
    }

    source.update(&mut visual, |source, cx| {
        source.spin_author = None;
        source.spin_shortcuts = None;
        source.step = None;
        source.menu_author = None;
        source.menu_shortcuts = None;
        source.show_item = false;
        cx.notify();
    });
    let (removed, removed_json) = published_snapshot(&mut visual);
    assert_eq!(observed.notifications.get(), notifications + 2);
    for (label, id) in [("Editor font size", spin), ("Font size commands", menu)] {
        assert_eq!(
            assert_properties(&removed, &removed_json, label, None, None, None),
            id
        );
    }
    assert!(
        labelled(&removed, "Font size commands")
            .1
            .children()
            .is_empty()
    );
    assert!(removed.nodes.iter().all(|(id, _)| *id != item));
    let item_raw = item.0.to_string();
    assert!(
        removed_json["nodes"]
            .as_object()
            .unwrap()
            .values()
            .all(|node| {
                node["accesskit_id"].as_str() != Some(item_raw.as_str())
                    && node["aria"]["author_id"] != "settings.editor.apply-font-size"
            })
    );
    assert_passive_queries(&mut visual, &observed, true, true);
}

#[open_gpui::test]
fn public_a11y_queries_are_passive_before_and_after_accepted_publication(cx: &mut TestAppContext) {
    let (_, mut visual, _, observed) = open_property_consumer(cx);
    assert!(visual.latest_accessibility_tree_update().is_none());
    assert!(
        visual
            .update(|window, _| window.debug_a11y_tree_json())
            .is_none()
    );
    assert_passive_queries(&mut visual, &observed, true, false);
    assert!(visual.activate_accessibility());
    published_snapshot(&mut visual);
    assert_passive_queries(&mut visual, &observed, true, true);
    assert!(visual.deactivate_accessibility());
    assert!(
        visual
            .update(|window, _| window.debug_a11y_tree_json())
            .is_none()
    );
    assert_passive_queries(&mut visual, &observed, true, false);
}

struct MenuSource {
    selected: Option<usize>,
}

struct ConfiguredMenu {
    source: Entity<MenuSource>,
    focus: FocusHandle,
    selected: Option<usize>,
    observed: Rc<Observations>,
    _subscriptions: Vec<Subscription>,
}

impl ConfiguredMenu {
    fn new(
        source: Entity<MenuSource>,
        observed: Rc<Observations>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let mut subscriptions = Vec::new();
        if window.is_a11y_enabled() {
            subscriptions.push(cx.on_focus_in(&focus, window, |this, _, cx| {
                if this.selected.is_none() {
                    this.select(this.source.read(cx).selected, cx);
                }
            }));
            subscriptions.push(cx.observe_in(&source, window, |this, source, _, cx| {
                this.observed
                    .notifications
                    .set(this.observed.notifications.get() + 1);
                this.select(source.read(cx).selected, cx);
            }));
        }
        observed
            .subscriptions
            .set(observed.subscriptions.get() + subscriptions.len());
        cx.on_release({
            let observed = Rc::clone(&observed);
            move |_, _| observed.releases.set(observed.releases.get() + 1)
        })
        .detach();
        Self {
            source,
            focus,
            selected: None,
            observed,
            _subscriptions: subscriptions,
        }
    }

    fn select(&mut self, selected: Option<usize>, cx: &mut Context<Self>) {
        self.selected = selected;
        self.observed
            .selections
            .set(self.observed.selections.get() + 1);
        cx.notify();
    }
}

impl Render for ConfiguredMenu {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        let active = window.is_a11y_active();
        if active {
            self.observed
                .semantic_frames
                .set(self.observed.semantic_frames.get() + 1);
        }
        let mut menu = div()
            .id("configured-font-menu")
            .role(Role::Menu)
            .aria_label("Configured font menu")
            .accessibility_id("settings.font-menu")
            .aria_keyshortcuts("Alt+F10")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col();
        for (index, label) in MENU_ITEMS.into_iter().enumerate() {
            let mut item = div()
                .id(("configured-font-item", index))
                .role(Role::MenuItem)
                .aria_label(label)
                .w(px(240.0))
                .h(px(28.0));
            if active {
                item = item.aria_selected(self.selected == Some(index));
                if self.selected == Some(index) {
                    item = item.aria_active_descendant();
                }
            }
            menu = menu.child(item);
        }
        menu
    }
}

fn open_configured_menu(
    cx: &mut TestAppContext,
    source: &Entity<MenuSource>,
) -> (
    WindowHandle<ConfiguredMenu>,
    VisualTestContext,
    Rc<Observations>,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let observed = Rc::new(Observations::default());
    let handle = cx.open_window(size(px(320.0), px(240.0)), |window, cx| {
        ConfiguredMenu::new(source.clone(), Rc::clone(&observed), window, cx)
    });
    let visual = VisualTestContext::from_window(handle.into(), cx);
    visual.run_until_parked();
    (handle, visual, observed)
}

fn focus_menu(handle: WindowHandle<ConfiguredMenu>, visual: &mut VisualTestContext) {
    let _activation = visual.update(|window, _| window.activate_window());
    handle
        .update(visual, |menu, window, cx| menu.focus.focus(window, cx))
        .unwrap();
    visual.run_until_parked();
}

fn set_selection(
    source: &Entity<MenuSource>,
    visual: &mut VisualTestContext,
    selected: Option<usize>,
) {
    source.update(visual, |source, cx| {
        source.selected = selected;
        cx.notify();
    });
    visual.run_until_parked();
}

fn assert_menu_selection(
    visual: &mut VisualTestContext,
    selected: Option<usize>,
) -> (Vec<NodeId>, u64) {
    let (tree, json) = published_snapshot(visual);
    let menu = labelled(&tree, "Configured font menu").0;
    assert_eq!(json["gpui_focus"], exported_node(&json, menu).0);
    let items = MENU_ITEMS
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let (id, node) = labelled(&tree, label);
            assert_eq!(node.is_selected(), Some(selected == Some(index)));
            assert_eq!(
                exported_node(&json, id).1["aria"]["selected"],
                selected == Some(index)
            );
            id
        })
        .collect::<Vec<_>>();
    assert_eq!(labelled(&tree, "Configured font menu").1.children(), items);
    match selected {
        Some(index) => {
            assert_eq!(tree.focus, items[index]);
            assert_eq!(
                json["active_descendant_focus"],
                exported_node(&json, items[index]).0
            );
        }
        None => {
            assert_eq!(tree.focus, menu);
            assert!(json["active_descendant_focus"].is_null());
        }
    }
    (
        items,
        json["frame"]["activation_generation"].as_u64().unwrap(),
    )
}

#[open_gpui::test]
fn public_a11y_enabled_composite_observes_before_attachment_across_activation_cycles(
    cx: &mut TestAppContext,
) {
    let source = cx.new(|_| MenuSource { selected: Some(0) });
    let (handle, mut visual, observed) = open_configured_menu(cx, &source);
    assert_eq!(observed.subscriptions.get(), 2);
    assert_eq!(
        handle.read_with(&visual, |menu, _| menu.selected).unwrap(),
        None
    );
    assert_passive_queries(&mut visual, &observed, true, false);
    focus_menu(handle, &mut visual);
    assert_eq!(
        handle.read_with(&visual, |menu, _| menu.selected).unwrap(),
        Some(0)
    );
    assert_eq!(observed.selections.get(), 1);
    set_selection(&source, &mut visual, Some(1));
    assert_eq!(
        handle.read_with(&visual, |menu, _| menu.selected).unwrap(),
        Some(1)
    );
    assert_eq!(observed.notifications.get(), 1);
    assert_eq!(observed.semantic_frames.get(), 0);
    assert!(visual.latest_accessibility_tree_update().is_none());
    assert!(visual.activate_accessibility());
    let (items, mut activation) = assert_menu_selection(&mut visual, Some(1));

    for selected in [Some(2), None, Some(0)] {
        assert!(visual.deactivate_accessibility());
        let history = visual.accessibility_tree_update_history();
        let semantic_frames = observed.semantic_frames.get();
        set_selection(&source, &mut visual, selected);
        assert_eq!(
            handle.read_with(&visual, |menu, _| menu.selected).unwrap(),
            selected
        );
        assert_eq!(observed.semantic_frames.get(), semantic_frames);
        assert_eq!(visual.accessibility_tree_update_history(), history);
        assert!(
            visual
                .update(|window, _| window.debug_a11y_tree_json())
                .is_none()
        );
        assert_passive_queries(&mut visual, &observed, true, false);
        assert!(visual.activate_accessibility());
        let (current_items, current_activation) = assert_menu_selection(&mut visual, selected);
        assert_eq!(current_items, items);
        assert!(current_activation > activation);
        assert!(visual.activate_accessibility());
        let (repeated_items, repeated_activation) = assert_menu_selection(&mut visual, selected);
        assert_eq!(repeated_items, items);
        assert!(repeated_activation > current_activation);
        activation = repeated_activation;
        assert_passive_queries(&mut visual, &observed, true, true);
    }
    assert_eq!(observed.subscriptions.get(), 2);
    assert_eq!(observed.notifications.get(), 4);
}

#[open_gpui::test]
fn public_disable_accessibility_only_affects_subsequent_windows(cx: &mut TestAppContext) {
    let source = cx.new(|_| MenuSource { selected: Some(0) });
    let (enabled_handle, mut enabled, enabled_observed) = open_configured_menu(cx, &source);
    focus_menu(enabled_handle, &mut enabled);
    cx.disable_accessibility();
    assert_passive_queries(&mut enabled, &enabled_observed, true, false);
    assert!(enabled.activate_accessibility());
    assert_menu_selection(&mut enabled, Some(0));
    cx.disable_accessibility();
    assert_passive_queries(&mut enabled, &enabled_observed, true, true);

    let (disabled_handle, mut disabled, disabled_observed) = open_configured_menu(cx, &source);
    focus_menu(disabled_handle, &mut disabled);
    assert_eq!(disabled_observed.subscriptions.get(), 0);
    assert_eq!(disabled_observed.selections.get(), 0);
    assert_eq!(
        disabled_handle
            .read_with(&disabled, |menu, _| menu.selected)
            .unwrap(),
        None
    );
    assert_passive_queries(&mut disabled, &disabled_observed, false, false);
    for _ in 0..3 {
        assert!(!disabled.activate_accessibility());
        assert!(!disabled.deactivate_accessibility());
    }
    let disabled_renders = disabled_observed.renders.get();
    set_selection(&source, &mut enabled, Some(2));
    assert_eq!(
        enabled_handle
            .read_with(&enabled, |menu, _| menu.selected)
            .unwrap(),
        Some(2)
    );
    assert_eq!(enabled_observed.notifications.get(), 1);
    assert_eq!(disabled_observed.renders.get(), disabled_renders);
    assert_eq!(disabled_observed.notifications.get(), 0);
    assert_eq!(disabled_observed.selections.get(), 0);
    assert_eq!(disabled_observed.semantic_frames.get(), 0);
    assert_eq!(
        disabled_handle
            .read_with(&disabled, |menu, _| menu.selected)
            .unwrap(),
        None
    );
    assert!(disabled.accessibility_tree_update_history().is_empty());
    assert!(
        disabled
            .update(|window, _| window.debug_a11y_tree_json())
            .is_none()
    );
    assert_passive_queries(&mut disabled, &disabled_observed, false, false);

    let old_disabled = disabled_handle.entity(&disabled).unwrap().downgrade();
    let replacement = disabled.update(|window, app| {
        window.replace_root(app, |window, cx| {
            ConfiguredMenu::new(source.clone(), Rc::clone(&disabled_observed), window, cx)
        })
    });
    drop(replacement);
    disabled.run_until_parked();
    assert!(old_disabled.upgrade().is_none());
    assert_eq!(disabled_observed.releases.get(), 1);
    focus_menu(disabled_handle, &mut disabled);
    assert_eq!(disabled_observed.subscriptions.get(), 0);
    assert_eq!(
        disabled_handle
            .read_with(&disabled, |menu, _| menu.selected)
            .unwrap(),
        None
    );
    assert!(!disabled.activate_accessibility());
    let disabled_counts = disabled_observed.counts();
    set_selection(&source, &mut enabled, Some(1));
    assert_eq!(disabled_observed.counts(), disabled_counts);
    assert!(disabled.accessibility_tree_update_history().is_empty());
    assert_passive_queries(&mut disabled, &disabled_observed, false, false);
    assert!(enabled.deactivate_accessibility());
    assert!(enabled.activate_accessibility());
    focus_menu(enabled_handle, &mut enabled);
    assert_menu_selection(&mut enabled, Some(1));
}

#[open_gpui::test]
fn public_a11y_composite_rebuild_and_close_release_subscriptions(cx: &mut TestAppContext) {
    let source = cx.new(|_| MenuSource { selected: Some(0) });
    let (handle, mut visual, observed) = open_configured_menu(cx, &source);
    focus_menu(handle, &mut visual);
    assert!(visual.activate_accessibility());
    assert_menu_selection(&mut visual, Some(0));
    let old_root = handle.entity(&visual).unwrap().downgrade();
    visual.update(|window, app| {
        window.replace_root(app, |_, _| Empty);
    });
    let (empty, _) = published_snapshot(&mut visual);
    assert_eq!(empty.nodes.len(), 1);
    assert!(empty.nodes[0].1.children().is_empty());
    assert!(old_root.upgrade().is_none());
    assert_eq!(observed.releases.get(), 1);
    let counts = observed.counts();
    set_selection(&source, &mut visual, Some(2));
    assert_eq!(observed.counts(), counts);

    let replacement = visual.update(|window, app| {
        window.replace_root(app, |window, cx| {
            ConfiguredMenu::new(source.clone(), Rc::clone(&observed), window, cx)
        })
    });
    let replacement_weak = replacement.downgrade();
    drop(replacement);
    visual.run_until_parked();
    focus_menu(handle, &mut visual);
    assert_menu_selection(&mut visual, Some(2));
    assert_eq!(observed.subscriptions.get(), 4);
    assert_eq!(observed.releases.get(), 1);
    set_selection(&source, &mut visual, Some(1));
    assert_menu_selection(&mut visual, Some(1));
    assert_eq!(observed.notifications.get(), 1);
    visual.update(|window, app| {
        window.remove_window(app);
        assert!(window.debug_a11y_tree_json().is_none());
    });
    cx.run_until_parked();
    assert!(!cx.windows().contains(&handle.into()));
    assert!(replacement_weak.upgrade().is_none());
    assert_eq!(observed.releases.get(), 2);
    let counts = observed.counts();
    source.update(cx, |source, cx| {
        source.selected = Some(0);
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(observed.counts(), counts);
}
