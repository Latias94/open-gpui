use super::*;

#[open_gpui::test]
fn duplicate_registration_and_failed_rebind_are_atomic_and_ids_remount(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);

    let events = Rc::new(RefCell::new(Vec::new()));
    let binding = register_layer(
        cx,
        &view,
        controlled_registration(
            "atomic",
            policy(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
                OutsidePressPolicy::Ignore,
            ),
            events.clone(),
        ),
    );

    let duplicate = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            controlled_registration(
                "atomic",
                policy(
                    OverlayLayerKind::Menu,
                    OverlayPresence::open(),
                    OutsidePressPolicy::DismissAndConsume,
                ),
                Rc::new(RefCell::new(Vec::new())),
            ),
            window,
            cx,
        )
    });
    assert_eq!(
        duplicate.err().expect("duplicate registration should fail"),
        WindowOverlayRuntimeError::DuplicateLayer(OverlayLayerId::new("atomic"))
    );
    assert_eq!(
        snapshot_layer(cx, &view, "atomic").kind(),
        OverlayLayerKind::NonModalDismissible
    );

    let invalid_rebind = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_layer(
            &binding,
            controlled_registration(
                "atomic",
                policy(
                    OverlayLayerKind::Menu,
                    OverlayPresence::open(),
                    OutsidePressPolicy::DismissAndConsume,
                ),
                events.clone(),
            )
            .parent("missing-parent"),
            window,
            cx,
        )
    });
    assert_eq!(
        invalid_rebind.unwrap_err(),
        WindowOverlayRuntimeError::MissingParent(OverlayLayerId::new("missing-parent"))
    );
    assert_eq!(
        snapshot_layer(cx, &view, "atomic").kind(),
        OverlayLayerKind::NonModalDismissible
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .rebind_layer(
                &binding,
                controlled_registration(
                    "atomic",
                    policy(
                        OverlayLayerKind::Menu,
                        OverlayPresence::open(),
                        OutsidePressPolicy::DismissAndConsume,
                    ),
                    events,
                ),
                window,
                cx,
            )
            .expect("valid rebind should commit");
    });
    assert_eq!(
        snapshot_layer(cx, &view, "atomic").kind(),
        OverlayLayerKind::Menu
    );

    unregister_layer(cx, &view, "atomic");
    let replacement = register_layer(
        cx,
        &view,
        controlled_registration(
            "atomic",
            policy(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
                OutsidePressPolicy::Ignore,
            ),
            Rc::new(RefCell::new(Vec::new())),
        ),
    );
    let stale = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
    });
    assert_eq!(
        stale.unwrap_err(),
        WindowOverlayRuntimeError::ForeignLease(OverlayLayerId::new("atomic"))
    );
    assert_ne!(binding.lease(), replacement.lease());
}

