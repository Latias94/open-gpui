use super::*;

#[open_gpui::test]
fn outside_policies_preserve_exactly_once_intent_and_dispatch_outcomes(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);

    let cases = [
        ("ignore", OutsidePressPolicy::Ignore, false, 0),
        ("consume", OutsidePressPolicy::Consume, true, 0),
        (
            "dismiss-consume",
            OutsidePressPolicy::DismissAndConsume,
            true,
            1,
        ),
        (
            "dismiss-pass-through",
            OutsidePressPolicy::DismissAndPassThrough,
            false,
            1,
        ),
    ];

    for (id, outside_press, consumed, expected_intents) in cases {
        let events = Rc::new(RefCell::new(Vec::new()));
        register_layer(
            cx,
            &view,
            controlled_registration(
                id,
                policy(
                    OverlayLayerKind::NonModalDismissible,
                    OverlayPresence::open(),
                    outside_press,
                ),
                events.clone(),
            ),
        );

        let first = cx.simulate_event_with_dispatch_snapshot(mouse_down(700.0, 450.0));
        let second = cx.simulate_event_with_dispatch_snapshot(mouse_down(700.0, 450.0));
        assert_eq!(first.default_prevented(), consumed, "{id} default outcome");
        assert_eq!(first.propagation_stopped(), consumed, "{id} propagation");
        assert_eq!(
            second.default_prevented(),
            consumed,
            "{id} repeated outcome"
        );
        assert_eq!(
            events.borrow().len(),
            expected_intents,
            "{id} should emit at most one controlled close intent"
        );
        if expected_intents == 1 {
            assert_eq!(events.borrow().as_slice(), &[false]);
        }

        unregister_layer(cx, &view, id);
    }
}

#[open_gpui::test]
fn transformed_and_clipped_surface_inside_region_uses_visible_window_bounds(
    cx: &mut open_gpui::TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    register_layer(
        cx,
        &view,
        controlled_registration(
            "transformed-inside-region",
            policy(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndConsume,
            ),
            events.clone(),
        ),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_surface_transform(
            "transformed-inside-region",
            SubtreeTransform::try_translation(point(px(300.0), px(200.0)))
                .expect("the inside-region translation should be valid"),
        );
        probe.set_surface_clipped("transformed-inside-region");
        cx.notify();
    });
    draw(cx);

    let displayed = cx
        .debug_bounds("window-overlay-runtime:transformed-inside-region:surface")
        .expect("the transformed overlay surface should render");
    let visible_point = point(displayed.origin.x + px(10.0), displayed.center().y);
    let clipped_point = point(displayed.right() - px(10.0), displayed.center().y);
    assert!(displayed.contains(&visible_point));
    assert!(displayed.contains(&clipped_point));
    assert!(visible_point.x < px(600.0));
    assert!(clipped_point.x > px(600.0));

    cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
        position: visible_point,
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    assert!(
        events.borrow().is_empty(),
        "displayed inside geometry must not dispatch an outside close intent"
    );

    cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
        position: clipped_point,
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    assert_eq!(
        events.borrow().as_slice(),
        &[false],
        "geometry outside the effective clip must remain an outside press"
    );
}

