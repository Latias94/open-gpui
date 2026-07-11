use super::*;

struct LayerOwner;

#[open_gpui::test]
fn window_teardown_cancels_local_capture_and_rejects_stale_runtime_work(
    cx: &mut open_gpui::TestAppContext,
) {
    let closing_window = cx.add_window(RuntimeProbe::new);
    let surviving_window = cx.add_window(RuntimeProbe::new);
    let closing_any = closing_window.clone().into();
    let surviving_any = surviving_window.clone().into();

    let (owner, stale_runtime, pointer_events) = closing_window
        .update(cx, |probe, window, cx| {
            window.activate_window();
            let owner = cx.new(|_| LayerOwner);
            let binding = probe
                .runtime
                .register_layer(
                    uncontrolled_registration(
                        "teardown-modal",
                        OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                    ),
                    window,
                    cx,
                )
                .expect("teardown modal should register");
            probe
                .runtime
                .bind_layer_to_entity_release(&binding, &owner, window, cx)
                .expect("teardown modal should observe owner release");
            probe.add_layer(binding);
            probe.set_pointer_capture("teardown-modal", true);
            cx.notify();
            (
                owner,
                probe.runtime.clone(),
                probe.surface_pointer_events.clone(),
            )
        })
        .expect("closing window should remain open during setup");
    cx.run_until_parked();
    cx.update_window(closing_any, |_, window, cx| window.draw(cx).clear())
        .expect("closing window should draw its capture target");
    cx.update_window(surviving_any, |_, window, cx| window.draw(cx).clear())
        .expect("surviving window should draw and accept focus");
    surviving_window
        .update(cx, |probe, window, cx| {
            probe.second_extra_focus.focus(window, cx);
        })
        .expect("surviving window should accept focus");

    {
        let mut visual = open_gpui::VisualTestContext::from_window(closing_any, cx);
        let surface = visual
            .debug_bounds("window-overlay-runtime:teardown-modal:surface")
            .expect("teardown modal surface should render");
        visual.simulate_mouse_down(surface.center(), MouseButton::Left, Default::default());
        assert!(visual.update(|window, _| window.captured_pointer().is_some()));
    }

    drop(owner);
    cx.update_window(closing_any, |_, window, cx| window.remove_window(cx))
        .expect("closing window should be removable with pending owner cleanup");
    cx.run_until_parked();

    assert_eq!(
        pointer_events.borrow().as_slice(),
        &[
            ("teardown-modal".to_owned(), "down"),
            ("teardown-modal".to_owned(), "cancel")
        ]
    );
    let (surviving_focus, surviving_layers, stale_result) = surviving_window
        .update(cx, |probe, window, cx| {
            (
                probe.second_extra_focus.is_focused(window),
                probe
                    .runtime
                    .snapshot(window, cx)
                    .expect("surviving runtime should remain readable")
                    .layers()
                    .len(),
                stale_runtime.snapshot(window, cx),
            )
        })
        .expect("surviving window should remain open");
    assert!(surviving_focus);
    assert_eq!(surviving_layers, 0);
    assert_eq!(
        stale_result.unwrap_err(),
        WindowOverlayRuntimeError::WrongWindow
    );
    assert!(closing_window.update(cx, |_, _, _| ()).is_err());
}