#[open_gpui::test]
fn incompatible_overlay_focus_profiles_fail_before_registration(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);

    let invalid_modal = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            controlled_registration(
                "invalid-modal-focus",
                OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                Rc::new(RefCell::new(Vec::new())),
            )
            .focus_mode(OverlayFocusMode::None),
            window,
            cx,
        )
    });
    assert_eq!(
        invalid_modal.err().expect("modal profile should fail"),
        WindowOverlayRuntimeError::IncompatibleFocusMode {
            layer: OverlayLayerId::new("invalid-modal-focus"),
            kind: OverlayLayerKind::Modal,
            focus_mode: OverlayFocusMode::None,
        }
    );

    let invalid_tooltip = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            controlled_registration(
                "invalid-tooltip-focus",
                OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
                Rc::new(RefCell::new(Vec::new())),
            )
            .focus_mode(OverlayFocusMode::Modal),
            window,
            cx,
        )
    });
    assert_eq!(
        invalid_tooltip.err().expect("tooltip profile should fail"),
        WindowOverlayRuntimeError::IncompatibleFocusMode {
            layer: OverlayLayerId::new("invalid-tooltip-focus"),
            kind: OverlayLayerKind::Tooltip,
            focus_mode: OverlayFocusMode::Modal,
        }
    );

    let invalid_modal_tab = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            controlled_registration(
                "invalid-modal-tab",
                OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                Rc::new(RefCell::new(Vec::new())),
            )
            .tab_behavior(OverlayTabBehavior::DismissSelf),
            window,
            cx,
        )
    });
    assert_eq!(
        invalid_modal_tab
            .err()
            .expect("modal Tab override should fail"),
        WindowOverlayRuntimeError::IncompatibleTabBehavior {
            layer: OverlayLayerId::new("invalid-modal-tab"),
            kind: OverlayLayerKind::Modal,
            behavior: OverlayTabBehavior::DismissSelf,
        }
    );

    let invalid_non_menu_root_tab = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            controlled_registration(
                "invalid-non-menu-root-tab",
                OverlayLayerPolicy::new(
                    OverlayLayerKind::NonModalDismissible,
                    OverlayPresence::open(),
                ),
                Rc::new(RefCell::new(Vec::new())),
            )
            .tab_behavior(OverlayTabBehavior::DismissMenuRoot),
            window,
            cx,
        )
    });
    assert_eq!(
        invalid_non_menu_root_tab
            .err()
            .expect("non-menu root dismissal should fail"),
        WindowOverlayRuntimeError::IncompatibleTabBehavior {
            layer: OverlayLayerId::new("invalid-non-menu-root-tab"),
            kind: OverlayLayerKind::NonModalDismissible,
            behavior: OverlayTabBehavior::DismissMenuRoot,
        }
    );

    let layer_count = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("invalid registrations must leave the runtime readable")
            .layers()
            .len()
    });
    assert_eq!(layer_count, 0);
}

#[open_gpui::test]
fn controlled_reopen_intents_remain_typed_while_hidden_or_closing(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let hidden_events = Rc::new(RefCell::new(Vec::new()));
    let closing_events = Rc::new(RefCell::new(Vec::new()));
    let hidden = register_layer(
        cx,
        &view,
        controlled_registration(
            "pending-hidden-reopen",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::hidden(),
            ),
            hidden_events.clone(),
        )
        .focus_mode(OverlayFocusMode::None),
    );
    let closing = register_layer(
        cx,
        &view,
        controlled_registration(
            "pending-closing-reopen",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::closing(),
            ),
            closing_events.clone(),
        )
        .focus_mode(OverlayFocusMode::None),
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&hidden, true, DismissReason::Programmatic, window, cx)
            .expect("hidden controlled layer should retain its reopen intent");
        probe
            .runtime
            .request_open_change(&closing, true, DismissReason::Trigger, window, cx)
            .expect("closing controlled layer should retain its reopen intent");
    });

    let hidden = snapshot_layer(cx, &view, "pending-hidden-reopen");
    assert_eq!(hidden.phase(), OverlayLayerPhase::Hidden);
    assert_eq!(hidden.presence(), OverlayPresence::hidden());
    assert_eq!(hidden.pending_open(), Some(true));
    assert_eq!(hidden.pending_intent(), Some(DismissReason::Programmatic));
    assert!(hidden.pending_intent_revision().is_some());
    let closing = snapshot_layer(cx, &view, "pending-closing-reopen");
    assert_eq!(closing.phase(), OverlayLayerPhase::Closing);
    assert_eq!(closing.presence(), OverlayPresence::closing());
    assert_eq!(closing.pending_open(), Some(true));
    assert_eq!(closing.pending_intent(), Some(DismissReason::Trigger));
    assert!(closing.pending_intent_revision().is_some());
    assert_eq!(hidden_events.borrow().as_slice(), &[true]);
    assert_eq!(closing_events.borrow().as_slice(), &[true]);
}