#[open_gpui::test]
fn parent_and_child_inside_regions_resolve_one_layer_without_breaking_modal_pass_through(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let parent_events = Rc::new(RefCell::new(Vec::new()));
    let child_events = Rc::new(RefCell::new(Vec::new()));

    register_layer(
        cx,
        &view,
        controlled_registration(
            "parent-modal",
            policy(
                OverlayLayerKind::Modal,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndConsume,
            ),
            parent_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "child-popover",
            policy(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndPassThrough,
            ),
            child_events.clone(),
        )
        .parent("parent-modal"),
    );
    set_inside_region(
        cx,
        &view,
        "parent-modal",
        "parent",
        rect(660.0, 430.0, 120.0, 120.0),
    );
    set_inside_region(
        cx,
        &view,
        "child-popover",
        "child",
        rect(700.0, 470.0, 40.0, 40.0),
    );

    let child_inside = cx.simulate_event_with_dispatch_snapshot(mouse_down(720.0, 490.0));
    assert!(!child_inside.default_prevented());
    assert!(parent_events.borrow().is_empty());
    assert!(child_events.borrow().is_empty());

    let parent_only = cx.simulate_event_with_dispatch_snapshot(mouse_down(680.0, 450.0));
    let repeated_parent_only = cx.simulate_event_with_dispatch_snapshot(mouse_down(680.0, 450.0));
    assert!(
        !parent_only.default_prevented() && !repeated_parent_only.default_prevented(),
        "a child pass-through dismissal inside its modal parent must reach parent content"
    );
    assert_eq!(child_events.borrow().as_slice(), &[false]);
    assert!(parent_events.borrow().is_empty());

    let outside_modal = cx.simulate_event_with_dispatch_snapshot(mouse_down(820.0, 620.0));
    assert!(outside_modal.default_prevented());
    assert!(outside_modal.propagation_stopped());
    assert_eq!(child_events.borrow().as_slice(), &[false]);
    assert!(parent_events.borrow().is_empty());
}

#[open_gpui::test]
fn pass_through_child_inherits_consumption_only_outside_its_nonmodal_ancestor_tree(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let parent_events = Rc::new(RefCell::new(Vec::new()));
    let child_events = Rc::new(RefCell::new(Vec::new()));

    register_layer(
        cx,
        &view,
        controlled_registration(
            "ancestor-menu",
            policy(
                OverlayLayerKind::Menu,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndConsume,
            ),
            parent_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "menu-branch",
            policy(
                OverlayLayerKind::Menu,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndPassThrough,
            ),
            child_events.clone(),
        )
        .parent("ancestor-menu"),
    );
    set_inside_region(
        cx,
        &view,
        "ancestor-menu",
        "root",
        rect(660.0, 430.0, 120.0, 120.0),
    );
    set_inside_region(
        cx,
        &view,
        "menu-branch",
        "branch",
        rect(700.0, 470.0, 40.0, 40.0),
    );

    let ancestor_only = cx.simulate_event_with_dispatch_snapshot(mouse_down(680.0, 450.0));
    assert!(!ancestor_only.default_prevented());
    assert!(!ancestor_only.propagation_stopped());
    assert_eq!(child_events.borrow().as_slice(), &[false]);
    assert!(parent_events.borrow().is_empty());

    let outside_tree = cx.simulate_event_with_dispatch_snapshot(mouse_down(820.0, 620.0));
    assert!(outside_tree.default_prevented());
    assert!(outside_tree.propagation_stopped());
    assert_eq!(
        child_events.borrow().as_slice(),
        &[false],
        "controlled close refusal should suppress duplicate child intents"
    );
    assert!(
        parent_events.borrow().is_empty(),
        "ancestor consumption must not dispatch a second close intent"
    );
}

#[open_gpui::test]
fn allowed_modal_surface_press_keeps_captured_move_and_up_routed_outside_modal(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "captured-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_pointer_capture("captured-modal", true);
        cx.notify();
    });
    settle_focus_claims(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:captured-modal:surface")
        .expect("capturing modal surface should render");
    set_inside_region(cx, &view, "captured-modal", "surface", surface);

    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    let capture = cx.update_window_entity(&view, |probe, _, _| probe.surface_pointer_capture);
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(capture)
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        let binding = probe.binding("captured-modal");
        probe
            .runtime
            .rebind_layer(
                &binding,
                uncontrolled_registration(
                    "captured-modal",
                    OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                )
                .focus_mode(OverlayFocusMode::Modal),
                window,
                cx,
            )
            .expect("an equivalent render-time rebind should preserve the active gesture");
    });
    let right_down = cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        button: MouseButton::Right,
        click_count: 1,
        first_mouse: false,
    });
    let right_up = cx.simulate_event_with_dispatch_snapshot(MouseUpEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        button: MouseButton::Right,
        click_count: 1,
    });
    assert!(!right_down.default_prevented());
    assert!(!right_down.propagation_stopped());
    assert!(!right_up.default_prevented());
    assert!(!right_up.propagation_stopped());
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(capture),
        "a companion button release must not end the initiating capture route"
    );
    let move_dispatch = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: None,
    });
    assert!(!move_dispatch.default_prevented());
    assert!(!move_dispatch.propagation_stopped());
    assert_eq!(
        cx.update(|window, _| window.captured_pointer().map(|capture| capture.handle())),
        Some(capture)
    );
    let up_dispatch = cx.simulate_event_with_dispatch_snapshot(MouseUpEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
    });
    assert!(!up_dispatch.default_prevented());
    assert!(!up_dispatch.propagation_stopped());
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));

    let (surface_events, underlay_events) = cx.update_window_entity(&view, |probe, _, _| {
        (
            probe.surface_pointer_events.borrow().clone(),
            probe.underlay_pointer_events.borrow().clone(),
        )
    });
    assert_eq!(
        surface_events,
        vec![
            ("captured-modal".to_owned(), "down"),
            ("captured-modal".to_owned(), "move"),
            ("captured-modal".to_owned(), "up"),
        ]
    );
    assert!(
        underlay_events.is_empty(),
        "captured press sequence must not retarget to the modal underlay"
    );
}

