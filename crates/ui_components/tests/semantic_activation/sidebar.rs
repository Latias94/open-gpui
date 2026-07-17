use super::*;

use open_gpui_ui_components::sidebar::SidebarItem;
use open_gpui_ui_components::{Sidebar, SidebarActivation, SidebarCollapseMode, SidebarSection};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedSidebarActivation {
    value: String,
    selected: bool,
    source: ActivationSource,
}

impl ObservedSidebarActivation {
    fn new(activation: &SidebarActivation, input: open_gpui_ui_components::Activation) -> Self {
        Self {
            value: activation.value().to_owned(),
            selected: activation.selected(),
            source: input.source(),
        }
    }
}

fn sidebar_node<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    let id = node_with_label(update, label);
    let node = update
        .nodes
        .iter()
        .find_map(|(candidate, node)| (*candidate == id).then_some(node))
        .expect("labelled sidebar item should exist in the final accessibility tree");
    (id, node)
}

#[open_gpui::test]
fn sidebar_routes_every_activation_source_through_one_controlled_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<ObservedSidebarActivation>>>,
        details_handle: ActivationHandle,
        disabled_handle: ActivationHandle,
        duplicate_handle: ActivationHandle,
        globally_disabled_handle: ActivationHandle,
        offcanvas_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();

            div()
                .child(
                    Sidebar::new("semantic-sidebar", "Primary navigation")
                        .selected("overview")
                        .default_focused("details")
                        .section(
                            SidebarSection::new("main", "Main")
                                .item(SidebarItem::new("overview", "Overview"))
                                .item(SidebarItem::new("details", "Details"))
                                .item(SidebarItem::new("disabled", "Disabled item").disabled(true))
                                .item(SidebarItem::new("duplicate", "Duplicate first"))
                                .item(SidebarItem::new("duplicate", "Duplicate second")),
                        )
                        .activation_handle("details", &self.details_handle)
                        .activation_handle("disabled", &self.disabled_handle)
                        .activation_handle("duplicate", &self.duplicate_handle)
                        .on_activate(move |activation, input, _, _| {
                            activations
                                .borrow_mut()
                                .push(ObservedSidebarActivation::new(&activation, input));
                        }),
                )
                .child(
                    Sidebar::new("disabled-sidebar", "Disabled navigation")
                        .disabled(true)
                        .section(
                            SidebarSection::new("disabled", "Disabled")
                                .item(SidebarItem::new("managed", "Globally disabled item")),
                        )
                        .activation_handle("managed", &self.globally_disabled_handle),
                )
                .child(
                    Sidebar::new("offcanvas-sidebar", "Offcanvas navigation")
                        .collapse_mode(SidebarCollapseMode::Offcanvas)
                        .collapsed(true)
                        .section(
                            SidebarSection::new("offcanvas", "Offcanvas")
                                .item(SidebarItem::new("hidden", "Offcanvas item")),
                        )
                        .activation_handle("hidden", &self.offcanvas_handle),
                )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let details_handle = ActivationHandle::new();
    let disabled_handle = ActivationHandle::new();
    let duplicate_handle = ActivationHandle::new();
    let globally_disabled_handle = ActivationHandle::new();
    let offcanvas_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        details_handle: details_handle.clone(),
        disabled_handle: disabled_handle.clone(),
        duplicate_handle: duplicate_handle.clone(),
        globally_disabled_handle: globally_disabled_handle.clone(),
        offcanvas_handle: offcanvas_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("sidebar should publish a final accessibility tree");
    let (details_node, details) = sidebar_node(&initial, "Details");
    let (_, overview) = sidebar_node(&initial, "Overview");
    let (_, disabled) = sidebar_node(&initial, "Disabled item");
    let (_, globally_disabled) = sidebar_node(&initial, "Globally disabled item");
    let (duplicate_first_id, duplicate_first) = sidebar_node(&initial, "Duplicate first");
    let (duplicate_second_id, duplicate_second) = sidebar_node(&initial, "Duplicate second");

    assert_eq!(details.role(), accesskit::Role::Button);
    assert_eq!(overview.is_selected(), Some(true));
    assert_eq!(details.is_selected(), Some(false));
    for disabled in [disabled, globally_disabled] {
        assert!(!disabled.supports_action(accesskit::Action::Click));
        assert!(!disabled.supports_action(accesskit::Action::Focus));
    }
    assert!(
        !initial
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Offcanvas item")),
        "offcanvas items must not leak into the final accessibility tree"
    );
    assert_ne!(duplicate_first_id, duplicate_second_id);
    for duplicate in [duplicate_first, duplicate_second] {
        assert!(duplicate.is_disabled());
        assert!(!duplicate.supports_action(accesskit::Action::Click));
        assert!(!duplicate.supports_action(accesskit::Action::Focus));
    }
    for prefix in [
        "sidebar:semantic-sidebar:duplicate-item:3:duplicate",
        "sidebar:semantic-sidebar:duplicate-item:4:duplicate",
    ] {
        let selector = sole_debug_selector_with_prefix(cx, prefix);
        assert!(
            cx.debug_bounds(&selector).is_some(),
            "duplicate sidebar item should remain visible as `{selector}`"
        );
    }

    let details_bounds = cx
        .debug_bounds("sidebar:semantic-sidebar:item:details")
        .expect("details item should expose a stable selector");
    cx.simulate_click(details_bounds.center(), Modifiers::none());
    assert_eq!(activations.borrow().len(), 1);
    assert!(cx.debug_selector_is_focused("sidebar:semantic-sidebar:item:details"));

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert!(!enter_down.default_prevented());
    assert_eq!(activations.borrow().len(), 1, "Enter activates on key-up");
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert!(!enter_up.default_prevented());
    assert_eq!(activations.borrow().len(), 2);

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(activations.borrow().len(), 2, "Space activates on key-up");
    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(activations.borrow().len(), 3);

    let repeat_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), true));
    let repeat_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(repeat_down.propagated());
    assert!(repeat_up.propagated());
    assert_eq!(activations.borrow().len(), 3, "unarmed repeats are ignored");

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", modified));
    assert!(modified_down.propagated());
    assert!(modified_up.propagated());
    assert_eq!(activations.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, details_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            details_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            duplicate_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            globally_disabled_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        assert_eq!(
            offcanvas_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
        window.draw(cx).clear();
    });

    let observed = activations.borrow();
    assert_eq!(observed.len(), 5);
    for (activation, source) in observed.iter().zip([
        ActivationSource::Pointer,
        ActivationSource::Keyboard(ActivationKey::Enter),
        ActivationSource::Keyboard(ActivationKey::Space),
        ActivationSource::Accessibility,
        ActivationSource::Programmatic,
    ]) {
        assert_eq!(activation.value, "details");
        assert!(
            !activation.selected,
            "caller-owned selection remains unchanged"
        );
        assert_eq!(activation.source, source);
    }
    drop(observed);

    let controlled = cx
        .latest_accessibility_tree_update()
        .expect("controlled sidebar state should remain published after activation");
    assert_eq!(
        sidebar_node(&controlled, "Overview").1.is_selected(),
        Some(true)
    );
    assert_eq!(
        sidebar_node(&controlled, "Details").1.is_selected(),
        Some(false)
    );
}

