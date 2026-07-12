use super::*;

#[open_gpui::test]
fn rebind_can_change_ownership_and_clears_controlled_intent(cx: &mut open_gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let binding = register_layer(
        cx,
        &view,
        controlled_registration(
            "ownership-transition",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            events.clone(),
        ),
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::EscapeKey, window, cx)
            .expect("controlled close intent should succeed");
    });
    assert_eq!(
        snapshot_layer(cx, &view, "ownership-transition").phase(),
        OverlayLayerPhase::CloseRequested,
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .rebind_layer(
                &binding,
                uncontrolled_registration(
                    "ownership-transition",
                    OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
                ),
                window,
                cx,
            )
            .expect("controlled layer should transition to uncontrolled ownership");
    });
    let uncontrolled = snapshot_layer(cx, &view, "ownership-transition");
    assert_eq!(uncontrolled.phase(), OverlayLayerPhase::Open);
    assert_eq!(uncontrolled.pending_intent(), None);

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .rebind_layer(
                &binding,
                controlled_registration(
                    "ownership-transition",
                    OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
                    events.clone(),
                ),
                window,
                cx,
            )
            .expect("uncontrolled layer should transition back to controlled ownership");
    });
    assert_eq!(
        snapshot_layer(cx, &view, "ownership-transition").phase(),
        OverlayLayerPhase::Open,
    );
}

#[open_gpui::test]
fn uncontrolled_commit_precedes_observer_and_reentrant_open_wins_focus(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let owner_open = Rc::new(Cell::new(true));
    let commits = Rc::new(RefCell::new(Vec::new()));
    let observations = Rc::new(RefCell::new(Vec::new()));
    let binding_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));

    let binding = cx.update_window_entity(&view, |probe, window, cx| {
        let runtime_for_observer = probe.runtime.clone();
        let owner_for_commit = owner_open.clone();
        let owner_for_observer = owner_open.clone();
        let commits_for_callback = commits.clone();
        let observations_for_callback = observations.clone();
        let slot_for_callback = binding_slot.clone();
        let registration = OverlayLayerRegistration::new(
            "reentrant",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            OverlayOwnership::Uncontrolled,
        )
        .focus_mode(OverlayFocusMode::Modal)
        .uncontrolled_commit(move |open, _, _| {
            owner_for_commit.set(open);
            commits_for_callback.borrow_mut().push(open);
        })
        .on_open_change(move |intent, window, cx| {
            let desired_open = intent.desired_open();
            let phase = runtime_for_observer
                .snapshot(window, cx)
                .expect("observer should use its window runtime")
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == "reentrant")
                .expect("reentrant layer should remain registered")
                .phase();
            observations_for_callback.borrow_mut().push((
                desired_open,
                owner_for_observer.get(),
                phase,
            ));
            if !desired_open {
                let binding = slot_for_callback
                    .borrow()
                    .clone()
                    .expect("binding should exist before a close callback");
                runtime_for_observer
                    .request_open_change(&binding, true, DismissReason::Programmatic, window, cx)
                    .expect("reentrant reopen should succeed");
            }
        });
        let binding = probe
            .runtime
            .register_layer(registration, window, cx)
            .expect("reentrant layer should register");
        binding_slot.replace(Some(binding.clone()));
        probe.add_layer(binding.clone());
        cx.notify();
        binding
    });
    settle_focus_claims(cx);
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:reentrant:surface"));

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
            .expect("close request should dispatch callbacks");
    });
    settle_focus_claims(cx);

    assert_eq!(commits.borrow().as_slice(), &[false, true]);
    assert_eq!(
        observations.borrow().as_slice(),
        &[
            (false, false, OverlayLayerPhase::Closing),
            (true, true, OverlayLayerPhase::Open),
        ]
    );
    assert_eq!(
        snapshot_layer(cx, &view, "reentrant").phase(),
        OverlayLayerPhase::Open
    );
    assert!(cx.debug_selector_is_focused("window-overlay-runtime:reentrant:surface"));
}

