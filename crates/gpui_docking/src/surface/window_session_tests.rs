use super::window_session::{
    DockSurfacePrimaryWindowOpenConflict, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfacePrimaryWindowUnavailable, DockSurfaceWindowSession,
    DockSurfaceWindowSessionBeginShutdownOutcome,
    DockSurfaceWindowSessionCloseDispatchCommitOutcome,
    DockSurfaceWindowSessionCloseDispatchOutcome,
    DockSurfaceWindowSessionCloseDispatchRetryOutcome, DockSurfaceWindowSessionCommitError,
    DockSurfaceWindowSessionOpeningRollbackReason, DockSurfaceWindowSessionPhase,
    DockSurfaceWindowSessionReason, DockSurfaceWindowSessionRollbackOutcome,
    DockSurfaceWindowSessionRuntimeEmptyOutcome,
    DockSurfaceWindowSessionShutdownConvergenceOutcome, DockSurfaceWindowSessionShutdownReason,
    DockSurfaceWindowSessionTerminalDisposition, DockSurfaceWindowSessionTerminalOutcome,
};
use crate::{
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportUnavailable, DockViewportClosePolicy,
};
use open_gpui::{
    AppContext as _, Empty, EntityId, PlatformWindowCreationCapabilities, QuitMode,
    WindowCreationSupport, WindowId, WindowInitialPresentationOrder, WindowOptions, px, size,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn opening_reservation_commits_only_the_exact_token_and_anchor() {
    let mut session = DockSurfaceWindowSession::new(EntityId::from(1));

    let opening = session
        .reserve_opening()
        .expect("a vacant session should reserve its first opening generation");
    assert_eq!(opening.generation(), 1);
    assert_eq!(
        session.status().phase(),
        DockSurfaceWindowSessionPhase::Opening
    );
    assert_eq!(
        session.reserve_opening(),
        Err(DockSurfacePrimaryWindowOpenConflict::AlreadyOpening { generation: 1 })
    );

    let anchor = WindowId::from((7_u64 << 32) | 1);
    let lease = session
        .commit_opening(opening, anchor)
        .expect("the exact opening token should commit the full window id");

    assert_eq!(lease.generation(), 1);
    assert_eq!(lease.anchor(), anchor);
    assert!(session.admits(lease));
    assert_eq!(
        session.status().phase(),
        DockSurfaceWindowSessionPhase::Active
    );
    assert_eq!(session.status().anchor(), Some(anchor));
    assert_eq!(
        session.reserve_opening(),
        Err(DockSurfacePrimaryWindowOpenConflict::AlreadyActive {
            generation: 1,
            anchor,
        })
    );
}

#[test]
fn shutdown_requires_exact_terminal_convergence_before_reopening() {
    let mut session = DockSurfaceWindowSession::new(EntityId::from(1));
    let opening = session.reserve_opening().expect("G1 should reserve");
    let anchor = WindowId::from((7_u64 << 32) | 1);
    let dependent = WindowId::from((9_u64 << 32) | 2);
    let g1 = session
        .commit_opening(opening, anchor)
        .expect("G1 should activate");

    assert_eq!(
        session.begin_shutdown(
            g1,
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
            [dependent, anchor, dependent],
        ),
        DockSurfaceWindowSessionBeginShutdownOutcome::Started {
            terminal_ticket_count: 2,
        }
    );
    assert!(
        session.is_shutting_down(g1),
        "the exact lease must own its shutdown transition"
    );
    let status = session.status();
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::ShuttingDown);
    assert_eq!(status.terminal_ticket_count(), 2);
    assert_eq!(status.pending_terminal_ticket_count(), 2);
    assert_eq!(status.runtime_empty(), Some(false));
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
        ))
    );
    assert_eq!(
        session.reserve_opening(),
        Err(DockSurfacePrimaryWindowOpenConflict::NotClosed {
            generation: 1,
            pending_terminal_tickets: 2,
        })
    );

    assert_eq!(
        session.mark_runtime_empty(g1),
        DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked
    );
    assert_eq!(
        session.claim_close_dispatch(g1, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    );
    assert_eq!(
        session.claim_close_dispatch(g1, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyDispatching
    );
    assert_eq!(
        session.settle_terminal(
            g1,
            dependent,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        ),
        DockSurfaceWindowSessionTerminalOutcome::Settled
    );
    assert_eq!(
        session.claim_close_dispatch(g1, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyTerminal
    );
    assert_eq!(
        session.complete_shutdown(g1),
        DockSurfaceWindowSessionShutdownConvergenceOutcome::Waiting {
            runtime_empty: true,
            pending_terminal_tickets: 1,
        }
    );
    assert_eq!(
        session.settle_terminal(
            g1,
            anchor,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        ),
        DockSurfaceWindowSessionTerminalOutcome::Settled
    );
    assert_eq!(
        session.complete_shutdown(g1),
        DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
    );
    assert!(
        !session.is_shutting_down(g1),
        "a closed generation must reject delayed shutdown callbacks"
    );
    assert_eq!(
        session.status().phase(),
        DockSurfaceWindowSessionPhase::Closed
    );
    assert_eq!(session.status().pending_terminal_ticket_count(), 0);
    assert_eq!(session.status().runtime_empty(), Some(true));

    let replacement = session
        .reserve_opening()
        .expect("closed G1 should admit G2");
    let g2 = session
        .commit_opening(replacement, WindowId::from((11_u64 << 32) | 1))
        .expect("G2 should activate");
    assert!(!session.admits(g1));
    assert!(session.admits(g2));
    assert_eq!(
        session.settle_terminal(
            g1,
            anchor,
            DockSurfaceWindowSessionTerminalDisposition::ConfirmedAbsentAfterAppShutdown,
        ),
        DockSurfaceWindowSessionTerminalOutcome::StaleLease
    );
}

#[test]
fn close_dispatch_failure_returns_the_exact_ticket_to_pending_for_retry() {
    let mut session = DockSurfaceWindowSession::new(EntityId::from(1));
    let opening = session.reserve_opening().expect("G1 should reserve");
    let anchor = WindowId::from((7_u64 << 32) | 1);
    let dependent = WindowId::from((9_u64 << 32) | 2);
    let lease = session
        .commit_opening(opening, anchor)
        .expect("G1 should activate");
    assert!(matches!(
        session.begin_shutdown(
            lease,
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
            [dependent, anchor],
        ),
        DockSurfaceWindowSessionBeginShutdownOutcome::Started { .. }
    ));

    assert_eq!(
        session.claim_close_dispatch(lease, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    );
    assert_eq!(
        session.retry_close_dispatch(lease, dependent),
        DockSurfaceWindowSessionCloseDispatchRetryOutcome::Pending
    );
    assert_eq!(
        session.claim_close_dispatch(lease, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    );
    assert_eq!(
        session.mark_close_dispatched(lease, dependent),
        DockSurfaceWindowSessionCloseDispatchCommitOutcome::Dispatched
    );
    assert_eq!(
        session.claim_close_dispatch(lease, dependent),
        DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyDispatched
    );
    assert_eq!(
        session.settle_terminal(
            lease,
            dependent,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        ),
        DockSurfaceWindowSessionTerminalOutcome::Settled
    );

    assert_eq!(
        session.claim_close_dispatch(lease, anchor),
        DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
    );
    assert_eq!(
        session.settle_terminal(
            lease,
            anchor,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        ),
        DockSurfaceWindowSessionTerminalOutcome::Settled
    );
    assert_eq!(
        session.mark_close_dispatched(lease, anchor),
        DockSurfaceWindowSessionCloseDispatchCommitOutcome::AlreadyTerminal
    );
}

#[test]
fn opening_rollback_closes_the_generation_and_rejects_foreign_or_stale_tokens() {
    let mut session = DockSurfaceWindowSession::new(EntityId::from(1));
    let opening = session.reserve_opening().expect("G1 should reserve");
    let mut foreign_session = DockSurfaceWindowSession::new(EntityId::from(2));
    let foreign_opening = foreign_session
        .reserve_opening()
        .expect("the independent surface should also reserve G1");

    assert_eq!(
        session.commit_opening(foreign_opening, WindowId::from(1)),
        Err(DockSurfaceWindowSessionCommitError::StaleOpeningToken)
    );
    assert_eq!(
        session.rollback_opening(
            opening,
            DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed,
        ),
        DockSurfaceWindowSessionRollbackOutcome::RolledBack
    );
    assert_eq!(
        session.status().phase(),
        DockSurfaceWindowSessionPhase::Closed
    );
    assert_eq!(
        session.status().reason(),
        Some(DockSurfaceWindowSessionReason::OpeningRolledBack(
            DockSurfaceWindowSessionOpeningRollbackReason::WindowOpenFailed,
        ))
    );

    let replacement = session.reserve_opening().expect("Closed should admit G2");
    assert_eq!(replacement.generation(), 2);
    assert_eq!(
        session.rollback_opening(
            opening,
            DockSurfaceWindowSessionOpeningRollbackReason::Cancelled,
        ),
        DockSurfaceWindowSessionRollbackOutcome::StaleOpeningToken
    );
    assert_eq!(session.status().generation(), 2);
    assert_eq!(
        session.status().phase(),
        DockSurfaceWindowSessionPhase::Opening
    );
}

#[open_gpui::test]
fn surface_clones_share_one_read_only_window_session_status(cx: &mut open_gpui::TestAppContext) {
    cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("an empty surface should validate");
        let clone = surface.clone();

        let status = surface.window_session_status(cx);
        assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Vacant);
        assert_eq!(status.generation(), 0);
        assert_eq!(status.anchor(), None);
        assert_eq!(clone.window_session_status(cx), status);

        let owner = surface.owner().clone();
        let anchor = WindowId::from((17_u64 << 32) | 1);
        cx.update_entity(&owner, |owner, _| {
            let session = owner.window_session_mut();
            let opening = session.reserve_opening().expect("G1 should reserve");
            session
                .commit_opening(opening, anchor)
                .expect("G1 should activate");
        });
        let active = clone.window_session_status(cx);
        assert_eq!(active.phase(), DockSurfaceWindowSessionPhase::Active);
        assert_eq!(active.generation(), 1);
        assert_eq!(active.anchor(), Some(anchor));
    });
}

#[open_gpui::test]
fn primary_open_commits_exact_anchor_without_initial_render_registration(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("an empty surface should validate");

        let opened = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
            outcome => panic!("the first primary should open, got {outcome:?}"),
        };
        let anchor = opened.window().window_id();
        let status = surface.window_session_status(cx);
        assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Active);
        assert_eq!(status.generation(), 1);
        assert_eq!(status.anchor(), Some(anchor));
        assert_eq!(opened.generation(), 1);
        assert!(
            !surface.is_viewport_open(surface.primary_space(), cx),
            "the Opening primary's synchronous initial draw must not publish a runtime mapping"
        );

        assert_eq!(
            surface.open_primary_window(WindowOptions::default(), cx),
            DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                DockSurfacePrimaryWindowUnavailable::Conflict(
                    DockSurfacePrimaryWindowOpenConflict::AlreadyActive {
                        generation: 1,
                        anchor,
                    },
                ),
            )
        );
    });
}