#[open_gpui::test]
fn a_layer_lease_cannot_change_its_focus_profile_while_remaining_open(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let binding = register_layer(
        cx,
        &view,
        controlled_registration(
            "stable-focus-profile",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            Rc::new(RefCell::new(Vec::new())),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );

    let error = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_layer(
            &binding,
            controlled_registration(
                "stable-focus-profile",
                OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
                Rc::new(RefCell::new(Vec::new())),
            )
            .focus_mode(OverlayFocusMode::Modal),
            window,
            cx,
        )
    });
    assert_eq!(
        error.expect_err("one lease cannot silently replace its focus runtime profile"),
        WindowOverlayRuntimeError::FocusModeChanged(OverlayLayerId::new("stable-focus-profile"))
    );
    let snapshot = snapshot_layer(cx, &view, "stable-focus-profile");
    assert_eq!(snapshot.kind(), OverlayLayerKind::NonModalDismissible);
    assert!(snapshot.focus_active());
}

#[open_gpui::test]
fn same_phase_rebind_invalidates_callbacks_captured_by_an_older_dispatch(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let root_commits = Rc::new(RefCell::new(Vec::new()));
    let driver_commits = Rc::new(RefCell::new(Vec::new()));
    let old_target_commits = Rc::new(RefCell::new(Vec::new()));
    let old_target_observers = Rc::new(RefCell::new(Vec::new()));
    let new_target_commits = Rc::new(RefCell::new(Vec::new()));
    let new_target_observers = Rc::new(RefCell::new(Vec::new()));
    let target_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));

    let root = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "revision-root",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let commits = root_commits.clone();
            move |open, _, _| commits.borrow_mut().push(open)
        }),
    );

    let target = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "revision-target",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("revision-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let commits = old_target_commits.clone();
            move |open, _, _| commits.borrow_mut().push(open)
        })
        .on_open_change({
            let observers = old_target_observers.clone();
            move |intent, _, _| observers.borrow_mut().push(intent.desired_open())
        }),
    );
    target_slot.replace(Some(target));

    let runtime = cx.update_window_entity(&view, |probe, _, _| probe.runtime.clone());
    register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "revision-driver",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("revision-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let runtime = runtime.clone();
            let target_slot = target_slot.clone();
            let driver_commits = driver_commits.clone();
            let new_target_commits = new_target_commits.clone();
            let new_target_observers = new_target_observers.clone();
            move |open, window, cx| {
                driver_commits.borrow_mut().push(open);
                if open {
                    return;
                }
                let target = target_slot
                    .borrow()
                    .clone()
                    .expect("target binding should exist before dispatch");
                runtime
                    .rebind_layer(
                        &target,
                        OverlayLayerRegistration::new(
                            "revision-target",
                            OverlayLayerPolicy::new(
                                OverlayLayerKind::NonModalDismissible,
                                OverlayPresence::closing(),
                            ),
                            OverlayOwnership::Uncontrolled,
                        )
                        .parent("revision-root")
                        .focus_mode(OverlayFocusMode::None)
                        .uncontrolled_commit({
                            let commits = new_target_commits.clone();
                            move |open, _, _| commits.borrow_mut().push(open)
                        })
                        .on_open_change({
                            let observers = new_target_observers.clone();
                            move |intent, _, _| observers.borrow_mut().push(intent.desired_open())
                        }),
                        window,
                        cx,
                    )
                    .expect("same-phase rebind should replace target callbacks");
            }
        }),
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&root, false, DismissReason::Programmatic, window, cx)
            .expect("root close should dispatch its subtree");
    });

    assert_eq!(driver_commits.borrow().as_slice(), &[false]);
    assert_eq!(root_commits.borrow().as_slice(), &[false]);
    assert!(old_target_commits.borrow().is_empty());
    assert!(old_target_observers.borrow().is_empty());
    assert!(new_target_commits.borrow().is_empty());
    assert!(new_target_observers.borrow().is_empty());
}

