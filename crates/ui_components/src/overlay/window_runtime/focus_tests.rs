use super::*;

use open_gpui::{Context, Render, Styled, VisualContext as _, div};

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