#[open_gpui::test]
fn identical_layer_ids_and_input_are_isolated_between_windows(cx: &mut open_gpui::TestAppContext) {
    let first_window = cx.add_window(RuntimeProbe::new);
    let second_window = cx.add_window(RuntimeProbe::new);
    let first_any = first_window.clone().into();
    let second_any = second_window.clone().into();
    let first_events = Rc::new(RefCell::new(Vec::new()));
    let second_events = Rc::new(RefCell::new(Vec::new()));

    let first_runtime_id = first_window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .runtime
                .register_layer(
                    controlled_registration(
                        "shared-id",
                        OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
                        first_events.clone(),
                    ),
                    window,
                    cx,
                )
                .expect("first window layer should register");
            probe.add_layer(binding);
            probe.runtime.entity_id()
        })
        .expect("first window should remain open");
    let second_runtime_id = second_window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .runtime
                .register_layer(
                    controlled_registration(
                        "shared-id",
                        OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
                        second_events.clone(),
                    ),
                    window,
                    cx,
                )
                .expect("second window layer should register");
            probe.add_layer(binding);
            probe.runtime.entity_id()
        })
        .expect("second window should remain open");
    assert_ne!(first_runtime_id, second_runtime_id);

    cx.update_window(first_any, |_, window, cx| window.draw(cx).clear())
        .expect("first window should remain open");
    cx.update_window(second_any, |_, window, cx| window.draw(cx).clear())
        .expect("second window should remain open");
    cx.simulate_keystrokes(first_any, "escape");

    assert_eq!(first_events.borrow().as_slice(), &[false]);
    assert!(second_events.borrow().is_empty());
    let second_phase = second_window
        .update(cx, |probe, window, cx| {
            probe
                .runtime
                .snapshot(window, cx)
                .expect("second snapshot should remain isolated")
                .layers()[0]
                .phase()
        })
        .expect("second window should remain open");
    assert_eq!(second_phase, OverlayLayerPhase::Open);
}

#[open_gpui::test]
fn overlay_surface_from_another_window_cannot_project_parentage_or_commit_geometry(
    cx: &mut open_gpui::TestAppContext,
) {
    let first_window = cx.add_window(RuntimeProbe::new);
    let first_any = first_window.clone().into();
    let close_events = Rc::new(RefCell::new(Vec::new()));
    let (runtime, binding) = first_window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .runtime
                .register_layer(
                    controlled_registration(
                        "foreign-surface-owner",
                        policy(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                            OutsidePressPolicy::DismissAndConsume,
                        ),
                        close_events.clone(),
                    )
                    .focus_mode(OverlayFocusMode::None),
                    window,
                    cx,
                )
                .expect("first window layer should register");
            probe.add_layer(binding.clone());
            probe.set_surface_rendered("foreign-surface-owner", false);
            (probe.runtime.clone(), binding)
        })
        .expect("first window should remain open");
    cx.update_window(first_any, |_, window, cx| window.draw(cx).clear())
        .expect("first window should draw without a local surface");

    let second_window = cx.add_window({
        let runtime = runtime.clone();
        let binding = binding.clone();
        move |_, _| ForeignOverlaySurfaceProbe { runtime, binding }
    });
    let second_any = second_window.clone().into();
    cx.update_window(second_any, |_, window, cx| window.draw(cx).clear())
        .expect("foreign surface window should draw without mutating its owner runtime");

    let mut first_visual = open_gpui::VisualTestContext::from_window(first_any, cx);
    let dispatch = first_visual.simulate_event_with_dispatch_snapshot(mouse_down(80.0, 64.0));
    assert!(dispatch.default_prevented());
    assert!(dispatch.propagation_stopped());
    assert_eq!(close_events.borrow().as_slice(), &[false]);
}

#[open_gpui::test]
fn owner_release_unregisters_scope_targets_and_allows_clean_remount(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);

    let (owner, old_binding, old_target): (Entity<LayerOwner>, _, OverlayFocusTargetLease) = cx
        .update_window_entity(&view, |probe, window, cx| {
            let owner = cx.new(|_| LayerOwner);
            let binding = probe
                .runtime
                .register_layer(
                    uncontrolled_registration(
                        "released-layer",
                        OverlayLayerPolicy::new(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::hidden(),
                        ),
                    )
                    .focus_mode(OverlayFocusMode::Passive),
                    window,
                    cx,
                )
                .expect("owned layer should register");
            let target = probe
                .runtime
                .register_focus_target(
                    &binding,
                    FocusTargetRegistration::new("released-target", &probe.first_extra_focus),
                    window,
                    cx,
                )
                .expect("owned target should register");
            probe
                .runtime
                .bind_layer_to_entity_release(&binding, &owner, window, cx)
                .expect("layer should observe its owner");
            probe.add_layer(binding.clone());
            (owner, binding, target)
        });
    drop(owner);
    settle_focus_claims(cx);

    let layers_after_release = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("released runtime should remain live")
            .layers()
            .len()
    });
    assert_eq!(layers_after_release, 0);
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.remove_layer("released-layer");
        cx.notify();
    });

    let new_binding = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "released-layer",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );
    let new_target = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_focus_target(
                &new_binding,
                FocusTargetRegistration::new("released-target", &probe.second_extra_focus),
                window,
                cx,
            )
            .expect("release cleanup should free the target identity")
    });
    let stale_layer = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_focus_target(
            &old_binding,
            &old_target,
            FocusTargetRegistration::new("released-target", &probe.first_extra_focus),
            window,
            cx,
        )
    });
    assert!(matches!(
        stale_layer,
        Err(WindowOverlayRuntimeError::ForeignLease(_))
            | Err(WindowOverlayRuntimeError::ForeignFocusTargetLease(_))
    ));
    assert_ne!(old_target, new_target);
}