#[open_gpui::test]
fn captured_owner_unmount_seals_the_old_allowed_route_until_mouse_up(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "unmounted-capture-owner",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_pointer_capture("unmounted-capture-owner", true);
        cx.notify();
    });
    settle_focus_claims(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:unmounted-capture-owner:surface")
        .expect("capturing modal surface should render");
    set_inside_region(cx, &view, "unmounted-capture-owner", "surface", surface);

    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    let captured_move = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(!captured_move.default_prevented());
    assert!(!captured_move.propagation_stopped());

    unregister_layer(cx, &view, "unmounted-capture-owner");
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
    cx.update_window_entity(&view, |probe, _, _| {
        probe.underlay_pointer_events.borrow_mut().clear();
    });

    let stale_move = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    let stale_up = cx.simulate_event_with_dispatch_snapshot(MouseUpEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
    });
    assert!(stale_move.default_prevented());
    assert!(stale_move.propagation_stopped());
    assert!(stale_up.default_prevented());
    assert!(stale_up.propagation_stopped());
    assert!(
        cx.update_window_entity(&view, |probe, _, _| {
            probe.underlay_pointer_events.borrow().is_empty()
        }),
        "an unmounted capture owner must not leak the old gesture to the underlay"
    );

    let fresh_move = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: None,
    });
    assert!(!fresh_move.default_prevented());
    assert!(!fresh_move.propagation_stopped());
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| {
            probe.underlay_pointer_events.borrow().clone()
        }),
        vec!["move"]
    );
}

#[open_gpui::test]
fn unrelated_hidden_registration_preserves_an_existing_captured_route(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "aba-captured-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_pointer_capture("aba-captured-modal", true);
        cx.notify();
    });
    draw(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:aba-captured-modal:surface")
        .expect("capturing modal surface should render");
    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    let hidden = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "aba-hidden-registration",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
        )
        .focus_mode(OverlayFocusMode::None),
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .unregister_layer(&hidden, window, cx)
            .expect("hidden ABA registration should unregister");
        probe.remove_layer("aba-hidden-registration");
        probe.surface_pointer_events.borrow_mut().clear();
        cx.notify();
    });

    let dispatch = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(!dispatch.default_prevented());
    assert!(!dispatch.propagation_stopped());
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));
    let surface_events = cx.update_window_entity(&view, |probe, _, _| {
        probe.surface_pointer_events.borrow().clone()
    });
    assert_eq!(
        surface_events,
        vec![("aba-captured-modal".to_owned(), "move")]
    );
}

#[open_gpui::test]
fn open_modal_registration_aba_invalidates_an_existing_captured_route(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "open-aba-capture-owner",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_pointer_capture("open-aba-capture-owner", true);
        cx.notify();
    });
    draw(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:open-aba-capture-owner:surface")
        .expect("capturing modal surface should render");
    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    let transient = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "open-aba-transient-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .unregister_layer(&transient, window, cx)
            .expect("transient modal should unregister");
        probe.remove_layer("open-aba-transient-modal");
        probe.surface_pointer_events.borrow_mut().clear();
        cx.notify();
    });

    let dispatch = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| {
            probe.surface_pointer_events.borrow().clone()
        }),
        vec![("open-aba-capture-owner".to_owned(), "cancel")]
    );
}

