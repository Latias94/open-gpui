use super::*;

#[open_gpui::test]
fn top_escape_ignore_blocks_lower_overlay_and_application_key_dispatch(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let lower_events = Rc::new(RefCell::new(Vec::new()));
    let top_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "escape-lower",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            lower_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "escape-ignore-top",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_escape_key_policy(EscapeKeyPolicy::Ignore),
            top_events.clone(),
        ),
    );
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.underlay_focus.focus(window, cx);
    });

    cx.simulate_keystrokes("escape");

    assert!(lower_events.borrow().is_empty());
    assert!(top_events.borrow().is_empty());
    let application_escape_count =
        cx.update_window_entity(&view, |probe, _, _| probe.underlay_escape_keys.get());
    assert_eq!(
        application_escape_count, 0,
        "top Escape Ignore owns arbitration and must stop application dispatch"
    );
}

#[open_gpui::test]
fn tab_preserve_tooltip_does_not_hide_default_menu_root_dismissal(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let menu_events = Rc::new(RefCell::new(Vec::new()));
    let tooltip_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "tab-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            menu_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "tab-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            tooltip_events.clone(),
        )
        .tab_behavior(OverlayTabBehavior::Preserve),
    );
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.underlay_focus.focus(window, cx);
    });

    cx.simulate_keystrokes("tab");

    assert_eq!(menu_events.borrow().as_slice(), &[false]);
    assert!(tooltip_events.borrow().is_empty());
    assert_eq!(
        snapshot_layer(cx, &view, "tab-menu").phase(),
        OverlayLayerPhase::CloseRequested
    );
}

#[open_gpui::test]
fn default_menu_tab_behavior_dismisses_the_contiguous_menu_root(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let root_events = Rc::new(RefCell::new(Vec::new()));
    let child_events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "tab-root-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            root_events.clone(),
        ),
    );
    register_layer(
        cx,
        &view,
        controlled_registration(
            "tab-child-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            child_events.clone(),
        )
        .parent("tab-root-menu"),
    );
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.underlay_focus.focus(window, cx);
    });

    cx.simulate_keystrokes("tab");

    assert_eq!(root_events.borrow().as_slice(), &[false]);
    assert!(child_events.borrow().is_empty());
    assert_eq!(
        snapshot_layer(cx, &view, "tab-root-menu").phase(),
        OverlayLayerPhase::CloseRequested
    );
    assert_eq!(
        snapshot_layer(cx, &view, "tab-child-menu").phase(),
        OverlayLayerPhase::Open
    );
}

#[open_gpui::test]
fn default_non_modal_restore_preserves_new_focus_until_the_surface_owns_it(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.underlay_focus.focus(window, cx);
    });
    let binding = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "conditional-default",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        ),
    );
    settle_focus_claims(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.first_extra_focus.focus(window, cx);
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
            .expect("non-modal layer should close without claiming focus");
    });
    settle_focus_claims(cx);

    assert!(cx.debug_selector_is_focused("window-overlay-runtime:extra-a"));
}

#[open_gpui::test]
fn modal_preserve_tab_runs_the_real_focus_loop_without_dismissal(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    register_layer(
        cx,
        &view,
        controlled_registration(
            "modal-tab-loop",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            events.clone(),
        ),
    );
    settle_focus_claims(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe.underlay_focus.focus(window, cx);
    });

    let dispatch = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
        keystroke: Keystroke::parse("tab").expect("Tab should parse"),
        is_held: false,
        prefer_character_input: false,
    });

    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    assert!(events.borrow().is_empty());
    assert_eq!(
        snapshot_layer(cx, &view, "modal-tab-loop").phase(),
        OverlayLayerPhase::Open
    );
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:modal-tab-loop:surface"));
}

