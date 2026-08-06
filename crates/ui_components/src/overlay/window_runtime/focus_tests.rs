use super::*;

use open_gpui::{AppContext as _, Context, Render, Styled, VisualContext as _, div};

struct FocusTargetReservationProbe {
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
    targets: OverlayFocusTargetSet,
    shared_focus: FocusHandle,
    first_focus: FocusHandle,
    second_focus: FocusHandle,
}

impl FocusTargetReservationProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        let binding = runtime
            .register_layer(
                OverlayLayerRegistration::new(
                    "focus-target-reservation-rollback",
                    OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::hidden()),
                    OverlayOwnership::Uncontrolled,
                )
                .uncontrolled_commit(|_, _, _| {}),
                window,
                cx,
            )
            .expect("focus target rollback probe should register its layer");
        Self {
            runtime,
            binding,
            targets: OverlayFocusTargetSet::default(),
            shared_focus: cx.focus_handle(),
            first_focus: cx.focus_handle(),
            second_focus: cx.focus_handle(),
        }
    }
}

impl Render for FocusTargetReservationProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[open_gpui::test]
fn failed_target_sync_releases_window_reservations_before_retry(
    cx: &mut open_gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(FocusTargetReservationProbe::new);

    let failure = cx.update_window_entity(&view, |probe, window, cx| {
        let runtime = probe.runtime.clone();
        let binding = probe.binding.clone();
        let shared_focus = probe.shared_focus.clone();
        probe.targets.sync(
            &runtime,
            &binding,
            vec![
                FocusTargetRegistration::new("first", &shared_focus),
                FocusTargetRegistration::new("second", &shared_focus),
            ],
            window,
            cx,
        )
    });
    assert!(matches!(
        failure,
        Err(WindowOverlayRuntimeError::Focus(
            FocusScopeRuntimeError::DuplicateTargetHandle(_)
        ))
    ));
    assert!(cx.update_window_entity(&view, |probe, _, _| probe.targets.leases.is_empty()));

    cx.update_window_entity(&view, |probe, window, cx| {
        let runtime = probe.runtime.clone();
        let binding = probe.binding.clone();
        let first_focus = probe.first_focus.clone();
        let second_focus = probe.second_focus.clone();
        probe
            .targets
            .sync(
                &runtime,
                &binding,
                vec![
                    FocusTargetRegistration::new("first", &first_focus),
                    FocusTargetRegistration::new("second", &second_focus),
                ],
                window,
                cx,
            )
            .expect("retry should reuse both declared IDs after compensating reservations");
        assert_eq!(
            probe
                .targets
                .leases
                .iter()
                .map(OverlayFocusTargetLease::declared_target_id)
                .collect::<Vec<_>>(),
            vec![&FocusTargetId::new("first"), &FocusTargetId::new("second")]
        );
    });
}

#[open_gpui::test]
fn stale_same_id_component_replacement_cancels_the_old_restore_claim(
    cx: &mut open_gpui::TestAppContext,
) {
    let cx = cx.add_empty_window();
    let (runtime, old_owner, replacement_owner, replacement) = cx.update(|window, cx| {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        let old_owner = cx.new(|_| ());
        let replacement_owner = cx.new(|_| ());
        let old_trigger = cx.focus_handle();
        let old = runtime
            .register_layer_for_entity_with_trigger_focus(
                OverlayLayerRegistration::new(
                    "same-id-restore-replacement",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::open(),
                    ),
                    OverlayOwnership::Uncontrolled,
                )
                .focus_restore_condition(OverlayFocusRestoreCondition::Always)
                .uncontrolled_commit(|_, _, _| {}),
                old_trigger,
                &old_owner,
                window,
                cx,
            )
            .expect("the original component authority should register");
        let frame_revision = window.rendered_frame_revision();
        runtime
            .state
            .update(cx, |state, _| {
                state.record_component_bind(old.lease(), frame_revision)
            })
            .expect("the original component bind should be current");
        let (scope, focus_runtime) = {
            let state = runtime.state.read(cx);
            (
                state.entries[old.lease().layer_id()]
                    .scope_id
                    .clone()
                    .expect("the passive layer should own a focus scope"),
                state.focus_runtime.clone(),
            )
        };

        runtime
            .apply_focus_transition(
                FocusTransition::Deactivate {
                    scope: scope.clone(),
                    restore: true,
                },
                window,
                cx,
            )
            .expect("ordinary close should queue the old restore claim");
        assert!(
            focus_runtime
                .has_pending_claim_for_scope(&scope, window, cx)
                .expect("the focus runtime should belong to this window"),
            "the replacement test must begin with an unsettled old restore claim"
        );

        assert!(
            runtime
                .replace_stale_component_subtree(
                    old.lease().layer_id(),
                    frame_revision.wrapping_add(1),
                    replacement_owner.entity_id(),
                    window,
                    cx,
                )
                .expect("same-ID replacement should retire the stale subtree")
        );
        assert!(
            !focus_runtime
                .has_pending_claim_for_scope(&scope, window, cx)
                .expect("the focus runtime should remain window-bound"),
            "replacement authority must cancel the stale restore before registering the new scope"
        );

        let replacement = runtime
            .register_layer_for_entity(
                OverlayLayerRegistration::new(
                    "same-id-restore-replacement",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::open(),
                    ),
                    OverlayOwnership::Uncontrolled,
                )
                .focus_restore_condition(OverlayFocusRestoreCondition::Always)
                .uncontrolled_commit(|_, _, _| {}),
                &replacement_owner,
                window,
                cx,
            )
            .expect("the replacement authority should reuse the stable ID");
        assert_ne!(old.lease().token, replacement.lease().token);

        (runtime, old_owner, replacement_owner, replacement)
    });

    drop(old_owner);
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            runtime
                .component_binding_status(&replacement, window, cx)
                .expect("the replacement binding should remain queryable"),
            OverlayLayerLeaseStatus::Registered {
                phase: OverlayLayerPhase::Open,
            },
            "the stale owner release must not retire the same-ID replacement"
        );
    });
    drop(replacement_owner);
}