#[open_gpui::test]
fn sidebar_item_handler_overrides_sidebar_fallback_for_every_entry_point(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        item_activations: Rc<RefCell<Vec<ObservedSidebarActivation>>>,
        sidebar_activations: Rc<RefCell<Vec<ObservedSidebarActivation>>>,
        handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item_activations = self.item_activations.clone();
            let sidebar_activations = self.sidebar_activations.clone();

            Sidebar::new("override-sidebar", "Override navigation")
                .default_focused("settings")
                .section(SidebarSection::new("main", "Main").item(
                    SidebarItem::new("settings", "Settings").on_activate(
                        move |activation, input, _, _| {
                            item_activations
                                .borrow_mut()
                                .push(ObservedSidebarActivation::new(&activation, input));
                        },
                    ),
                ))
                .activation_handle("settings", &self.handle)
                .on_activate(move |activation, input, _, _| {
                    sidebar_activations
                        .borrow_mut()
                        .push(ObservedSidebarActivation::new(&activation, input));
                })
        }
    }

    let item_activations = Rc::new(RefCell::new(Vec::new()));
    let sidebar_activations = Rc::new(RefCell::new(Vec::new()));
    let handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        item_activations: item_activations.clone(),
        sidebar_activations: sidebar_activations.clone(),
        handle: handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("override sidebar should publish a final accessibility tree");
    let settings_node = node_with_label(&update, "Settings");

    let settings_bounds = cx
        .debug_bounds("sidebar:override-sidebar:item:settings")
        .expect("settings item should expose a stable selector");
    cx.simulate_click(settings_bounds.center(), Modifiers::none());

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_down.propagation_stopped());
    assert!(enter_up.propagation_stopped());

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, settings_node,))
    );
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });

    assert_eq!(
        item_activations
            .borrow()
            .iter()
            .map(|activation| activation.source)
            .collect::<Vec<_>>(),
        [
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Accessibility,
            ActivationSource::Programmatic,
        ]
    );
    assert!(
        sidebar_activations.borrow().is_empty(),
        "an item handler replaces, rather than supplements, the sidebar fallback"
    );
}