#[open_gpui::test]
fn native_close_during_map_rolls_back_the_surface_opening(cx: &mut open_gpui::TestAppContext) {
    cx.close_next_window_during_map();
    let surface = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("an empty surface should validate");
        let outcome = surface.open_primary_window(WindowOptions::default(), cx);
        assert!(matches!(
            outcome,
            DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                    reason: DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening,
                    ..
                }
            )
        ));
        surface
    });

    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::OpeningRolledBack(
            DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening,
        ))
    );
}

#[open_gpui::test]
fn native_close_during_hidden_initial_presentation_rolls_back_the_surface_opening(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
        focus_on_appearing: WindowCreationSupport::Supported,
        transient_for: WindowCreationSupport::Supported,
        provisional_presentation: WindowCreationSupport::Supported,
        initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
    });
    cx.close_next_window_during_initial_presentation();
    let surface = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("an empty surface should validate");
        let outcome = surface.open_primary_window(WindowOptions::default(), cx);
        assert!(matches!(
            outcome,
            DockSurfacePrimaryWindowOpenOutcome::Unavailable(
                DockSurfacePrimaryWindowUnavailable::OpeningRolledBack {
                    reason: DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening,
                    ..
                }
            )
        ));
        surface
    });

    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::OpeningRolledBack(
            DockSurfaceWindowSessionOpeningRollbackReason::ClosedDuringOpening,
        ))
    );
}