#[open_gpui::test]
fn stale_subtree_dispatch_cannot_cross_an_identical_id_remount(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let root_commits = Rc::new(RefCell::new(Vec::new()));
    let old_target_commits = Rc::new(RefCell::new(Vec::new()));
    let old_target_observers = Rc::new(RefCell::new(Vec::new()));
    let replacement_commits = Rc::new(RefCell::new(Vec::new()));
    let replacement_observers = Rc::new(RefCell::new(Vec::new()));
    let driver_commits = Rc::new(RefCell::new(Vec::new()));
    let root_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));
    let target_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));
    let replacement_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));

    let root = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "aba-root",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let root_commits = root_commits.clone();
            move |open, _, _| root_commits.borrow_mut().push(open)
        }),
    );
    root_slot.replace(Some(root.clone()));

    let old_target = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "aba-target",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("aba-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let old_target_commits = old_target_commits.clone();
            move |open, _, _| old_target_commits.borrow_mut().push(open)
        })
        .on_open_change({
            let old_target_observers = old_target_observers.clone();
            move |intent, _, _| {
                old_target_observers
                    .borrow_mut()
                    .push(intent.desired_open())
            }
        }),
    );
    target_slot.replace(Some(old_target.clone()));

    let runtime = cx.update_window_entity(&view, |probe, _, _| probe.runtime.clone());
    register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "aba-driver",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("aba-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let runtime = runtime.clone();
            let root_slot = root_slot.clone();
            let target_slot = target_slot.clone();
            let replacement_slot = replacement_slot.clone();
            let replacement_commits = replacement_commits.clone();
            let replacement_observers = replacement_observers.clone();
            let driver_commits = driver_commits.clone();
            move |open, window, cx| {
                driver_commits.borrow_mut().push(open);
                if open {
                    return;
                }
                let root = root_slot
                    .borrow()
                    .clone()
                    .expect("root binding should exist before ABA dispatch");
                runtime
                    .request_open_change(&root, true, DismissReason::Programmatic, window, cx)
                    .expect("driver should reopen the root before remounting its sibling");
                let old_target = target_slot
                    .borrow()
                    .clone()
                    .expect("old target binding should exist before ABA dispatch");
                runtime
                    .unregister_layer(&old_target, window, cx)
                    .expect("closing sibling without focus should unregister synchronously");
                let replacement = runtime
                    .register_layer(
                        OverlayLayerRegistration::new(
                            "aba-target",
                            OverlayLayerPolicy::new(
                                OverlayLayerKind::NonModalDismissible,
                                OverlayPresence::hidden(),
                            ),
                            OverlayOwnership::Uncontrolled,
                        )
                        .parent("aba-root")
                        .focus_mode(OverlayFocusMode::None)
                        .uncontrolled_commit({
                            let replacement_commits = replacement_commits.clone();
                            move |open, _, _| replacement_commits.borrow_mut().push(open)
                        })
                        .on_open_change({
                            let replacement_observers = replacement_observers.clone();
                            move |intent, _, _| {
                                replacement_observers
                                    .borrow_mut()
                                    .push(intent.desired_open())
                            }
                        }),
                        window,
                        cx,
                    )
                    .expect("same stable ID should remount with a new lease");
                runtime
                    .request_open_change(
                        &replacement,
                        true,
                        DismissReason::Programmatic,
                        window,
                        cx,
                    )
                    .expect("replacement should reach the old dispatch generation");
                replacement_slot.replace(Some(replacement));
            }
        }),
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&root, false, DismissReason::Programmatic, window, cx)
            .expect("root close should dispatch the ABA driver first");
    });

    let replacement = replacement_slot
        .borrow()
        .clone()
        .expect("driver should install a replacement binding");
    assert_ne!(old_target.lease(), replacement.lease());
    assert_eq!(driver_commits.borrow().as_slice(), &[false]);
    assert_eq!(root_commits.borrow().as_slice(), &[true]);
    assert!(old_target_commits.borrow().is_empty());
    assert!(old_target_observers.borrow().is_empty());
    assert_eq!(replacement_commits.borrow().as_slice(), &[true]);
    assert_eq!(replacement_observers.borrow().as_slice(), &[true]);
    assert_eq!(
        snapshot_layer(cx, &view, "aba-target").phase(),
        OverlayLayerPhase::Open
    );
}