#[open_gpui::test]
fn sidebar_accesskit_focus_adopts_the_roving_tab_stop_after_redraw(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Sidebar::new("focus-sync-sidebar", "Focus sync")
                .default_focused("a")
                .section(
                    SidebarSection::new("main", "Main")
                        .item(SidebarItem::new("a", "Alpha"))
                        .item(SidebarItem::new("b", "Beta")),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("focus-sync sidebar should publish a final accessibility tree");
    let beta = node_with_label(&initial, "Beta");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, beta)));

    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_selector_is_focused("sidebar:focus-sync-sidebar:item:b"));
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("AccessKit focus should reach the final tree")
            .focus,
        beta
    );

    cx.update(|window, cx| {
        window.focus_next(cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();

    assert!(
        cx.debug_selector_is_focused("sidebar:focus-sync-sidebar:item:b"),
        "the physically focused item must become the sole roving tab stop"
    );
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("tab traversal focus should reach the final tree")
            .focus,
        beta
    );
}

#[open_gpui::test]
fn sidebar_cross_section_roving_keys_publish_final_accessibility_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Sidebar::new("cross-section-sidebar", "Cross-section navigation")
                .default_focused("alpha")
                .section(
                    SidebarSection::new("first", "First")
                        .item(SidebarItem::new("alpha", "Alpha"))
                        .item(SidebarItem::new("blocked", "Blocked").disabled(true)),
                )
                .section(
                    SidebarSection::new("second", "Second")
                        .item(SidebarItem::new("gamma", "Gamma"))
                        .item(SidebarItem::new("omega", "Omega")),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());

    let initial = cx
        .latest_accessibility_tree_update()
        .expect("cross-section sidebar should publish a final accessibility tree");
    let alpha = node_with_label(&initial, "Alpha");
    let gamma = node_with_label(&initial, "Gamma");
    let omega = node_with_label(&initial, "Omega");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, alpha,)));

    for (key, expected_node, expected_selector) in [
        ("down", gamma, "sidebar:cross-section-sidebar:item:gamma"),
        ("up", alpha, "sidebar:cross-section-sidebar:item:alpha"),
        ("end", omega, "sidebar:cross-section-sidebar:item:omega"),
        ("home", alpha, "sidebar:cross-section-sidebar:item:alpha"),
    ] {
        let dispatch =
            cx.simulate_event_with_dispatch_snapshot(key_down(key, Modifiers::none(), false));
        assert!(
            dispatch.propagation_stopped(),
            "{key} should be owned by Sidebar"
        );
        assert!(!dispatch.default_prevented());
        cx.run_until_parked();

        assert!(cx.debug_selector_is_focused(expected_selector));
        assert_eq!(
            cx.latest_accessibility_tree_update()
                .expect("roving focus should reach the final accessibility tree")
                .focus,
            expected_node
        );
    }
}