#[open_gpui::test]
fn superseding_modal_cancels_the_previous_capture_owner_exactly_once(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "capture-owner-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_pointer_capture("capture-owner-modal", true);
        cx.notify();
    });
    settle_focus_claims(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:capture-owner-modal:surface")
        .expect("capture owner should render");
    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "superseding-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    let blocked_move = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(blocked_move.default_prevented());
    assert!(blocked_move.propagation_stopped());
    cx.run_until_parked();

    assert!(cx.update(|window, _| window.captured_pointer().is_none()));
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| {
            probe.surface_pointer_events.borrow().clone()
        }),
        vec![
            ("capture-owner-modal".to_owned(), "down"),
            ("capture-owner-modal".to_owned(), "cancel"),
        ]
    );

    cx.simulate_event(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: None,
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| {
            probe
                .surface_pointer_events
                .borrow()
                .iter()
                .filter(|(_, event)| *event == "cancel")
                .count()
        }),
        1
    );
}

#[open_gpui::test]
fn superseding_modal_cancels_an_uncaptured_press_before_its_mouse_up(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Default::default(),
    );
    let modal = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "superseding-uncaptured-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    let stale_up = cx.simulate_event_with_dispatch_snapshot(MouseUpEvent {
        position: point(px(10.0), px(10.0)),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
    });
    assert!(stale_up.default_prevented());
    assert!(stale_up.propagation_stopped());
    cx.run_until_parked();
    assert!(
        !cx.update(|window, cx| window.has_active_pointer_session(cx)),
        "the blocked gesture must settle through terminal cancellation"
    );
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| probe.underlay_clicks.get()),
        0
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .unregister_layer(&modal, window, cx)
            .expect("superseding modal should unregister");
        probe.remove_layer("superseding-uncaptured-modal");
        cx.notify();
    });
    settle_focus_claims(cx);
    cx.simulate_click(point(px(10.0), px(10.0)), Default::default());
    assert_eq!(
        cx.update_window_entity(&view, |probe, _, _| probe.underlay_clicks.get()),
        1,
        "a fresh gesture must activate normally after the canceled press"
    );
}

#[open_gpui::test]
fn window_deactivation_cancels_allowed_gesture_routes_before_reactivation(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "deactivation-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);
    let surface = cx
        .debug_bounds("window-overlay-runtime:deactivation-modal:surface")
        .expect("modal surface should render");
    set_inside_region(cx, &view, "deactivation-modal", "surface", surface);

    cx.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update_window_entity(&view, |probe, _, _| {
        probe.underlay_pointer_events.borrow_mut().clear();
    });

    let dispatch = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(100.0), px(100.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    let underlay_events = cx.update_window_entity(&view, |probe, _, _| {
        probe.underlay_pointer_events.borrow().clone()
    });
    assert!(underlay_events.is_empty());
}

#[open_gpui::test]
fn modal_blocks_a_captured_pointer_when_the_runtime_was_installed_after_capture(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(LateInstalledRuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);

    cx.simulate_mouse_down(
        point(px(24.0), px(24.0)),
        MouseButton::Left,
        Default::default(),
    );
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    cx.update_window_entity(&view, |probe, window, cx| {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        let binding = runtime
            .register_layer(
                uncontrolled_registration(
                    "late-runtime-modal",
                    OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                ),
                window,
                cx,
            )
            .expect("late-installed runtime should register its modal barrier");
        probe.runtime_binding = Some((runtime, binding));
    });

    let dispatch = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
        position: point(px(280.0), px(180.0)),
        modifiers: Default::default(),
        pressed_button: Some(MouseButton::Left),
    });
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
}