#[open_gpui::test]
fn anchor_close_veto_force_closes_dependents_before_anchor(cx: &mut open_gpui::TestAppContext) {
    let closed = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
    });

    let (surface, anchor, dependent) = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .close_policy(DockViewportClosePolicy::Prevent)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            DockSurfaceViewportOpenOutcome::Unavailable(
                DockSurfaceViewportUnavailable::OpenFailed(message),
            ) => panic!("the dependent viewport should open: {message}"),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        (surface, anchor, dependent)
    });

    assert!(cx.windows().contains(&anchor));
    assert!(cx.windows().contains(&dependent));
    let close = cx.simulate_window_close_request(anchor);
    assert!(
        !close.native_close_allowed(),
        "the native close must be vetoed until the surface coordinator runs"
    );
    assert!(
        close.logical_window_removed(),
        "the surface coordinator must remove the anchor after the dependents converge"
    );
    assert!(
        close.native_terminal_started(),
        "the committed coordinated close must reach native terminal before it returns"
    );
    cx.update(|_| {});
    cx.run_until_parked();

    assert!(!cx.windows().contains(&dependent));
    assert!(!cx.windows().contains(&anchor));
    assert_eq!(
        closed.borrow().as_slice(),
        [dependent.window_id(), anchor.window_id()],
        "dependent windows must become terminal before the anchor"
    );
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
    assert!(!cx.did_quit(), "surface shutdown must not quit the app");

    let replacement =
        cx.update(
            |cx| match surface.open_primary_window(WindowOptions::default(), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
                outcome => panic!("a converged surface should reopen, got {outcome:?}"),
            },
        );
    assert_eq!(replacement.generation(), 2);
    let replacement_anchor = replacement.window().window_id();
    cx.update(|cx| super::handle_surface_window_closed(surface.owner(), anchor.window_id(), cx));
    let replacement_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        replacement_status.phase(),
        DockSurfaceWindowSessionPhase::Active
    );
    assert_eq!(replacement_status.generation(), 2);
    assert_eq!(replacement_status.anchor(), Some(replacement_anchor));
}

#[open_gpui::test]
fn borrow_conflicted_dependent_close_retries_after_the_window_update_boundary(
    cx: &mut open_gpui::TestAppContext,
) {
    let (surface, anchor, dependent, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the active surface should expose its exact lease")
        });
        (surface, anchor, dependent, lease)
    });

    dependent
        .update(cx, |_, _, app| {
            let effects = super::prepare_surface_shutdown(
                surface.owner(),
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                app,
            )
            .expect("the active lease should begin shutdown");
            super::apply_surface_shutdown_close_effects(surface.owner(), effects, app);
        })
        .expect("the dependent must remain available for the conflicting update");
    cx.run_until_parked();

    assert!(!cx.windows().contains(&dependent));
    assert!(!cx.windows().contains(&anchor));
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
}