#[open_gpui::test]
fn focus_target_leases_reject_cross_layer_changed_identity_and_stale_use(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let first = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "target-owner-a",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );
    let second = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "target-owner-b",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        )
        .focus_mode(OverlayFocusMode::Passive),
    );
    draw(cx);

    let target = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_focus_target(
                &first,
                FocusTargetRegistration::new("owned-target", &probe.first_extra_focus),
                window,
                cx,
            )
            .expect("first layer should own its target")
    });
    let cross_layer = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_focus_target(
            &second,
            &target,
            FocusTargetRegistration::new("owned-target", &probe.first_extra_focus),
            window,
            cx,
        )
    });
    assert_eq!(
        cross_layer.unwrap_err(),
        WindowOverlayRuntimeError::ForeignFocusTargetLease(FocusTargetId::new("owned-target"))
    );

    let changed_id = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_focus_target(
            &first,
            &target,
            FocusTargetRegistration::new("renamed-target", &probe.first_extra_focus),
            window,
            cx,
        )
    });
    assert_eq!(
        changed_id.unwrap_err(),
        WindowOverlayRuntimeError::FocusTargetIdChanged {
            expected: FocusTargetId::new("owned-target"),
            actual: FocusTargetId::new("renamed-target"),
        }
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .unregister_focus_target(&first, &target, window, cx)
            .expect("target should unregister");
    });
    let replacement = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_focus_target(
                &first,
                FocusTargetRegistration::new("owned-target", &probe.second_extra_focus),
                window,
                cx,
            )
            .expect("target id should be reusable")
    });
    let stale = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_focus_target(
            &first,
            &target,
            FocusTargetRegistration::new("owned-target", &probe.first_extra_focus),
            window,
            cx,
        )
    });
    assert_eq!(
        stale.unwrap_err(),
        WindowOverlayRuntimeError::ForeignFocusTargetLease(FocusTargetId::new("owned-target"))
    );
    assert_ne!(target, replacement);
}

#[open_gpui::test]
fn unregister_is_terminal_for_the_incarnation_before_deferred_cleanup(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .register_window_fallback(
                FocusTargetRegistration::new("terminal-fallback", &probe.fallback_focus),
                window,
                cx,
            )
            .expect("fallback should register");
        probe.fallback_focus.focus(window, cx);
    });
    let binding = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "terminal-layer",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);

    let (reopen, rebind, inside, target) = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .unregister_layer(&binding, window, cx)
            .expect("terminal cleanup should begin");
        let reopen = probe.runtime.request_open_change(
            &binding,
            true,
            DismissReason::Programmatic,
            window,
            cx,
        );
        let rebind = probe.runtime.rebind_layer(
            &binding,
            uncontrolled_registration(
                "terminal-layer",
                OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            )
            .focus_mode(OverlayFocusMode::Modal),
            window,
            cx,
        );
        let inside = probe.runtime.set_element_inside_region(
            &binding,
            OverlayInsideRegionId::new("terminal-inside"),
            rect(20.0, 20.0, 40.0, 40.0),
            window,
            cx,
        );
        let target = probe.runtime.register_focus_target(
            &binding,
            FocusTargetRegistration::new("terminal-target", &probe.first_extra_focus),
            window,
            cx,
        );
        (reopen, rebind, inside, target)
    });
    assert!(reopen.is_err(), "unregistering lease must not reopen");
    assert!(rebind.is_err(), "unregistering lease must not rebind");
    assert!(
        inside.is_err(),
        "unregistering lease must not accept live geometry"
    );
    assert!(
        target.is_err(),
        "unregistering lease must not acquire focus targets"
    );
    assert_ne!(
        snapshot_layer(cx, &view, "terminal-layer").phase(),
        OverlayLayerPhase::Open
    );

    settle_focus_claims(cx);
    let layer_count = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("runtime should survive terminal cleanup")
            .layers()
            .len()
    });
    assert_eq!(layer_count, 0);
}

