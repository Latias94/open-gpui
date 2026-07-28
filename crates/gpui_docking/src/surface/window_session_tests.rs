use super::window_session::{
    DockSurfacePrimaryWindowOpenConflict, DockSurfacePrimaryWindowOpenOutcome,
    DockSurfacePrimaryWindowUnavailable, DockSurfaceWindowSession,
    DockSurfaceWindowSessionBeginShutdownOutcome, DockSurfaceWindowSessionCloseDispatchOutcome,
    DockSurfaceWindowSessionCommitError, DockSurfaceWindowSessionOpeningRollbackReason,
    DockSurfaceWindowSessionPhase, DockSurfaceWindowSessionReason,
    DockSurfaceWindowSessionRollbackOutcome, DockSurfaceWindowSessionRuntimeEmptyOutcome,
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
use std::{cell::RefCell, rc::Rc};

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
        DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyClaimed
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
    assert!(
        !cx.simulate_window_close(anchor),
        "the native close must be vetoed until the surface coordinator runs"
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

    assert!(!cx.simulate_window_close(anchor));
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
    assert!(!cx.simulate_window_close(anchor));
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

    assert!(!cx.simulate_window_close(first_anchor));
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