#[open_gpui::test]
fn cleanup_callback_panic_closes_all_dependents_before_propagating_and_fences_reopen(
    cx: &mut open_gpui::TestAppContext,
) {
    let (surface, anchor, first_dependent, second_dependent, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let first_dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx)
        {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the first dependent viewport should open, got {outcome:?}"),
        };
        let second_dependent = match surface.open_viewport("tertiary", WindowOptions::default(), cx)
        {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the second dependent viewport should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the active surface should expose its exact lease")
        });
        (surface, anchor, first_dependent, second_dependent, lease)
    });
    cx.run_until_parked();

    let first_dependent_terminal = cx.hold_window_native_terminal(first_dependent);
    let second_dependent_terminal = cx.hold_window_native_terminal(second_dependent);
    let anchor_terminal = cx.hold_window_native_terminal(anchor);
    let owner = surface.owner().clone();
    let owner_weak = owner.downgrade();
    let activation_subscription = cx.update(|app| {
        app.update_entity(&owner, |owner, _| {
            let begin =
                owner
                    .activation_mut()
                    .begin_request(lease, owner_weak, "main".into(), |_, _| {
                        panic!("injected surface shutdown cleanup panic")
                    });
            let (_, subscription, _, settlements) = begin.into_parts();
            assert!(
                settlements.is_empty(),
                "a mounted active host should leave the activation request pending"
            );
            subscription
        })
    });

    let panic = catch_unwind(AssertUnwindSafe(|| {
        cx.update(|app| {
            let effects = super::prepare_surface_shutdown(
                surface.owner(),
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                app,
            )
            .expect("the active lease should begin shutdown");
            super::apply_surface_shutdown_close_effects(surface.owner(), effects, app);
        });
    }))
    .expect_err("the first cleanup callback panic should propagate after close effects");
    let panic_message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    assert_eq!(
        panic_message,
        Some("injected surface shutdown cleanup panic")
    );
    drop(activation_subscription);

    assert!(!cx.windows().contains(&first_dependent));
    assert!(!cx.windows().contains(&second_dependent));
    assert!(cx.windows().contains(&anchor));
    assert!(matches!(
        cx.update(|app| surface.open_primary_window(WindowOptions::default(), app)),
        DockSurfacePrimaryWindowOpenOutcome::Unavailable(
            DockSurfacePrimaryWindowUnavailable::Conflict(
                DockSurfacePrimaryWindowOpenConflict::NotClosed {
                    generation: 1,
                    pending_terminal_tickets: 3,
                }
            )
        )
    ));

    assert!(first_dependent_terminal.release());
    cx.run_until_parked();
    assert!(cx.windows().contains(&anchor));
    let waiting_for_second_dependent = cx.update(|app| surface.window_session_status(app));
    assert_eq!(
        waiting_for_second_dependent.phase(),
        DockSurfaceWindowSessionPhase::ShuttingDown
    );
    assert_eq!(
        waiting_for_second_dependent.pending_terminal_ticket_count(),
        2
    );

    assert!(second_dependent_terminal.release());
    cx.run_until_parked();
    assert!(!cx.windows().contains(&anchor));
    let waiting_for_anchor = cx.update(|app| surface.window_session_status(app));
    assert_eq!(
        waiting_for_anchor.phase(),
        DockSurfaceWindowSessionPhase::ShuttingDown
    );
    assert_eq!(waiting_for_anchor.pending_terminal_ticket_count(), 1);

    assert!(anchor_terminal.release());
    cx.run_until_parked();
    let closed = cx.update(|app| surface.window_session_status(app));
    assert_eq!(closed.phase(), DockSurfaceWindowSessionPhase::Closed);
    let reopened = cx.update(|app| surface.open_primary_window(WindowOptions::default(), app));
    assert!(matches!(
        reopened,
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) if opened.generation() == 2
    ));
}

#[open_gpui::test]
fn app_shutdown_converges_after_registry_clear_before_propagating_cleanup_panic(
    cx: &mut open_gpui::TestAppContext,
) {
    let (surface, anchor, dependent, lease) = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the active surface should expose its exact lease")
        });
        (surface, anchor, dependent, lease)
    });
    cx.run_until_parked();

    let dependent_terminal = cx.hold_window_native_terminal(dependent);
    let anchor_terminal = cx.hold_window_native_terminal(anchor);
    let owner = surface.owner().clone();
    let owner_weak = owner.downgrade();
    let activation_subscription = cx.update(|app| {
        app.update_entity(&owner, |owner, _| {
            let begin =
                owner
                    .activation_mut()
                    .begin_request(lease, owner_weak, "main".into(), |_, _| {
                        panic!("injected App shutdown cleanup panic")
                    });
            let (_, subscription, _, settlements) = begin.into_parts();
            assert!(settlements.is_empty());
            subscription
        })
    });

    let panic = catch_unwind(AssertUnwindSafe(|| cx.quit()))
        .expect_err("App shutdown should propagate the cleanup panic after convergence");
    let panic_message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    assert_eq!(panic_message, Some("injected App shutdown cleanup panic"));
    drop(activation_subscription);

    assert!(cx.windows().is_empty());
    let status = cx.update(|app| surface.window_session_status(app));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));

    assert!(dependent_terminal.release());
    assert!(anchor_terminal.release());
    cx.run_until_parked();
}

#[open_gpui::test]
fn app_shutdown_converges_every_surface_before_propagating_the_first_cleanup_panic(
    cx: &mut open_gpui::TestAppContext,
) {
    let (
        first,
        first_anchor,
        first_dependent,
        first_lease,
        second,
        second_anchor,
        second_dependent,
    ) = cx.update(|cx| {
        let first = crate::DockSurface::builder("first")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the first surface should validate");
        let first_anchor = match first.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the first primary should open, got {outcome:?}"),
        };
        let first_dependent =
            match first.open_viewport("first-secondary", WindowOptions::default(), cx) {
                DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the first dependent should open, got {outcome:?}"),
            };
        let first_lease = cx.read_entity(first.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the first surface should expose its exact lease")
        });

        let second = crate::DockSurface::builder("second")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the second surface should validate");
        let second_anchor = match second.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the second primary should open, got {outcome:?}"),
        };
        let second_dependent =
            match second.open_viewport("second-secondary", WindowOptions::default(), cx) {
                DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the second dependent should open, got {outcome:?}"),
            };

        (
            first,
            first_anchor,
            first_dependent,
            first_lease,
            second,
            second_anchor,
            second_dependent,
        )
    });
    cx.run_until_parked();

    let first_anchor_terminal = cx.hold_window_native_terminal(first_anchor);
    let first_dependent_terminal = cx.hold_window_native_terminal(first_dependent);
    let second_anchor_terminal = cx.hold_window_native_terminal(second_anchor);
    let second_dependent_terminal = cx.hold_window_native_terminal(second_dependent);
    let first_owner = first.owner().clone();
    let first_owner_weak = first_owner.downgrade();
    let activation_subscription = cx.update(|app| {
        app.update_entity(&first_owner, |owner, _| {
            let begin = owner.activation_mut().begin_request(
                first_lease,
                first_owner_weak,
                "first".into(),
                |_, _| panic!("injected first-surface App shutdown cleanup panic"),
            );
            let (_, subscription, _, settlements) = begin.into_parts();
            assert!(settlements.is_empty());
            subscription
        })
    });

    let panic = catch_unwind(AssertUnwindSafe(|| cx.quit()))
        .expect_err("App shutdown should propagate the first panic after every surface converges");
    let panic_message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    assert_eq!(
        panic_message,
        Some("injected first-surface App shutdown cleanup panic")
    );
    drop(activation_subscription);

    assert!(cx.windows().is_empty());
    let (first_status, second_status) = cx.update(|app| {
        (
            first.window_session_status(app),
            second.window_session_status(app),
        )
    });
    for status in [first_status, second_status] {
        assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
        assert_eq!(status.pending_terminal_ticket_count(), 0);
        assert_eq!(status.runtime_empty(), Some(true));
    }

    assert!(first_anchor_terminal.release());
    assert!(first_dependent_terminal.release());
    assert!(second_anchor_terminal.release());
    assert!(second_dependent_terminal.release());
    cx.run_until_parked();
}