#[open_gpui::test]
fn controlled_refusal_keeps_modal_focus_registration_and_deduplicates_intent(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let binding = register_layer(
        cx,
        &view,
        controlled_registration(
            "controlled-modal",
            policy(
                OverlayLayerKind::Modal,
                OverlayPresence::open(),
                OutsidePressPolicy::Consume,
            ),
            events.clone(),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:controlled-modal:surface"));

    cx.simulate_keystrokes("escape escape tab");
    let snapshot = snapshot_layer(cx, &view, "controlled-modal");
    assert_eq!(events.borrow().as_slice(), &[false]);
    assert_eq!(snapshot.phase(), OverlayLayerPhase::CloseRequested);
    assert_eq!(snapshot.presence(), OverlayPresence::open());
    assert_eq!(snapshot.pending_open(), Some(false));
    assert_eq!(snapshot.pending_intent(), Some(DismissReason::EscapeKey));
    let first_revision = snapshot
        .pending_intent_revision()
        .expect("controlled close should expose its intent revision");
    assert!(snapshot.keyboard_eligible());
    assert!(snapshot.modal_pointer_barrier());
    assert!(snapshot.focus_active());
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:controlled-modal:surface"));

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .rebind_layer(
                &binding,
                controlled_registration(
                    "controlled-modal",
                    policy(
                        OverlayLayerKind::Modal,
                        OverlayPresence::open(),
                        OutsidePressPolicy::Consume,
                    ),
                    events.clone(),
                )
                .focus_mode(OverlayFocusMode::Modal),
                window,
                cx,
            )
            .expect("controlled owner refusal should rebind as still open");
    });
    cx.simulate_keystrokes("escape escape");
    assert_eq!(events.borrow().as_slice(), &[false]);
    assert_eq!(
        snapshot_layer(cx, &view, "controlled-modal").phase(),
        OverlayLayerPhase::CloseRequested
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .reject_controlled_intent(&binding, first_revision, window, cx)
            .expect("owner should explicitly reject the matching intent");
    });
    let rejected = snapshot_layer(cx, &view, "controlled-modal");
    assert_eq!(rejected.phase(), OverlayLayerPhase::Open);
    assert_eq!(rejected.pending_open(), None);
    assert_eq!(rejected.pending_intent_revision(), None);

    cx.simulate_keystrokes("escape escape");
    assert_eq!(events.borrow().as_slice(), &[false, false]);
    let second_revision = snapshot_layer(cx, &view, "controlled-modal")
        .pending_intent_revision()
        .expect("retry should create a new intent revision");
    assert_ne!(first_revision, second_revision);
    let stale_error = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .reject_controlled_intent(&binding, first_revision, window, cx)
            .expect_err("an old intent revision must not resolve a newer request")
    });
    assert_eq!(
        stale_error,
        WindowOverlayRuntimeError::StaleIntent(OverlayLayerId::new("controlled-modal"))
    );

    let underlay = cx
        .debug_bounds("window-overlay-runtime:underlay")
        .expect("underlay should render");
    cx.simulate_click(underlay.center(), Default::default());
    let clicks = cx.update_window_entity(&view, |probe, _, _| probe.underlay_clicks.get());
    assert_eq!(clicks, 0);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:controlled-modal:surface"));
}