#[open_gpui::test]
fn sidebar_reentrant_activation_keeps_the_newest_focus_claim(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        trace: Rc<RefCell<Vec<String>>>,
        first_handle: ActivationHandle,
        second_handle: ActivationHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let trace = self.trace.clone();
            let second_handle = self.second_handle.clone();
            Sidebar::new("reentrant-semantic-sidebar", "Reentrant navigation")
                .default_focused("first")
                .section(
                    SidebarSection::new("main", "Main")
                        .item(SidebarItem::new("first", "First"))
                        .item(SidebarItem::new("second", "Second")),
                )
                .activation_handle("first", &self.first_handle)
                .activation_handle("second", &self.second_handle)
                .on_activate(move |activation, _, window, cx| {
                    trace.borrow_mut().push(activation.value().to_owned());
                    if activation.value() == "first" {
                        assert_eq!(
                            second_handle.request(window, cx),
                            ActivationRequestResult::Dispatched
                        );
                    }
                })
        }
    }

    let trace = Rc::new(RefCell::new(Vec::new()));
    let first_handle = ActivationHandle::new();
    let second_handle = ActivationHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        trace: trace.clone(),
        first_handle: first_handle.clone(),
        second_handle: second_handle.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("reentrant sidebar should publish a final accessibility tree");
    let second = node_with_label(&initial, "Second");

    cx.update(|window, cx| {
        assert_eq!(
            first_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });

    assert_eq!(trace.borrow().as_slice(), &["first", "second"]);
    assert!(cx.debug_selector_is_focused("sidebar:reentrant-semantic-sidebar:item:second"));
    assert_eq!(
        cx.latest_accessibility_tree_update()
            .expect("reentrant focus should reach the final accessibility tree")
            .focus,
        second
    );
}