#[open_gpui::test]
fn delayed_native_terminals_block_reopen_until_the_exact_generation_converges(
    cx: &mut open_gpui::TestAppContext,
) {
    let (surface, anchor, dependent) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        (surface, anchor, dependent)
    });
    let dependent_terminal = cx.hold_window_native_terminal(dependent);
    let anchor_terminal = cx.hold_window_native_terminal(anchor);

    let close = cx.simulate_window_close_request(anchor);
    assert!(!close.native_close_allowed());
    assert!(!close.logical_window_removed());
    assert!(!close.native_terminal_started());
    let synchronously_frozen = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        synchronously_frozen.phase(),
        DockSurfaceWindowSessionPhase::ShuttingDown,
        "the close query must freeze admission before deferred window effects run"
    );
    assert_eq!(
        synchronously_frozen.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
        ))
    );
    cx.update(|_| {});
    cx.run_until_parked();

    assert!(!cx.windows().contains(&dependent));
    assert!(cx.windows().contains(&anchor));
    let waiting_for_dependent = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        waiting_for_dependent.phase(),
        DockSurfaceWindowSessionPhase::ShuttingDown
    );
    assert_eq!(waiting_for_dependent.pending_terminal_ticket_count(), 2);
    assert_eq!(waiting_for_dependent.runtime_empty(), Some(false));
    assert!(matches!(
        cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx)),
        DockSurfacePrimaryWindowOpenOutcome::Unavailable(
            DockSurfacePrimaryWindowUnavailable::Conflict(
                DockSurfacePrimaryWindowOpenConflict::NotClosed {
                    generation: 1,
                    pending_terminal_tickets: 2,
                }
            )
        )
    ));

    assert!(dependent_terminal.release());
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    let waiting_for_anchor = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        waiting_for_anchor.phase(),
        DockSurfaceWindowSessionPhase::ShuttingDown
    );
    assert_eq!(waiting_for_anchor.pending_terminal_ticket_count(), 1);
    assert_eq!(waiting_for_anchor.runtime_empty(), Some(false));
    assert!(matches!(
        cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx)),
        DockSurfacePrimaryWindowOpenOutcome::Unavailable(
            DockSurfacePrimaryWindowUnavailable::Conflict(
                DockSurfacePrimaryWindowOpenConflict::NotClosed {
                    generation: 1,
                    pending_terminal_tickets: 1,
                }
            )
        )
    ));

    assert!(anchor_terminal.release());
    cx.run_until_parked();

    let closed = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(closed.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(closed.pending_terminal_ticket_count(), 0);
    assert_eq!(closed.runtime_empty(), Some(true));
    let reopened = cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx));
    assert!(matches!(
        reopened,
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) if opened.generation() == 2
    ));
}

#[open_gpui::test]
fn direct_anchor_destruction_defers_dependent_teardown_until_close_fanout_finishes(
    cx: &mut open_gpui::TestAppContext,
) {
    let (surface, anchor, dependent) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        (surface, anchor, dependent)
    });

    anchor
        .update(cx, |_, window, cx| window.remove_window(cx))
        .expect("the exact anchor should remain live until direct destruction");
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    assert!(
        !cx.windows().contains(&dependent),
        "the dependent close observer must run after the anchor close fanout releases its subscribers"
    );
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::AnchorDestroyed,
        ))
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));

    let reopened = cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx));
    let reopened = match reopened {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
        outcome => panic!("direct-destruction convergence should admit G2, got {outcome:?}"),
    };
    assert_eq!(reopened.generation(), 2);
    cx.run_until_parked();
    let reopened_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        reopened_status.phase(),
        DockSurfaceWindowSessionPhase::Active
    );
    assert_eq!(reopened_status.generation(), 2);
}

#[open_gpui::test]
fn facade_windows_retain_session_authority_after_application_drops_surface_handle(
    cx: &mut open_gpui::TestAppContext,
) {
    let (owner, anchor, dependent) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let owner = surface.owner().downgrade();
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let dependent = match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        (owner, anchor, dependent)
    });

    assert!(
        owner.upgrade().is_some(),
        "facade-created hosts must retain their shared session authority"
    );
    let close = cx.simulate_window_close_request(anchor);
    assert!(!close.native_close_allowed());
    assert!(close.logical_window_removed());
    cx.update(|_| {});
    cx.run_until_parked();

    assert!(!cx.windows().contains(&dependent));
    assert!(!cx.windows().contains(&anchor));
    assert!(
        owner.upgrade().is_none(),
        "the session authority should release after its last facade window closes"
    );
}