#[open_gpui::test]
fn parent_topology_is_lease_stable_and_parent_close_adopts_present_descendants(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let first_parent = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "first-parent",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "second-parent",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        ),
    );
    let child = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "topology-child",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
        )
        .parent("first-parent"),
    );

    let changed_parent = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.rebind_layer(
            &child,
            uncontrolled_registration(
                "topology-child",
                OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            )
            .parent("second-parent"),
            window,
            cx,
        )
    });
    assert!(
        changed_parent.is_err(),
        "a lease must not migrate between parents"
    );
    assert_eq!(
        snapshot_layer(cx, &view, "topology-child")
            .parent()
            .map(OverlayLayerId::as_str),
        Some("first-parent")
    );

    let inactive_parent = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "inactive-parent",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::hidden()),
        ),
    );
    let interactive_under_hidden = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.register_layer(
            uncontrolled_registration(
                "invalid-interactive-child",
                OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            )
            .parent("inactive-parent"),
            window,
            cx,
        )
    });
    assert!(
        interactive_under_hidden.is_err(),
        "interactive child requires every ancestor to be interactive"
    );
    assert_eq!(
        snapshot_layer(cx, &view, "inactive-parent").phase(),
        OverlayLayerPhase::Hidden
    );
    assert_eq!(
        inactive_parent.lease().layer_id().as_str(),
        "inactive-parent"
    );

    let forced_parent_closing = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(
                &first_parent,
                false,
                DismissReason::Programmatic,
                window,
                cx,
            )
            .expect("ancestor close should force its open subtree into closing")
    });
    assert_eq!(
        snapshot_layer(cx, &view, "first-parent").phase(),
        OverlayLayerPhase::Closing
    );
    assert_eq!(
        snapshot_layer(cx, &view, "topology-child").phase(),
        OverlayLayerPhase::Closing
    );
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&first_parent, forced_parent_closing, window, cx)
            .expect("forced subtree should finalize with its root");
    });
    assert_eq!(
        snapshot_layer(cx, &view, "topology-child").phase(),
        OverlayLayerPhase::Hidden
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&first_parent, true, DismissReason::Programmatic, window, cx)
            .expect("parent should reopen before its child");
        probe
            .runtime
            .request_open_change(&child, true, DismissReason::Programmatic, window, cx)
            .expect("child should reopen under an interactive parent");
    });
    let child_closing = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&child, false, DismissReason::Programmatic, window, cx)
            .expect("leaf should close first")
    });
    let parent_closing = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(
                &first_parent,
                false,
                DismissReason::Programmatic,
                window,
                cx,
            )
            .expect("parent may enter closing while a leaf already owns ordinary exit presence")
    });
    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&first_parent, parent_closing, window, cx)
            .expect("parent exit should atomically adopt an already-closing descendant")
    });
    assert_eq!(
        snapshot_layer(cx, &view, "topology-child").phase(),
        OverlayLayerPhase::Hidden
    );
    let stale_child_finish = cx.update_window_entity(&view, |probe, window, cx| {
        probe.runtime.finish_exit(&child, child_closing, window, cx)
    });
    assert!(
        stale_child_finish.is_err(),
        "the adopted descendant exit callback must become stale"
    );
    assert_eq!(
        snapshot_layer(cx, &view, "first-parent").phase(),
        OverlayLayerPhase::Hidden
    );
}