#[open_gpui::test]
fn sidebar_programmatic_handle_tracks_offcanvas_and_item_lifecycle(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        collapsed: bool,
        show_target: bool,
        handle: ActivationHandle,
        activations: Rc<RefCell<Vec<ActivationSource>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section =
                SidebarSection::new("main", "Main").item(SidebarItem::new("anchor", "Anchor"));
            let section = if self.show_target {
                section.item(SidebarItem::new("target", "Target"))
            } else {
                section
            };
            let activations = self.activations.clone();

            Sidebar::new("lifecycle-sidebar", "Lifecycle navigation")
                .collapse_mode(SidebarCollapseMode::Offcanvas)
                .collapsed(self.collapsed)
                .section(section)
                .activation_handle("target", &self.handle)
                .on_activate(move |_, input, _, _| {
                    activations.borrow_mut().push(input.source());
                })
        }
    }

    let handle = ActivationHandle::new();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        collapsed: false,
        show_target: true,
        handle: handle.clone(),
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    assert!(
        cx.latest_accessibility_tree_update()
            .expect("expanded sidebar should publish a final accessibility tree")
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Target"))
    );
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });

    view.update(cx, |probe, cx| {
        probe.collapsed = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert!(
        !cx.latest_accessibility_tree_update()
            .expect("offcanvas sidebar should publish a final accessibility tree")
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Target"))
    );
    cx.update(|window, cx| {
        assert_eq!(handle.request(window, cx), ActivationRequestResult::Blocked);
    });

    view.update(cx, |probe, cx| {
        probe.collapsed = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });

    view.update(cx, |probe, cx| {
        probe.show_target = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    let removed = cx
        .latest_accessibility_tree_update()
        .expect("removed item should leave the final accessibility tree");
    assert!(
        !removed
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Target"))
    );
    let anchor = node_with_label(&removed, "Anchor");
    assert!(cx.debug_selector_is_focused("sidebar:lifecycle-sidebar:item:anchor"));
    assert_eq!(removed.focus, anchor);
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });

    view.update(cx, |probe, cx| {
        probe.show_target = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert!(
        cx.latest_accessibility_tree_update()
            .expect("reappearing item should return to the final accessibility tree")
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Target"))
    );
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Programmatic,
            ActivationSource::Programmatic,
            ActivationSource::Programmatic,
        ]
    );
}

#[open_gpui::test]
fn sidebar_section_identity_is_collision_free_for_duplicate_and_reserved_values(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Sidebar::new("identity:sidebar", "Identity navigation")
                .section(
                    SidebarSection::new("shared", "Shared first")
                        .item(SidebarItem::new("alpha", "Alpha")),
                )
                .section(
                    SidebarSection::new("shared", "Shared second")
                        .item(SidebarItem::new("beta", "Beta")),
                )
                .section(
                    SidebarSection::new("shared:occurrence:1", "Reserved section")
                        .item(SidebarItem::new("gamma:child", "Gamma")),
                )
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| Probe);
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("identity sidebar should publish a final accessibility tree");
    let ids = [
        node_with_label(&update, "Shared first"),
        node_with_label(&update, "Shared second"),
        node_with_label(&update, "Reserved section"),
    ];
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[0], ids[2]);
    assert_ne!(ids[1], ids[2]);

    for prefix in [
        "sidebar:identity%3Asidebar:duplicate-section:0:shared",
        "sidebar:identity%3Asidebar:duplicate-section:1:shared",
    ] {
        let selector = sole_debug_selector_with_prefix(cx, prefix);
        assert!(
            cx.debug_bounds(&selector).is_some(),
            "section identity should expose the collision-free selector `{selector}`"
        );
    }
    for selector in [
        "sidebar:identity%3Asidebar:section:shared%3Aoccurrence%3A1",
        "sidebar:identity%3Asidebar:item:gamma%3Achild",
    ] {
        assert!(cx.debug_bounds(selector).is_some());
    }
}