#[open_gpui::test]
fn runtime_surface_replays_inside_bounds_when_gpui_reuses_cached_prepaint(
    cx: &mut open_gpui::TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let renders = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let events = events.clone();
        let renders = renders.clone();
        move |window, cx| {
            let runtime = WindowOverlayRuntime::for_window(window, cx);
            let binding = runtime
                .register_layer(
                    controlled_registration(
                        "cached-surface",
                        policy(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                            OutsidePressPolicy::DismissAndConsume,
                        ),
                        events,
                    ),
                    window,
                    cx,
                )
                .expect("cached overlay layer should register");
            let child = cx.new(|_| CachedOverlaySurfaceProbe {
                runtime: runtime.clone(),
                binding: binding.clone(),
                renders,
            });
            CachedOverlaySurfaceRoot {
                runtime,
                binding,
                child,
            }
        }
    });
    draw(cx);
    assert!(renders.get() > 0, "cached surface should render once");
    let surface = rect(240.0, 96.0, 180.0, 28.0);
    let fresh_renders = renders.get();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        renders.get(),
        fresh_renders,
        "cached surface child must not render during journal reuse"
    );

    cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
        position: surface.center(),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    assert!(events.borrow().is_empty());
    let phase = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("cached overlay runtime should remain current")
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == probe.binding.lease().layer_id().as_str())
            .expect("cached overlay layer should remain registered")
            .phase()
    });
    assert_eq!(phase, OverlayLayerPhase::Open);
}

#[open_gpui::test]
fn cached_surface_journal_cannot_refresh_a_replacement_layer_with_the_same_id(
    cx: &mut open_gpui::TestAppContext,
) {
    let original_events = Rc::new(RefCell::new(Vec::new()));
    let replacement_events = Rc::new(RefCell::new(Vec::new()));
    let renders = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let original_events = original_events.clone();
        let renders = renders.clone();
        move |window, cx| {
            let runtime = WindowOverlayRuntime::for_window(window, cx);
            let binding = runtime
                .register_layer(
                    controlled_registration(
                        "cached-surface-aba",
                        policy(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                            OutsidePressPolicy::DismissAndConsume,
                        ),
                        original_events,
                    ),
                    window,
                    cx,
                )
                .expect("original cached overlay layer should register");
            let child = cx.new(|_| CachedOverlaySurfaceProbe {
                runtime: runtime.clone(),
                binding: binding.clone(),
                renders,
            });
            CachedOverlaySurfaceRoot {
                runtime,
                binding,
                child,
            }
        }
    });
    draw(cx);
    let original = cx.update_window_entity(&view, |probe, window, cx| {
        let original = probe.binding.clone();
        probe
            .runtime
            .unregister_layer(&original, window, cx)
            .expect("original cached overlay layer should unregister");
        original
    });
    settle_focus_claims(cx);
    let stale_journal_renders = renders.get();

    let replacement = cx.update_window_entity(&view, {
        let replacement_events = replacement_events.clone();
        move |probe, window, cx| {
            let replacement = probe
                .runtime
                .register_layer(
                    controlled_registration(
                        "cached-surface-aba",
                        policy(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                            OutsidePressPolicy::DismissAndConsume,
                        ),
                        replacement_events,
                    ),
                    window,
                    cx,
                )
                .expect("replacement cached overlay layer should register");
            assert_ne!(original.lease(), replacement.lease());
            probe.binding = replacement.clone();
            cx.notify();
            replacement
        }
    });

    draw(cx);
    assert_eq!(
        renders.get(),
        stale_journal_renders,
        "the replacement frame must replay the stale lease journal instead of rendering again"
    );
    let dispatch = cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
        position: rect(240.0, 96.0, 180.0, 28.0).center(),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    assert!(original_events.borrow().is_empty());
    assert_eq!(replacement_events.borrow().as_slice(), &[false]);
    let phase = cx.update_window_entity(&view, |probe, window, cx| {
        assert_eq!(probe.binding.lease(), replacement.lease());
        probe
            .runtime
            .snapshot(window, cx)
            .expect("replacement overlay runtime should remain current")
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == "cached-surface-aba")
            .expect("replacement overlay layer should remain registered")
            .phase()
    });
    assert_eq!(phase, OverlayLayerPhase::CloseRequested);
}