#[open_gpui::test]
fn committed_initial_presentation_failure_shuts_down_the_exact_session(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.reject_next_window_initial_presentation();
    let (surface, anchor) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should commit before presentation, got {outcome:?}"),
        };
        (surface, anchor)
    });
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::PresentationFailed,
        ))
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));

    let reopened = cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx));
    let reopened = match reopened {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
        outcome => panic!("presentation-failure convergence should admit G2, got {outcome:?}"),
    };
    assert_eq!(reopened.generation(), 2);
    cx.run_until_parked();
    let reopened_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        reopened_status.phase(),
        DockSurfaceWindowSessionPhase::Active
    );
    assert_eq!(reopened_status.generation(), 2);
}

#[open_gpui::test]
fn replacing_primary_root_cannot_detach_presentation_failure_shutdown(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.reject_next_window_initial_presentation();
    let (surface, anchor) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should commit before presentation, got {outcome:?}"),
        };
        anchor
            .update(cx, |_, window, cx| {
                window.replace_root(cx, |_, _| open_gpui::Empty);
            })
            .expect("the committed primary should allow replacing its root");
        (surface, anchor)
    });
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::PresentationFailed,
        ))
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn app_shutdown_confirms_every_surface_window_terminal_after_registry_clear(
    cx: &mut open_gpui::TestAppContext,
) {
    let surface = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(_) => {}
            outcome => panic!("the primary should open, got {outcome:?}"),
        }
        match surface.open_viewport("secondary", WindowOptions::default(), cx) {
            DockSurfaceViewportOpenOutcome::Opened(_) => {}
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        }
        surface
    });

    cx.quit();

    assert!(cx.windows().is_empty());
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::AppShutdown,
        ))
    );
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));

    let reopened = cx.update(|cx| surface.open_primary_window(WindowOptions::default(), cx));
    let reopened = match reopened {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
        outcome => panic!("post-clear convergence should admit G2, got {outcome:?}"),
    };
    assert_eq!(reopened.generation(), 2);
    cx.run_until_parked();
    let reopened_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        reopened_status.phase(),
        DockSurfaceWindowSessionPhase::Active
    );
    assert_eq!(reopened_status.generation(), 2);

    let reopened_terminal = cx.hold_window_native_terminal(reopened.window());
    cx.quit();

    assert!(cx.windows().is_empty());
    let second_shutdown_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(
        second_shutdown_status.phase(),
        DockSurfaceWindowSessionPhase::Closed
    );
    assert_eq!(second_shutdown_status.generation(), 2);
    assert_eq!(
        second_shutdown_status.reason(),
        Some(DockSurfaceWindowSessionReason::Shutdown(
            DockSurfaceWindowSessionShutdownReason::AppShutdown,
        ))
    );
    assert_eq!(second_shutdown_status.pending_terminal_ticket_count(), 0);
    assert_eq!(second_shutdown_status.runtime_empty(), Some(true));
    assert!(reopened_terminal.release());
    cx.run_until_parked();
}

#[open_gpui::test]
fn app_shutdown_rolls_back_opening_and_closes_its_provisional_anchor(
    cx: &mut open_gpui::TestAppContext,
) {
    let surface = cx.update(|cx| {
        crate::DockSurface::builder("main")
            .build(cx)
            .expect("the surface should validate")
    });
    let opening = cx.update(|cx| {
        cx.update_entity(surface.owner(), |owner, owner_cx| {
            let opening = owner
                .window_session_mut()
                .reserve_opening()
                .expect("the vacant session should reserve");
            owner_cx.notify();
            opening
        })
    });
    let provisional = cx
        .open_window(size(px(240.0), px(160.0)), |_, _| Empty)
        .into();
    let runtime = cx.update(|cx| surface.viewport_runtime(cx));
    assert!(
        runtime
            .begin_primary_anchor_open_attempt(provisional, opening)
            .is_some()
    );

    cx.quit();

    assert!(!cx.windows().contains(&provisional));
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(
        status.reason(),
        Some(DockSurfaceWindowSessionReason::OpeningRolledBack(
            DockSurfaceWindowSessionOpeningRollbackReason::AppShutdown,
        ))
    );
}

#[open_gpui::test]
fn anchor_shutdown_isolated_to_its_surface_generation(cx: &mut open_gpui::TestAppContext) {
    let (first, first_anchor, first_dependent, second, second_anchor, second_dependent) = cx
        .update(|cx| {
            cx.set_quit_mode(QuitMode::Explicit);
            let first = crate::DockSurface::builder("first")
                .allow_platform_viewports(true)
                .build(cx)
                .expect("the first surface should validate");
            let second = crate::DockSurface::builder("second")
                .allow_platform_viewports(true)
                .build(cx)
                .expect("the second surface should validate");
            let first_anchor = match first.open_primary_window(WindowOptions::default(), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the first primary should open, got {outcome:?}"),
            };
            let second_anchor = match second.open_primary_window(WindowOptions::default(), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the second primary should open, got {outcome:?}"),
            };
            let first_dependent =
                match first.open_viewport("first-secondary", WindowOptions::default(), cx) {
                    DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                    outcome => panic!("the first dependent should open, got {outcome:?}"),
                };
            let second_dependent =
                match second.open_viewport("second-secondary", WindowOptions::default(), cx) {
                    DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                    outcome => panic!("the second dependent should open, got {outcome:?}"),
                };
            (
                first,
                first_anchor,
                first_dependent,
                second,
                second_anchor,
                second_dependent,
            )
        });

    let close = cx.simulate_window_close_request(first_anchor);
    assert!(!close.native_close_allowed());
    assert!(close.logical_window_removed());
    cx.update(|_| {});
    cx.run_until_parked();

    assert!(!cx.windows().contains(&first_anchor));
    assert!(!cx.windows().contains(&first_dependent));
    assert!(cx.windows().contains(&second_anchor));
    assert!(cx.windows().contains(&second_dependent));
    assert_eq!(
        cx.update(|cx| first.window_session_status(cx).phase()),
        DockSurfaceWindowSessionPhase::Closed
    );
    assert_eq!(
        cx.update(|cx| second.window_session_status(cx).phase()),
        DockSurfaceWindowSessionPhase::Active
    );
}