#[open_gpui::test]
fn subtree_close_dispatches_topmost_first_and_skips_reentrantly_stale_commits(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let trace = Rc::new(RefCell::new(Vec::<String>::new()));
    let root_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));
    let lower_slot = Rc::new(RefCell::new(None::<OverlayLayerBinding>));

    let root = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "dispatch-root",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let trace = trace.clone();
            move |open, _, _| trace.borrow_mut().push(format!("root-commit:{open}"))
        })
        .on_open_change({
            let trace = trace.clone();
            move |intent, _, _| {
                trace
                    .borrow_mut()
                    .push(format!("root-observer:{}", intent.desired_open()))
            }
        }),
    );
    root_slot.replace(Some(root.clone()));

    let lower = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "dispatch-lower",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("dispatch-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let trace = trace.clone();
            move |open, _, _| trace.borrow_mut().push(format!("lower-commit:{open}"))
        })
        .on_open_change({
            let trace = trace.clone();
            move |intent, _, _| {
                trace
                    .borrow_mut()
                    .push(format!("lower-observer:{}", intent.desired_open()))
            }
        }),
    );
    lower_slot.replace(Some(lower));

    let runtime = cx.update_window_entity(&view, |probe, _, _| probe.runtime.clone());
    register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "dispatch-top",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            OverlayOwnership::Uncontrolled,
        )
        .parent("dispatch-root")
        .focus_mode(OverlayFocusMode::None)
        .uncontrolled_commit({
            let runtime = runtime.clone();
            let root_slot = root_slot.clone();
            let lower_slot = lower_slot.clone();
            let trace = trace.clone();
            move |open, window, cx| {
                trace.borrow_mut().push(format!("top-commit:{open}"));
                if open {
                    return;
                }
                let root = root_slot
                    .borrow()
                    .clone()
                    .expect("root binding should exist before subtree dispatch");
                runtime
                    .request_open_change(&root, true, DismissReason::Programmatic, window, cx)
                    .expect("topmost commit should be able to reopen the root");
                let lower = lower_slot
                    .borrow()
                    .clone()
                    .expect("lower binding should exist before subtree dispatch");
                runtime
                    .request_open_change(&lower, true, DismissReason::Programmatic, window, cx)
                    .expect("topmost commit should be able to reopen a lower sibling");
            }
        })
        .on_open_change({
            let trace = trace.clone();
            move |intent, _, _| {
                trace
                    .borrow_mut()
                    .push(format!("top-observer:{}", intent.desired_open()))
            }
        }),
    );

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&root, false, DismissReason::Programmatic, window, cx)
            .expect("root close should build one subtree dispatch plan");
    });

    assert_eq!(
        trace.borrow().as_slice(),
        [
            "top-commit:false",
            "root-commit:true",
            "root-observer:true",
            "lower-commit:true",
            "lower-observer:true",
            "top-observer:false",
        ]
    );
    assert_eq!(
        snapshot_layer(cx, &view, "dispatch-root").phase(),
        OverlayLayerPhase::Open
    );
    assert_eq!(
        snapshot_layer(cx, &view, "dispatch-lower").phase(),
        OverlayLayerPhase::Open
    );
    assert_eq!(
        snapshot_layer(cx, &view, "dispatch-top").phase(),
        OverlayLayerPhase::Closing
    );
}

#[open_gpui::test]
fn closing_modal_is_inert_but_blocks_old_bounds_and_reopen_rejects_stale_finish(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let binding = register_layer(
        cx,
        &view,
        OverlayLayerRegistration::new(
            "closing-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            OverlayOwnership::Uncontrolled,
        )
        .uncontrolled_commit(|_, _, _| {})
        .on_open_change({
            let events = events.clone();
            move |intent, _, _| events.borrow_mut().push(intent.desired_open())
        }),
    );
    set_inside_region(
        cx,
        &view,
        "closing-modal",
        "old-surface",
        rect(40.0, 40.0, 120.0, 120.0),
    );

    let closing_generation = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, false, DismissReason::Programmatic, window, cx)
            .expect("modal should enter closing")
    });
    let closing = snapshot_layer(cx, &view, "closing-modal");
    assert_eq!(closing.phase(), OverlayLayerPhase::Closing);
    assert_eq!(closing.presence(), OverlayPresence::closing());
    assert!(!closing.keyboard_eligible());
    assert!(closing.modal_pointer_barrier());

    let old_surface_hit = cx.simulate_event_with_dispatch_snapshot(mouse_down(80.0, 80.0));
    assert!(old_surface_hit.default_prevented());
    assert!(old_surface_hit.propagation_stopped());
    cx.simulate_keystrokes("escape");
    assert_eq!(events.borrow().as_slice(), &[false]);

    let reopened_generation = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&binding, true, DismissReason::Programmatic, window, cx)
            .expect("modal should reopen")
    });
    assert!(reopened_generation.get() > closing_generation.get());
    let stale_finish = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&binding, closing_generation, window, cx)
    });
    assert_eq!(
        stale_finish.unwrap_err(),
        WindowOverlayRuntimeError::StaleGeneration(OverlayLayerId::new("closing-modal"))
    );
    assert_eq!(
        snapshot_layer(cx, &view, "closing-modal").phase(),
        OverlayLayerPhase::Open
    );
}

