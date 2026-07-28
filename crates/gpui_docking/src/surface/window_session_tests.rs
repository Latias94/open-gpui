use super::window_session::{
    DockSurfacePrimaryWindowOpenConflict, DockSurfaceWindowSession,
    DockSurfaceWindowSessionBeginShutdownOutcome, DockSurfaceWindowSessionCommitError,
    DockSurfaceWindowSessionOpeningRollbackReason, DockSurfaceWindowSessionPhase,
    DockSurfaceWindowSessionReason, DockSurfaceWindowSessionRollbackOutcome,
    DockSurfaceWindowSessionRuntimeEmptyOutcome,
    DockSurfaceWindowSessionShutdownConvergenceOutcome, DockSurfaceWindowSessionShutdownReason,
    DockSurfaceWindowSessionTerminalAbsenceReason, DockSurfaceWindowSessionTerminalDisposition,
    DockSurfaceWindowSessionTerminalOutcome,
};
use open_gpui::{AppContext as _, EntityId, WindowId};

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
        session.settle_terminal(
            g1,
            dependent,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        ),
        DockSurfaceWindowSessionTerminalOutcome::Settled
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
            DockSurfaceWindowSessionTerminalDisposition::ConfirmedAbsent(
                DockSurfaceWindowSessionTerminalAbsenceReason::WindowDestroyed,
            ),
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
            DockSurfaceWindowSessionTerminalDisposition::ConfirmedAbsent(
                DockSurfaceWindowSessionTerminalAbsenceReason::AppShutdown,
            ),
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