#[open_gpui::test]
fn transparent_layers_skip_ownership_but_descendant_geometry_still_protects_ancestors(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);

    let lower_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "lower-menu",
            policy(
                OverlayLayerKind::Menu,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndConsume,
            ),
            lower_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "unrelated-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            Rc::new(RefCell::new(Vec::new())),
        ),
    );
    set_inside_region(
        cx,
        &view,
        "unrelated-tooltip",
        "tooltip",
        rect(200.0, 200.0, 40.0, 40.0),
    );
    let transparent_hit = cx.simulate_event_with_dispatch_snapshot(mouse_down(220.0, 220.0));
    assert!(transparent_hit.default_prevented());
    assert_eq!(lower_events.borrow().as_slice(), &[false]);
    unregister_layer(cx, &view, "unrelated-tooltip");
    unregister_layer(cx, &view, "lower-menu");

    register_layer(
        cx,
        &view,
        controlled_registration(
            "modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            Rc::new(RefCell::new(Vec::new())),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "modal-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            Rc::new(RefCell::new(Vec::new())),
        ),
    );
    set_inside_region(cx, &view, "modal", "modal", rect(20.0, 20.0, 100.0, 100.0));
    set_inside_region(
        cx,
        &view,
        "modal-tooltip",
        "tooltip",
        rect(240.0, 240.0, 40.0, 40.0),
    );
    let transparent_outside_modal =
        cx.simulate_event_with_dispatch_snapshot(mouse_down(260.0, 260.0));
    assert!(transparent_outside_modal.default_prevented());
    let scroll_outside_modal = cx.simulate_event_with_dispatch_snapshot(ScrollWheelEvent {
        position: point(px(260.0), px(260.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-24.0))),
        modifiers: Default::default(),
        touch_phase: TouchPhase::Moved,
    });
    let pinch_outside_modal = cx.simulate_event_with_dispatch_snapshot(PinchEvent {
        position: point(px(260.0), px(260.0)),
        delta: 0.1,
        modifiers: Default::default(),
        phase: TouchPhase::Moved,
    });
    assert!(scroll_outside_modal.default_prevented());
    assert!(scroll_outside_modal.propagation_stopped());
    assert!(pinch_outside_modal.default_prevented());
    assert!(pinch_outside_modal.propagation_stopped());
    unregister_layer(cx, &view, "modal-tooltip");
    unregister_layer(cx, &view, "modal");

    let ancestor_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "ancestor-menu",
            policy(
                OverlayLayerKind::Menu,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndConsume,
            ),
            ancestor_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "descendant-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            Rc::new(RefCell::new(Vec::new())),
        )
        .parent("ancestor-menu"),
    );
    set_inside_region(
        cx,
        &view,
        "descendant-tooltip",
        "tooltip",
        rect(680.0, 450.0, 40.0, 40.0),
    );
    let descendant_inside = cx.simulate_event_with_dispatch_snapshot(mouse_down(700.0, 470.0));
    assert!(!descendant_inside.default_prevented());
    assert!(ancestor_events.borrow().is_empty());
}

#[open_gpui::test]
fn transparent_geometry_cannot_punch_through_a_modal_barrier_below_pass_through_content(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let modal_events = Rc::new(RefCell::new(Vec::new()));
    let top_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "barrier-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            modal_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "barrier-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            Rc::new(RefCell::new(Vec::new())),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "barrier-pass-through",
            policy(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
                OutsidePressPolicy::DismissAndPassThrough,
            ),
            top_events.clone(),
        ),
    );
    set_inside_region(
        cx,
        &view,
        "barrier-modal",
        "modal",
        rect(20.0, 20.0, 100.0, 100.0),
    );
    set_inside_region(
        cx,
        &view,
        "barrier-tooltip",
        "tooltip",
        rect(680.0, 450.0, 40.0, 40.0),
    );
    set_inside_region(
        cx,
        &view,
        "barrier-pass-through",
        "surface",
        rect(500.0, 100.0, 80.0, 80.0),
    );

    let dispatch = cx.simulate_event_with_dispatch_snapshot(mouse_down(700.0, 470.0));
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    assert_eq!(top_events.borrow().as_slice(), &[false]);
    assert!(modal_events.borrow().is_empty());
}