#[open_gpui::test]
fn uncontrolled_root_finalizes_forced_controlled_descendant_despite_owner_refusal(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(RuntimeProbe::new);
    draw(cx);
    let root = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "forced-root",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    let descendant_events = Rc::new(RefCell::new(Vec::new()));
    let descendant = register_layer(
        cx,
        &view,
        controlled_registration(
            "forced-controlled-descendant",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            descendant_events.clone(),
        )
        .parent("forced-root")
        .focus_mode(OverlayFocusMode::Modal),
    );
    settle_focus_claims(cx);

    let root_generation = cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .request_open_change(&root, false, DismissReason::Programmatic, window, cx)
            .expect("uncontrolled ancestor should force its present subtree closed")
    });
    let root_closing = snapshot_layer(cx, &view, "forced-root");
    let descendant_closing = snapshot_layer(cx, &view, "forced-controlled-descendant");
    assert_eq!(root_closing.phase(), OverlayLayerPhase::Closing);
    assert_eq!(descendant_closing.phase(), OverlayLayerPhase::Closing);
    assert_eq!(descendant_closing.pending_open(), Some(false));
    assert_eq!(
        descendant_closing.pending_intent(),
        Some(DismissReason::Programmatic)
    );
    assert_eq!(descendant_events.borrow().as_slice(), &[false]);

    cx.update_window_entity(&view, |probe, window, cx| {
        probe
            .runtime
            .finish_exit(&root, root_generation, window, cx)
            .expect("root exit should atomically finalize forced descendants");
    });
    let root_hidden = snapshot_layer(cx, &view, "forced-root");
    let descendant_hidden = snapshot_layer(cx, &view, "forced-controlled-descendant");
    assert_eq!(root_hidden.phase(), OverlayLayerPhase::Hidden);
    assert_eq!(descendant_hidden.phase(), OverlayLayerPhase::Hidden);
    assert!(!root_hidden.modal_pointer_barrier());
    assert!(!descendant_hidden.modal_pointer_barrier());
    let underlay_dispatch = cx.simulate_event_with_dispatch_snapshot(mouse_down(700.0, 450.0));
    assert!(!underlay_dispatch.default_prevented());
    assert!(!underlay_dispatch.propagation_stopped());

    settle_focus_claims(cx);
    unregister_layer(cx, &view, "forced-controlled-descendant");
    unregister_layer(cx, &view, "forced-root");
    let remounted_root = register_layer(
        cx,
        &view,
        uncontrolled_registration(
            "forced-root",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::hidden()),
        )
        .focus_mode(OverlayFocusMode::Modal),
    );
    let remounted_descendant = register_layer(
        cx,
        &view,
        controlled_registration(
            "forced-controlled-descendant",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::hidden()),
            Rc::new(RefCell::new(Vec::new())),
        )
        .parent("forced-root")
        .focus_mode(OverlayFocusMode::Modal),
    );
    assert_eq!(remounted_root.lease().layer_id().as_str(), "forced-root");
    assert_eq!(
        remounted_descendant.lease().layer_id().as_str(),
        "forced-controlled-descendant"
    );
    assert_ne!(descendant.lease(), remounted_descendant.lease());
}