#[open_gpui::test]
fn delayed_controlled_close_restore_cannot_steal_focus_from_callback_opened_layer(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("delayed-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("fallback should register");
        probe.fallback_focus.focus(window, cx);
    });
    let callback_opened = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "callback-opened",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::hidden()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    let close_events = Rc::new(RefCell::new(Vec::new()));
    let closing_binding = Rc::new(RefCell::new(None::<OverlayLayerBinding>));
    let _delayed = cx.update_window_entity(&view, |probe, window, cx| {
        let runtime_for_callback = probe.runtime.clone();
        let opened_for_callback = callback_opened.clone();
        let closing_for_callback = closing_binding.clone();
        let events_for_callback = close_events.clone();
        let registration = OverlayLayerRegistration::new(
            "delayed-controlled",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            OverlayOwnership::Controlled,
        )
        .focus_mode(OverlayFocusMode::Modal)
        .on_open_change(move |open, window, cx| {
            events_for_callback.borrow_mut().push(open);
            if open {
                return;
            }
            runtime_for_callback
                .request_open_change(
                    &opened_for_callback,
                    true,
                    DismissReason::Programmatic,
                    window,
                    cx,
                )
                .expect("close callback should open the replacement layer");
            opened_for_callback.surface_focus().focus(window, cx);

            let runtime_for_commit = runtime_for_callback.clone();
            let closing_for_commit = closing_for_callback.clone();
            window.on_next_frame(move |window, cx| {
                let binding = closing_for_commit
                    .borrow()
                    .clone()
                    .expect("controlled binding should exist before owner commit");
                runtime_for_commit
                    .rebind_layer(
                        &binding,
                        OverlayLayerRegistration::new(
                            "delayed-controlled",
                            OverlayLayerPolicy::new(
                                OverlayLayerKind::Modal,
                                OverlayPresence::closing(),
                            ),
                            OverlayOwnership::Controlled,
                        )
                        .focus_mode(OverlayFocusMode::Modal),
                        window,
                        cx,
                    )
                    .expect("owner should commit the delayed controlled close");
            });
            window.refresh();
        });
        let binding = probe
            .runtime
            .register_layer(registration, window, cx)
            .expect("controlled layer should register");
        closing_binding.replace(Some(binding.clone()));
        probe.add_layer(binding.clone());
        cx.notify();
        binding
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:delayed-controlled:surface"));

    cx.simulate_keystrokes("escape");
    settle_focus_claims(cx);

    assert_eq!(close_events.borrow().as_slice(), &[false]);
    assert_eq!(
        snapshot_layer(cx, &view, "delayed-controlled").phase(),
        OverlayLayerPhase::Closing
    );
    assert_eq!(
        snapshot_layer(cx, &view, "callback-opened").phase(),
        OverlayLayerPhase::Open
    );
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:callback-opened:surface"));
}

#[open_gpui::test]
fn nested_focus_scopes_restore_parent_then_window_fallback_in_lifo_order(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("window-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("window fallback should register");
        probe.fallback_focus.focus(window, cx);
    });

    let parent = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "focus-parent",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:focus-parent:surface"));

    let child = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "focus-child",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .parent("focus-parent")
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:focus-child:surface"));

    let child_closing = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&child, false, DismissReason::Programmatic, window, cx)
            .expect("child should close")
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:focus-parent:surface"));
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&child, child_closing, window, cx)
            .expect("child must become hidden before its parent closes");
    });

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&parent, false, DismissReason::Programmatic, window, cx)
            .expect("parent should close");
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:fallback"));
}

#[open_gpui::test]
fn unmounted_trigger_restores_live_window_fallback(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("window-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("window fallback should register");
    });
    let binding = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "trigger-fallback",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        binding.trigger_focus().focus(window, cx);
        probe
            .runtime
            .request_open_change(&binding, true, DismissReason::Programmatic, window, cx)
            .expect("hidden layer should open");
        binding.surface_focus().focus(window, cx);
    });
    draw(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:trigger-fallback:surface"));

    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_trigger_rendered("trigger-fallback", false);
        cx.notify();
    });
    draw(cx);
    assert!(
        cx.debug_bounds("window-overlay-runtime:trigger-fallback:trigger")
            .is_none()
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
            .expect("layer should close");
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:fallback"));
}

#[open_gpui::test]
fn unmounted_initial_focus_surface_restores_without_a_newer_focus_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("initial-unmount-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("window fallback should register");
        probe.fallback_focus.focus(window, cx);
    });
    let binding = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "initial-unmount-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:initial-unmount-modal:surface"));

    cx.update_window_entity(&view, |probe, _, cx| {
        probe.set_surface_rendered("initial-unmount-modal", false);
        cx.notify();
    });
    draw(cx);
    assert!(
        cx.debug_bounds("window-overlay-runtime:initial-unmount-modal:surface")
            .is_none()
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
            .expect("modal should restore after its initial surface unmounts");
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:fallback"));
}

#[open_gpui::test]
fn parent_subtree_close_cancels_an_already_closing_child_restore_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("subtree-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("window fallback should register");
        probe.fallback_focus.focus(window, cx);
    });
    let parent = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "claim-parent",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal)
        .focus_restore_condition(OverlayFocusRestoreCondition::Never),
    );
    settle_focus_claims(cx);
    let child = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "claim-child",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .parent("claim-parent")
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&child, false, DismissReason::Programmatic, window, cx)
            .expect("child close should queue its ordinary restore claim");
    });
    let parent_generation = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&parent, false, DismissReason::Programmatic, window, cx)
            .expect("parent close should adopt the already-closing child")
    });
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&parent, parent_generation, window, cx)
            .expect("parent exit should finalize the adopted subtree");
    });
    settle_focus_claims(cx);

    assert!(
        !cx.debug_selector_is_focused("window-overlay-runtime:fallback"),
        "a child restore claim must not bypass its parent's Never policy"
    );
}