#[open_gpui::test]
fn sidebar_duplicate_reorder_invalidates_snapshot_local_identity(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        reversed: bool,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let alpha = SidebarItem::new("duplicate", "Duplicate")
                .accessibility_description("Alpha identity");
            let beta = SidebarItem::new("duplicate", "Duplicate")
                .accessibility_description("Beta identity");
            let section = SidebarSection::new("main", "Main");
            let section = if self.reversed {
                section.item(beta).item(alpha)
            } else {
                section.item(alpha).item(beta)
            };

            Sidebar::new("snapshot-reorder-sidebar", "Snapshot reorder navigation").section(section)
        }
    }

    let (view, cx) = cx.add_window_view(|_, _| Probe { reversed: false });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());
    let initial = cx
        .latest_accessibility_tree_update()
        .expect("initial duplicate sidebar should publish a final tree");
    let old_nodes = [
        node_with_label(&initial, "Duplicate, Alpha identity"),
        node_with_label(&initial, "Duplicate, Beta identity"),
    ];
    let old_selectors =
        cx.debug_selectors_with_prefix("sidebar:snapshot-reorder-sidebar:duplicate-item:");
    assert_eq!(old_selectors.len(), 2);

    view.update(cx, |probe, cx| {
        probe.reversed = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let reordered = cx
        .latest_accessibility_tree_update()
        .expect("reordered duplicate sidebar should publish a final tree");
    let new_nodes = [
        node_with_label(&reordered, "Duplicate, Alpha identity"),
        node_with_label(&reordered, "Duplicate, Beta identity"),
    ];
    assert!(old_nodes.iter().all(|old| !new_nodes.contains(old)));
    assert!(
        old_nodes
            .iter()
            .all(|old| { !reordered.nodes.iter().any(|(current, _)| current == old) })
    );

    let new_selectors =
        cx.debug_selectors_with_prefix("sidebar:snapshot-reorder-sidebar:duplicate-item:");
    assert_eq!(new_selectors.len(), 2);
    assert!(
        old_selectors
            .iter()
            .all(|old| !new_selectors.contains(old) && cx.debug_bounds(old).is_none())
    );
}

#[open_gpui::test]
fn sidebar_selectors_and_handlers_cannot_collide_across_component_ids(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        first: Rc<RefCell<Vec<String>>>,
        second: Rc<RefCell<Vec<String>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let first = self.first.clone();
            let second = self.second.clone();
            div()
                .child(
                    Sidebar::new("a", "Duplicate navigation")
                        .section(
                            SidebarSection::new("main", "Main")
                                .item(SidebarItem::new("item:x", "Duplicate first"))
                                .item(SidebarItem::new("item:x", "Duplicate second")),
                        )
                        .on_activate(move |activation, _, _, _| {
                            first.borrow_mut().push(activation.value().to_owned());
                        }),
                )
                .child(
                    Sidebar::new("a:duplicate-item:1", "Unique navigation")
                        .section(
                            SidebarSection::new("main", "Main")
                                .item(SidebarItem::new("x", "Unique item")),
                        )
                        .on_activate(move |activation, _, _, _| {
                            second.borrow_mut().push(activation.value().to_owned());
                        }),
                )
                .child(
                    Sidebar::new("1", "String id navigation").section(
                        SidebarSection::new("main", "Main")
                            .item(SidebarItem::new("string", "String id item")),
                    ),
                )
                .child(
                    Sidebar::new(1usize, "Integer id navigation").section(
                        SidebarSection::new("main", "Main")
                            .item(SidebarItem::new("integer", "Integer id item")),
                    ),
                )
        }
    }

    let first = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| Probe {
        first: first.clone(),
        second: second.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let duplicate = sole_debug_selector_with_prefix(cx, "sidebar:a:duplicate-item:1:item%3Ax");
    let unique = "sidebar:a%3Aduplicate-item%3A1:item:x";
    for selector in [
        duplicate.as_str(),
        unique,
        "sidebar:1:item:string",
        "sidebar:%00i1:item:integer",
        "scroll-area:sidebar:1:scroll",
        "scroll-area:sidebar:%00i1:scroll",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "component identity should expose the collision-free selector `{selector}`"
        );
    }

    let duplicate_bounds = cx
        .debug_bounds(&duplicate)
        .expect("duplicate item should remain rendered for diagnostics");
    cx.simulate_click(duplicate_bounds.center(), Modifiers::none());
    assert!(first.borrow().is_empty());
    assert!(second.borrow().is_empty());

    let unique_bounds = cx
        .debug_bounds(unique)
        .expect("unique item should expose its own selector");
    cx.simulate_click(unique_bounds.center(), Modifiers::none());
    assert!(first.borrow().is_empty());
    assert_eq!(second.borrow().as_slice(), &["x"]);
}