#[open_gpui::test]
fn programmatic_anchor_removal_waits_for_exact_native_capture_before_dependent_teardown(
    cx: &mut open_gpui::TestAppContext,
) {
    use crate::{
        DockController, DockGraph, DockHost, DockNode, DockSpaceId, DockViewportRuntimeHandle,
        DockWorkspace, debug::DockDebugRegion, drag::DockDragPayload,
        drop_preview::DockDropRoutePreviewKind, host_test_support::*,
    };
    use open_gpui::{
        AnyWindowHandle, Bounds, DevicePixels, Modifiers, MouseButton, NativeCapturedDragPhase,
        PlatformPointerCaptureReleaseOutcome, PlatformWindowHit, PlatformWindowHitStack,
        PlatformWindowPhysicalCoverage, PlatformWindowPhysicalGeometry, PointerCancelReason,
        VisualTestContext, point,
    };

    let source_space = DockSpaceId::from("source");
    let dependent_space = DockSpaceId::from("dependent");
    let mut source_graph = DockGraph::new();
    let source_tabs = source_graph.insert_node(DockNode::Tabs {
        items: vec![item("a"), item("b")],
        selected: Some(item("a")),
    });
    let dependent_tabs = source_graph.insert_node(DockNode::Tabs {
        items: vec![item("c")],
        selected: Some(item("c")),
    });
    source_graph.set_root(source_space.clone(), source_tabs);
    source_graph.set_root(dependent_space.clone(), dependent_tabs);
    let mut source_workspace = DockWorkspace::new(source_space.clone(), source_graph);
    source_workspace
        .policy_mut()
        .set_allow_platform_viewports(true);
    source_workspace.register_panel_view(item("a"), "Panel A", test_view(cx, "A"));
    source_workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
    source_workspace.register_panel_view(item("c"), "Panel C", test_view(cx, "C"));
    let source_controller = cx.new(|_| DockController::new(source_workspace));
    let surface = cx.update(|app| crate::DockSurface::from_controller(source_controller, app));
    let (source_window, dependent_window, source_runtime, lease) = cx.update(|app| {
        let source_window =
            match surface.open_primary_window(viewport_window_options(360.0, 220.0), app) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("the source primary should open, got {outcome:?}"),
            };
        let dependent_window = match surface.open_viewport(
            dependent_space.clone(),
            viewport_window_options(360.0, 220.0),
            app,
        ) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the dependent viewport should open, got {outcome:?}"),
        };
        let source_runtime = surface.viewport_runtime(app);
        let lease = app.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the opened source surface should expose its exact active lease")
        });
        (source_window, dependent_window, source_runtime, lease)
    });

    let target_space = DockSpaceId::from("foreign-target");
    let mut target_graph = DockGraph::new();
    let target_tabs = target_graph.insert_node(DockNode::Tabs {
        items: vec![item("x")],
        selected: Some(item("x")),
    });
    target_graph.set_root(target_space.clone(), target_tabs);
    let target_workspace = workspace_with_panels(cx, target_graph, &[("x", "Panel X", "X")]);
    let target_controller = cx.new(|_| DockController::new(target_workspace));
    let target_runtime = DockViewportRuntimeHandle::new(target_controller.clone());
    let (target_window, target_host, mut target_visual) = open_controller_space_with_runtime(
        cx,
        target_controller,
        target_runtime.clone(),
        target_space.clone(),
        size(px(360.0), px(220.0)),
    );
    let target_window: AnyWindowHandle = target_window.into();
    let source_host = source_window
        .downcast::<DockHost>()
        .expect("the source primary should render DockHost")
        .root(cx)
        .expect("the source primary should expose its DockHost root");
    let mut source_visual = VisualTestContext::from_window(source_window, cx);

    let source_tab = selector_for(
        &source_visual,
        &source_host,
        DockDebugRegion::Tab {
            tabs: source_tabs,
            item: item("a"),
        },
    )
    .expect("the source tab selector should be emitted");
    let target_tabs_selector = selector_for(
        &target_visual,
        &target_host,
        DockDebugRegion::Tabs { node: target_tabs },
    )
    .expect("the foreign target tabs selector should be emitted");
    let start = debug_bounds(&mut source_visual, &source_tab).center();
    let threshold = point(start.x + px(24.0), start.y);
    let target_local = debug_bounds(&mut target_visual, &target_tabs_selector).center();
    let target_global_from_source = point(px(400.0) + target_local.x, target_local.y);

    let source_physical_bounds = Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(720), DevicePixels(440)),
    );
    let target_physical_bounds = Bounds::new(
        point(DevicePixels(800), DevicePixels(0)),
        size(DevicePixels(720), DevicePixels(440)),
    );
    cx.set_platform_window_physical_client_geometry(
        source_window,
        Some(source_physical_bounds),
        2.0,
    );
    cx.set_platform_window_physical_client_geometry(
        target_window,
        Some(target_physical_bounds),
        2.0,
    );
    let sampled_point = point(
        DevicePixels((target_global_from_source.x.as_f32() * 2.0).round() as i32),
        DevicePixels((target_global_from_source.y.as_f32() * 2.0).round() as i32),
    );
    let target_coverage = PlatformWindowPhysicalCoverage::try_new(target_physical_bounds)
        .expect("the foreign target coverage should be representable");
    let target_geometry = PlatformWindowPhysicalGeometry::try_new(target_physical_bounds, 2.0)
        .expect("the foreign target geometry should be representable");
    cx.set_platform_window_hit_stack(
        PlatformWindowHitStack::try_available(
            sampled_point,
            vec![PlatformWindowHit::RegisteredApplication {
                window: target_window,
                coverage: target_coverage,
                geometry: target_geometry,
            }],
        )
        .expect("the foreign target hit stack should be valid"),
    );

    let observed_native_events = Rc::new(RefCell::new(Vec::new()));
    let _native_observer = cx.update({
        let observed_native_events = observed_native_events.clone();
        move |app| {
            app.observe_native_captured_drag(move |event, _| {
                observed_native_events
                    .borrow_mut()
                    .push((event.generation(), event.phase()));
            })
        }
    });
    activate_window_for_pointer_input(&mut source_visual);
    source_visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
    source_visual.simulate_mouse_move(
        target_global_from_source,
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let payload =
        DockDragPayload::new_item(source_space, source_tabs, item("a"), "Panel A".to_string());
    let session = source_runtime
        .active_payload_drag_session(&payload)
        .expect("the source surface should own an active native drag session");
    assert_eq!(
        session.lineage(),
        crate::DockViewportRuntimeLineage::Surface(lease),
        "the captured route must belong to the exact surface lease being shut down"
    );
    assert!(cx.read(|app| {
        crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
    }));
    assert!(target_runtime.has_routed_drop_preview_for_drag_session(Some(&session)));
    assert!(
        selector_for(
            &VisualTestContext::from_window(target_window, cx),
            &target_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "the foreign surface should expose its rejection marker before shutdown"
    );
    let old_generation = observed_native_events
        .borrow()
        .last()
        .expect("the active native drag should publish at least one captured event")
        .0;
    let event_count_before_shutdown = observed_native_events.borrow().len();
    let release_attempts = Rc::new(Cell::new(0));
    cx.set_pointer_capture_release_callback(source_window, {
        let release_attempts = release_attempts.clone();
        move |_| {
            release_attempts.set(release_attempts.get() + 1);
            PlatformPointerCaptureReleaseOutcome::Rejected
        }
    });
    let native_terminal = cx.hold_window_native_terminal(source_window);
    cx.update(|app| {
        source_window
            .update(app, |_, window, app| window.remove_window(app))
            .expect("the anchor should remain reachable until programmatic removal commits");
        assert_eq!(
            surface.window_session_status(app).phase(),
            DockSurfaceWindowSessionPhase::ShuttingDown
        );
        assert!(
            !crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app),
            "programmatic removal must synchronously retire the exact native route"
        );
        assert_eq!(
            source_runtime.active_payload_drag_session(&payload),
            None,
            "pointer cancellation may retire logical drag state before native capture is terminal"
        );
        assert!(
            target_runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
            "foreign feedback must remain until the source native capture is terminal"
        );
        assert!(
            !app.has_active_drag(),
            "programmatic removal must synchronously clear GPUI's exact active drag"
        );
        assert!(
            dependent_window.update(app, |_, _, _| ()).is_ok(),
            "the dependent must remain live until the source native capture is terminal"
        );
    });
    assert_eq!(
        release_attempts.get(),
        1,
        "programmatic removal should attempt and reject the exact native release once"
    );
    assert_eq!(
        observed_native_events.borrow().len(),
        event_count_before_shutdown + 1,
        "shutdown must deliver one typed terminal at the completed outer App boundary"
    );
    assert_eq!(
        observed_native_events.borrow().last().copied(),
        Some((
            old_generation,
            NativeCapturedDragPhase::Cancelled(PointerCancelReason::WindowClosed),
        )),
    );
    cx.run_until_parked();
    assert_eq!(
        observed_native_events.borrow().len(),
        event_count_before_shutdown + 1,
        "parking after shutdown must not duplicate the typed terminal"
    );

    assert!(cx.read(|app| {
        !crate::native_captured_drag::has_active_native_captured_drag_route_for_test(app)
    }));
    assert_eq!(source_runtime.active_payload_drag_session(&payload), None);
    assert!(
        target_runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
        "a rejected native release must retain the frozen foreign feedback owner"
    );
    let target_before_terminal = VisualTestContext::from_window(target_window, cx);
    assert!(
        selector_for(
            &target_before_terminal,
            &target_host,
            DockDebugRegion::DropRoutePreview {
                kind: DockDropRoutePreviewKind::Rejected,
            },
        )
        .is_some(),
        "the frozen foreign marker must remain visible before capture release"
    );
    assert!(!cx.windows().contains(&source_window));
    assert!(cx.windows().contains(&dependent_window));
    assert_eq!(
        cx.update(|app| surface.window_session_status(app).phase()),
        DockSurfaceWindowSessionPhase::ShuttingDown
    );

    assert!(native_terminal.release());
    cx.run_until_parked();
    assert_eq!(
        release_attempts.get(),
        1,
        "native terminal completion must not retry an already rejected release"
    );
    assert_eq!(source_runtime.active_payload_drag_session(&payload), None);
    assert!(
        !target_runtime.has_routed_drop_preview_for_drag_session(Some(&session)),
        "the capture-terminal continuation must clear foreign feedback"
    );
    assert!(!cx.windows().contains(&dependent_window));
    assert!(!cx.windows().contains(&source_window));
    assert_eq!(
        cx.update(|app| surface.window_session_status(app).phase()),
        DockSurfaceWindowSessionPhase::Closed
    );
}