#[open_gpui::test]
fn owner_release_forces_registered_subtree_teardown_leaf_to_root(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let owner = cx.update_window_entity(&view, |probe, window, cx| {
        let owner = cx.new(|_| LayerOwner);
        let parent = probe
            .runtime
            .register_layer(
                uncontrolled_registration(
                    "release-root",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::hidden(),
                    ),
                )
                .focus_mode(OverlayFocusMode::Passive),
                window,
                cx,
            )
            .expect("release root should register");
        let child = probe
            .runtime
            .register_layer(
                uncontrolled_registration(
                    "release-child",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::hidden(),
                    ),
                )
                .parent("release-root")
                .focus_mode(OverlayFocusMode::Passive),
                window,
                cx,
            )
            .expect("release child should register");
        let grandchild = probe
            .runtime
            .register_layer(
                uncontrolled_registration(
                    "release-grandchild",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::hidden(),
                    ),
                )
                .parent("release-child")
                .focus_mode(OverlayFocusMode::Passive),
                window,
                cx,
            )
            .expect("release grandchild should register");
        probe
            .runtime
            .bind_layer_to_entity_release(&parent, &owner, window, cx)
            .expect("root should observe owner release");
        probe.add_layer(parent);
        probe.add_layer(child);
        probe.add_layer(grandchild);
        owner
    });
    drop(owner);
    settle_focus_claims(cx);

    let remaining = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("runtime should survive subtree release")
            .layers()
            .len()
    });
    assert_eq!(remaining, 0);
    cx.update_window_entity(&view, |probe, _, cx| {
        probe.layers.clear();
        cx.notify();
    });

    let remounted_root = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "release-root",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );
    let remounted_child = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "release-child",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
        )
        .parent("release-root")
        .focus_mode(OverlayFocusMode::Passive),
    );
    assert_eq!(remounted_root.lease().layer_id().as_str(), "release-root");
    assert_eq!(remounted_child.lease().layer_id().as_str(), "release-child");
}

#[open_gpui::test]
fn owner_release_uses_focused_descendant_scope_when_root_has_no_scope(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    draw(cx);
    let owner = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("release-focus-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("window fallback should register");
        probe.fallback_focus.focus(window, cx);
        let owner = cx.new(|_| LayerOwner);
        let root = probe
            .runtime
            .register_layer(
                uncontrolled_registration(
                    "release-focus-root",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::open(),
                    ),
                )
                .focus_mode(OverlayFocusMode::None),
                window,
                cx,
            )
            .expect("scope-free release root should register");
        let child = probe
            .runtime
            .register_layer(
                uncontrolled_registration(
                    "release-focus-child",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::open(),
                    ),
                )
                .parent("release-focus-root")
                .focus_mode(OverlayFocusMode::Passive),
                window,
                cx,
            )
            .expect("focused release child should register");
        probe
            .runtime
            .bind_layer_to_entity_release(&root, &owner, window, cx)
            .expect("root should own release cleanup for the whole subtree");
        probe.add_layer(root);
        probe.add_layer(child);
        cx.notify();
        owner
    });
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .binding("release-focus-child")
            .surface_focus()
            .focus(window, cx);
    });
    draw(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:release-focus-child:surface"));

    drop(owner);
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:fallback"));
    let remaining = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("runtime should survive focused subtree release")
            .layers()
            .len()
    });
    assert_eq!(remaining, 0);
}