#[open_gpui::test]
fn sidebar_duplicate_redraw_repairs_owned_focus_without_stealing_external_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        duplicate_middle_value: bool,
        middle_handle: ActivationHandle,
        outside_focus: open_gpui::FocusHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let section =
                SidebarSection::new("main", "Main").item(SidebarItem::new("left", "Left"));
            let section = if self.duplicate_middle_value {
                section
                    .item(SidebarItem::new("middle-b", "Middle first"))
                    .item(SidebarItem::new("middle-b", "Middle second"))
            } else {
                section
                    .item(SidebarItem::new("middle-a", "Middle first"))
                    .item(SidebarItem::new("middle-b", "Middle second"))
            };
            let section = section
                .item(SidebarItem::new(
                    "middle-b-occurrence-1",
                    "Unique hyphen occurrence",
                ))
                .item(SidebarItem::new(
                    "middle-b:occurrence:1",
                    "Unique colon occurrence",
                ))
                .item(SidebarItem::new("right", "Right"));

            div()
                .child(
                    Sidebar::new("duplicate-redraw-sidebar", "Duplicate redraw navigation")
                        .default_focused("left")
                        .section(section)
                        .activation_handle("middle-b", &self.middle_handle),
                )
                .child(
                    div()
                        .id("sidebar-outside-focus")
                        .debug_selector(|| "sidebar-outside-focus".to_owned())
                        .role(accesskit::Role::Button)
                        .aria_label("Outside focus")
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.outside_focus)
                        .child("Outside focus"),
                )
        }
    }

    let middle_handle = ActivationHandle::new();
    let (view, cx) = cx.add_window_view(|_, cx| Probe {
        duplicate_middle_value: false,
        middle_handle: middle_handle.clone(),
        outside_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.activate_accessibility());

    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        window.draw(cx).clear();
    });
    assert!(cx.debug_selector_is_focused("sidebar:duplicate-redraw-sidebar:item:middle-b"));

    view.update(cx, |probe, cx| {
        probe.duplicate_middle_value = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    let duplicate_update = cx
        .latest_accessibility_tree_update()
        .expect("duplicate redraw should publish a final accessibility tree");
    let left_node = node_with_label(&duplicate_update, "Left");
    let (first_duplicate_id, first_duplicate) = sidebar_node(&duplicate_update, "Middle first");
    let (second_duplicate_id, second_duplicate) = sidebar_node(&duplicate_update, "Middle second");
    let unique_hyphen_id = node_with_label(&duplicate_update, "Unique hyphen occurrence");
    let unique_colon_id = node_with_label(&duplicate_update, "Unique colon occurrence");

    assert_ne!(first_duplicate_id, second_duplicate_id);
    assert_ne!(first_duplicate_id, unique_hyphen_id);
    assert_ne!(first_duplicate_id, unique_colon_id);
    assert_ne!(second_duplicate_id, unique_hyphen_id);
    assert_ne!(second_duplicate_id, unique_colon_id);
    for duplicate in [first_duplicate, second_duplicate] {
        assert!(duplicate.is_disabled());
        assert!(!duplicate.supports_action(accesskit::Action::Click));
        assert!(!duplicate.supports_action(accesskit::Action::Focus));
    }
    for prefix in [
        "sidebar:duplicate-redraw-sidebar:duplicate-item:1:middle-b",
        "sidebar:duplicate-redraw-sidebar:duplicate-item:2:middle-b",
    ] {
        let selector = sole_debug_selector_with_prefix(cx, prefix);
        assert!(
            cx.debug_bounds(&selector).is_some(),
            "sidebar identity namespaces should remain disjoint for `{selector}`"
        );
    }
    for selector in [
        "sidebar:duplicate-redraw-sidebar:item:middle-b-occurrence-1",
        "sidebar:duplicate-redraw-sidebar:item:middle-b%3Aoccurrence%3A1",
    ] {
        assert!(cx.debug_bounds(selector).is_some());
    }
    assert!(cx.debug_selector_is_focused("sidebar:duplicate-redraw-sidebar:item:left"));
    assert_eq!(duplicate_update.focus, left_node);
    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });

    view.update(cx, |probe, cx| {
        probe.duplicate_middle_value = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        assert_eq!(
            middle_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        let outside_focus = view.read(cx).outside_focus.clone();
        outside_focus.focus(window, cx);
        window.draw(cx).clear();
    });
    cx.run_until_parked();
    assert!(cx.debug_selector_is_focused("sidebar-outside-focus"));
    let outside_node = node_with_label(
        &cx.latest_accessibility_tree_update()
            .expect("external focus should reach the accessibility tree"),
        "Outside focus",
    );

    view.update(cx, |probe, cx| {
        probe.duplicate_middle_value = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert!(cx.debug_selector_is_focused("sidebar-outside-focus"));
    let external_update = cx
        .latest_accessibility_tree_update()
        .expect("duplicate redraw should preserve external accessibility focus");
    assert_eq!(external_update.focus, outside_node);
    assert!(!cx.debug_selector_is_focused("sidebar:duplicate-redraw-sidebar:item:left"));
}
