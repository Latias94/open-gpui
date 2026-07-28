use open_gpui::{AnyWindowHandle, EntityId, WindowId};

/// Public lifecycle phase for one facade-managed Dock surface window session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfaceWindowSessionPhase {
    /// The surface has never reserved a primary window.
    Vacant,
    /// A primary window has been reserved but has not committed a full window identity.
    Opening,
    /// The exact committed anchor admits managed window work.
    Active,
    /// New managed work is frozen while owned windows converge to terminal state.
    ShuttingDown,
    /// The previous generation converged and a later generation may open.
    Closed,
}

/// Read-only lifecycle snapshot for one facade-managed Dock surface window session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockSurfaceWindowSessionStatus {
    phase: DockSurfaceWindowSessionPhase,
    generation: u64,
    anchor: Option<WindowId>,
    reason: Option<DockSurfaceWindowSessionReason>,
    terminal_ticket_count: usize,
    pending_terminal_ticket_count: usize,
    runtime_empty: Option<bool>,
}

impl DockSurfaceWindowSessionStatus {
    /// Returns the current session phase.
    pub const fn phase(self) -> DockSurfaceWindowSessionPhase {
        self.phase
    }

    /// Returns the latest reserved generation, or zero before the first reservation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the exact committed anchor for the current or most recently closed generation.
    pub const fn anchor(self) -> Option<WindowId> {
        self.anchor
    }

    /// Returns why an opening rolled back or an active generation began shutdown.
    pub const fn reason(self) -> Option<DockSurfaceWindowSessionReason> {
        self.reason
    }

    /// Returns the number of exact window tickets in the terminal snapshot.
    pub const fn terminal_ticket_count(self) -> usize {
        self.terminal_ticket_count
    }

    /// Returns the number of terminal window tickets that have not settled.
    pub const fn pending_terminal_ticket_count(self) -> usize {
        self.pending_terminal_ticket_count
    }

    /// Returns current-generation runtime convergence during or after shutdown.
    pub const fn runtime_empty(self) -> Option<bool> {
        self.runtime_empty
    }
}

/// Public terminal reason for a closed or shutting-down window session.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfaceWindowSessionReason {
    /// The primary window never committed and the opening reservation was rolled back.
    OpeningRolledBack(DockSurfaceWindowSessionOpeningRollbackReason),
    /// A committed anchor began deterministic shutdown.
    Shutdown(DockSurfaceWindowSessionShutdownReason),
}

/// Typed reason an opening reservation was rolled back before activation.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfaceWindowSessionOpeningRollbackReason {
    /// GPUI could not create or commit the primary window.
    WindowOpenFailed,
    /// The primary window closed synchronously while it was being created.
    ClosedDuringOpening,
    /// Initial presentation failed before the committed anchor became visible.
    PresentationFailedBeforeVisibility,
    /// Application shutdown interrupted primary creation.
    AppShutdown,
    /// The owner explicitly cancelled primary creation.
    Cancelled,
    /// Root construction or initial rendering panicked before activation.
    Panicked,
}

/// Typed reason a committed window session entered deterministic shutdown.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfaceWindowSessionShutdownReason {
    /// The current anchor received an ordinary close request.
    AnchorCloseRequested,
    /// The current anchor disappeared without completing the guarded close path.
    AnchorDestroyed,
    /// Presentation failed after the primary window identity committed.
    PresentationFailed,
    /// Application shutdown is clearing all platform windows.
    AppShutdown,
}

/// Typed reason a primary-window reservation was rejected.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DockSurfacePrimaryWindowOpenConflict {
    /// Another primary window is still being opened for this surface.
    AlreadyOpening {
        /// Generation that owns the current opening reservation.
        generation: u64,
    },
    /// This surface already has an active committed anchor.
    AlreadyActive {
        /// Generation that owns the active anchor.
        generation: u64,
        /// Exact committed anchor window.
        anchor: WindowId,
    },
    /// The preceding generation has started shutdown but has not converged to closed.
    NotClosed {
        /// Generation that still owns shutdown.
        generation: u64,
        /// Number of terminal window tickets that have not settled.
        pending_terminal_tickets: usize,
    },
}

/// Result of one facade-managed primary-window open request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DockSurfacePrimaryWindowOpenOutcome {
    /// A committed primary window activated a new session generation.
    Opened(DockSurfacePrimaryWindowOpened),
    /// The request was rejected or rolled back before activation.
    Unavailable(DockSurfacePrimaryWindowUnavailable),
}

/// A committed facade-managed primary window and its session generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockSurfacePrimaryWindowOpened {
    window: AnyWindowHandle,
    generation: u64,
}

impl DockSurfacePrimaryWindowOpened {
    pub(crate) const fn new(window: AnyWindowHandle, generation: u64) -> Self {
        Self { window, generation }
    }

    /// Returns the exact committed primary window.
    pub const fn window(self) -> AnyWindowHandle {
        self.window
    }

    /// Returns the activated window-session generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Typed reason a facade-managed primary window could not open.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DockSurfacePrimaryWindowUnavailable {
    /// Another window-session generation still owns the primary role.
    Conflict(DockSurfacePrimaryWindowOpenConflict),
    /// Window creation rolled back the exact opening reservation.
    OpeningRolledBack {
        /// Lifecycle reason recorded by the window-session authority.
        reason: DockSurfaceWindowSessionOpeningRollbackReason,
        /// Diagnostic supplied by the GPUI window creation boundary.
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockSurfaceWindowSessionOpeningToken {
    authority: EntityId,
    generation: u64,
}

impl DockSurfaceWindowSessionOpeningToken {
    #[cfg(test)]
    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockSurfaceWindowSessionLease {
    authority: EntityId,
    generation: u64,
    anchor: WindowId,
}

impl DockSurfaceWindowSessionLease {
    pub(crate) const fn authority(self) -> EntityId {
        self.authority
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn anchor(self) -> WindowId {
        self.anchor
    }

    pub(crate) fn activates(
        self,
        token: DockSurfaceWindowSessionOpeningToken,
        anchor: WindowId,
    ) -> bool {
        self.authority == token.authority
            && self.generation == token.generation
            && self.anchor == anchor
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionCommitError {
    NotOpening,
    StaleOpeningToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionRollbackOutcome {
    RolledBack,
    StaleOpeningToken,
    NotOpening,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionBeginShutdownOutcome {
    Started { terminal_ticket_count: usize },
    AlreadyShuttingDown,
    StaleLease,
    NotActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionTerminalDisposition {
    ObservedClosed,
    ConfirmedAbsentAfterAppShutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionTerminalOutcome {
    Settled,
    AlreadyTerminal,
    UnknownWindow,
    StaleLease,
    NotShuttingDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionCloseDispatchOutcome {
    Claimed,
    AlreadyClaimed,
    AlreadyTerminal,
    UnknownWindow,
    StaleLease,
    NotShuttingDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionRuntimeEmptyOutcome {
    Marked,
    AlreadyEmpty,
    StaleLease,
    NotShuttingDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockSurfaceWindowSessionShutdownConvergenceOutcome {
    Closed,
    Waiting {
        runtime_empty: bool,
        pending_terminal_tickets: usize,
    },
    StaleLease,
    NotShuttingDown,
}

#[derive(Debug)]
struct DockSurfaceWindowSessionTerminalTicket {
    window_id: WindowId,
    close_dispatched: bool,
    terminal: Option<DockSurfaceWindowSessionTerminalDisposition>,
}

impl DockSurfaceWindowSessionTerminalTicket {
    fn pending(window_id: WindowId) -> Self {
        Self {
            window_id,
            close_dispatched: false,
            terminal: None,
        }
    }
}

#[derive(Debug)]
enum DockSurfaceWindowSessionState {
    Vacant,
    Opening {
        token: DockSurfaceWindowSessionOpeningToken,
    },
    Active {
        lease: DockSurfaceWindowSessionLease,
    },
    ShuttingDown {
        lease: DockSurfaceWindowSessionLease,
        reason: DockSurfaceWindowSessionShutdownReason,
        runtime_empty: bool,
        terminal_tickets: Vec<DockSurfaceWindowSessionTerminalTicket>,
    },
    Closed {
        generation: u64,
        anchor: Option<WindowId>,
        reason: DockSurfaceWindowSessionReason,
        terminal_tickets: Vec<DockSurfaceWindowSessionTerminalTicket>,
    },
}

#[derive(Debug)]
pub(crate) struct DockSurfaceWindowSession {
    authority: EntityId,
    last_generation: u64,
    state: DockSurfaceWindowSessionState,
}

impl DockSurfaceWindowSession {
    pub(crate) fn new(authority: EntityId) -> Self {
        Self {
            authority,
            last_generation: 0,
            state: DockSurfaceWindowSessionState::Vacant,
        }
    }

    pub(crate) fn reserve_opening(
        &mut self,
    ) -> Result<DockSurfaceWindowSessionOpeningToken, DockSurfacePrimaryWindowOpenConflict> {
        match &self.state {
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Closed { .. } => {}
            DockSurfaceWindowSessionState::Opening { token } => {
                return Err(DockSurfacePrimaryWindowOpenConflict::AlreadyOpening {
                    generation: token.generation,
                });
            }
            DockSurfaceWindowSessionState::Active { lease } => {
                return Err(DockSurfacePrimaryWindowOpenConflict::AlreadyActive {
                    generation: lease.generation,
                    anchor: lease.anchor,
                });
            }
            DockSurfaceWindowSessionState::ShuttingDown {
                lease,
                terminal_tickets,
                ..
            } => {
                return Err(DockSurfacePrimaryWindowOpenConflict::NotClosed {
                    generation: lease.generation,
                    pending_terminal_tickets: terminal_tickets
                        .iter()
                        .filter(|ticket| ticket.terminal.is_none())
                        .count(),
                });
            }
        }

        self.last_generation = self
            .last_generation
            .checked_add(1)
            .expect("dock surface window-session generation space exhausted");
        let token = DockSurfaceWindowSessionOpeningToken {
            authority: self.authority,
            generation: self.last_generation,
        };
        self.state = DockSurfaceWindowSessionState::Opening { token };
        Ok(token)
    }

    pub(crate) fn commit_opening(
        &mut self,
        token: DockSurfaceWindowSessionOpeningToken,
        anchor: WindowId,
    ) -> Result<DockSurfaceWindowSessionLease, DockSurfaceWindowSessionCommitError> {
        let current = match &self.state {
            DockSurfaceWindowSessionState::Opening { token } => *token,
            _ => return Err(DockSurfaceWindowSessionCommitError::NotOpening),
        };
        if current != token {
            return Err(DockSurfaceWindowSessionCommitError::StaleOpeningToken);
        }

        let lease = DockSurfaceWindowSessionLease {
            authority: self.authority,
            generation: token.generation,
            anchor,
        };
        self.state = DockSurfaceWindowSessionState::Active { lease };
        Ok(lease)
    }

    pub(crate) fn active_lease(&self) -> Option<DockSurfaceWindowSessionLease> {
        match &self.state {
            DockSurfaceWindowSessionState::Active { lease } => Some(*lease),
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Opening { .. }
            | DockSurfaceWindowSessionState::ShuttingDown { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => None,
        }
    }

    pub(crate) fn opening_token(&self) -> Option<DockSurfaceWindowSessionOpeningToken> {
        match &self.state {
            DockSurfaceWindowSessionState::Opening { token } => Some(*token),
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Active { .. }
            | DockSurfaceWindowSessionState::ShuttingDown { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => None,
        }
    }

    pub(crate) fn active_lease_for_anchor(
        &self,
        anchor: WindowId,
    ) -> Option<DockSurfaceWindowSessionLease> {
        self.active_lease().filter(|lease| lease.anchor == anchor)
    }

    pub(crate) fn protects_anchor_from_native_close(&self, anchor: WindowId) -> bool {
        match &self.state {
            DockSurfaceWindowSessionState::Active { lease }
            | DockSurfaceWindowSessionState::ShuttingDown { lease, .. } => lease.anchor == anchor,
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Opening { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => false,
        }
    }

    pub(crate) fn shutting_down_lease_for_window(
        &self,
        window_id: WindowId,
    ) -> Option<DockSurfaceWindowSessionLease> {
        match &self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease,
                terminal_tickets,
                ..
            } if terminal_tickets
                .iter()
                .any(|ticket| ticket.window_id == window_id) =>
            {
                Some(*lease)
            }
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Opening { .. }
            | DockSurfaceWindowSessionState::Active { .. }
            | DockSurfaceWindowSessionState::ShuttingDown { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => None,
        }
    }

    pub(crate) fn shutting_down_lease(&self) -> Option<DockSurfaceWindowSessionLease> {
        match &self.state {
            DockSurfaceWindowSessionState::ShuttingDown { lease, .. } => Some(*lease),
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Opening { .. }
            | DockSurfaceWindowSessionState::Active { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => None,
        }
    }

    pub(crate) fn pending_terminal_window_ids(
        &self,
        lease: DockSurfaceWindowSessionLease,
    ) -> Option<Vec<WindowId>> {
        match &self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease: current,
                terminal_tickets,
                ..
            } if *current == lease => Some(
                terminal_tickets
                    .iter()
                    .filter(|ticket| ticket.terminal.is_none())
                    .map(|ticket| ticket.window_id)
                    .collect(),
            ),
            DockSurfaceWindowSessionState::Vacant
            | DockSurfaceWindowSessionState::Opening { .. }
            | DockSurfaceWindowSessionState::Active { .. }
            | DockSurfaceWindowSessionState::ShuttingDown { .. }
            | DockSurfaceWindowSessionState::Closed { .. } => None,
        }
    }

    pub(crate) fn rollback_opening(
        &mut self,
        token: DockSurfaceWindowSessionOpeningToken,
        reason: DockSurfaceWindowSessionOpeningRollbackReason,
    ) -> DockSurfaceWindowSessionRollbackOutcome {
        let current = match &self.state {
            DockSurfaceWindowSessionState::Opening { token } => *token,
            _ => return DockSurfaceWindowSessionRollbackOutcome::NotOpening,
        };
        if current != token {
            return DockSurfaceWindowSessionRollbackOutcome::StaleOpeningToken;
        }

        self.state = DockSurfaceWindowSessionState::Closed {
            generation: token.generation,
            anchor: None,
            reason: DockSurfaceWindowSessionReason::OpeningRolledBack(reason),
            terminal_tickets: Vec::new(),
        };
        DockSurfaceWindowSessionRollbackOutcome::RolledBack
    }

    pub(crate) fn begin_shutdown(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        reason: DockSurfaceWindowSessionShutdownReason,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> DockSurfaceWindowSessionBeginShutdownOutcome {
        match &self.state {
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease => {}
            DockSurfaceWindowSessionState::Active { .. } => {
                return DockSurfaceWindowSessionBeginShutdownOutcome::StaleLease;
            }
            DockSurfaceWindowSessionState::ShuttingDown { lease: current, .. }
                if *current == lease =>
            {
                return DockSurfaceWindowSessionBeginShutdownOutcome::AlreadyShuttingDown;
            }
            DockSurfaceWindowSessionState::ShuttingDown { .. } => {
                return DockSurfaceWindowSessionBeginShutdownOutcome::StaleLease;
            }
            _ => return DockSurfaceWindowSessionBeginShutdownOutcome::NotActive,
        }

        let mut terminal_tickets = Vec::new();
        for window_id in windows {
            if window_id != lease.anchor
                && !terminal_tickets.iter().any(
                    |ticket: &DockSurfaceWindowSessionTerminalTicket| ticket.window_id == window_id,
                )
            {
                terminal_tickets.push(DockSurfaceWindowSessionTerminalTicket::pending(window_id));
            }
        }
        terminal_tickets.push(DockSurfaceWindowSessionTerminalTicket::pending(
            lease.anchor,
        ));
        let terminal_ticket_count = terminal_tickets.len();
        self.state = DockSurfaceWindowSessionState::ShuttingDown {
            lease,
            reason,
            runtime_empty: false,
            terminal_tickets,
        };
        DockSurfaceWindowSessionBeginShutdownOutcome::Started {
            terminal_ticket_count,
        }
    }

    pub(crate) fn mark_runtime_empty(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
    ) -> DockSurfaceWindowSessionRuntimeEmptyOutcome {
        match &mut self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease: current,
                runtime_empty,
                ..
            } if *current == lease => {
                if *runtime_empty {
                    DockSurfaceWindowSessionRuntimeEmptyOutcome::AlreadyEmpty
                } else {
                    *runtime_empty = true;
                    DockSurfaceWindowSessionRuntimeEmptyOutcome::Marked
                }
            }
            DockSurfaceWindowSessionState::ShuttingDown { .. } => {
                DockSurfaceWindowSessionRuntimeEmptyOutcome::StaleLease
            }
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease => {
                DockSurfaceWindowSessionRuntimeEmptyOutcome::NotShuttingDown
            }
            _ => DockSurfaceWindowSessionRuntimeEmptyOutcome::StaleLease,
        }
    }

    pub(crate) fn settle_terminal(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        window_id: WindowId,
        disposition: DockSurfaceWindowSessionTerminalDisposition,
    ) -> DockSurfaceWindowSessionTerminalOutcome {
        match &mut self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease: current,
                terminal_tickets,
                ..
            } if *current == lease => {
                let Some(ticket) = terminal_tickets
                    .iter_mut()
                    .find(|ticket| ticket.window_id == window_id)
                else {
                    return DockSurfaceWindowSessionTerminalOutcome::UnknownWindow;
                };
                if ticket.terminal.is_some() {
                    DockSurfaceWindowSessionTerminalOutcome::AlreadyTerminal
                } else {
                    ticket.terminal = Some(disposition);
                    DockSurfaceWindowSessionTerminalOutcome::Settled
                }
            }
            DockSurfaceWindowSessionState::ShuttingDown { .. } => {
                DockSurfaceWindowSessionTerminalOutcome::StaleLease
            }
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease => {
                DockSurfaceWindowSessionTerminalOutcome::NotShuttingDown
            }
            _ => DockSurfaceWindowSessionTerminalOutcome::StaleLease,
        }
    }

    pub(crate) fn claim_close_dispatch(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        window_id: WindowId,
    ) -> DockSurfaceWindowSessionCloseDispatchOutcome {
        match &mut self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease: current,
                terminal_tickets,
                ..
            } if *current == lease => {
                let Some(ticket) = terminal_tickets
                    .iter_mut()
                    .find(|ticket| ticket.window_id == window_id)
                else {
                    return DockSurfaceWindowSessionCloseDispatchOutcome::UnknownWindow;
                };
                if ticket.terminal.is_some() {
                    DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyTerminal
                } else if ticket.close_dispatched {
                    DockSurfaceWindowSessionCloseDispatchOutcome::AlreadyClaimed
                } else {
                    ticket.close_dispatched = true;
                    DockSurfaceWindowSessionCloseDispatchOutcome::Claimed
                }
            }
            DockSurfaceWindowSessionState::ShuttingDown { .. } => {
                DockSurfaceWindowSessionCloseDispatchOutcome::StaleLease
            }
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease => {
                DockSurfaceWindowSessionCloseDispatchOutcome::NotShuttingDown
            }
            _ => DockSurfaceWindowSessionCloseDispatchOutcome::StaleLease,
        }
    }

    pub(crate) fn complete_shutdown(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
    ) -> DockSurfaceWindowSessionShutdownConvergenceOutcome {
        let (reason, terminal_tickets) = match &mut self.state {
            DockSurfaceWindowSessionState::ShuttingDown {
                lease: current,
                reason,
                runtime_empty,
                terminal_tickets,
            } if *current == lease => {
                let pending_terminal_tickets = terminal_tickets
                    .iter()
                    .filter(|ticket| ticket.terminal.is_none())
                    .count();
                if !*runtime_empty || pending_terminal_tickets != 0 {
                    return DockSurfaceWindowSessionShutdownConvergenceOutcome::Waiting {
                        runtime_empty: *runtime_empty,
                        pending_terminal_tickets,
                    };
                }
                (*reason, std::mem::take(terminal_tickets))
            }
            DockSurfaceWindowSessionState::ShuttingDown { .. } => {
                return DockSurfaceWindowSessionShutdownConvergenceOutcome::StaleLease;
            }
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease => {
                return DockSurfaceWindowSessionShutdownConvergenceOutcome::NotShuttingDown;
            }
            _ => return DockSurfaceWindowSessionShutdownConvergenceOutcome::StaleLease,
        };
        self.state = DockSurfaceWindowSessionState::Closed {
            generation: lease.generation,
            anchor: Some(lease.anchor),
            reason: DockSurfaceWindowSessionReason::Shutdown(reason),
            terminal_tickets,
        };
        DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
    }

    pub(crate) fn admits(&self, lease: DockSurfaceWindowSessionLease) -> bool {
        matches!(
            &self.state,
            DockSurfaceWindowSessionState::Active { lease: current } if *current == lease
        )
    }

    pub(crate) fn status(&self) -> DockSurfaceWindowSessionStatus {
        match &self.state {
            DockSurfaceWindowSessionState::Vacant => DockSurfaceWindowSessionStatus {
                phase: DockSurfaceWindowSessionPhase::Vacant,
                generation: self.last_generation,
                anchor: None,
                reason: None,
                terminal_ticket_count: 0,
                pending_terminal_ticket_count: 0,
                runtime_empty: None,
            },
            DockSurfaceWindowSessionState::Opening { token } => DockSurfaceWindowSessionStatus {
                phase: DockSurfaceWindowSessionPhase::Opening,
                generation: token.generation,
                anchor: None,
                reason: None,
                terminal_ticket_count: 0,
                pending_terminal_ticket_count: 0,
                runtime_empty: None,
            },
            DockSurfaceWindowSessionState::Active { lease } => DockSurfaceWindowSessionStatus {
                phase: DockSurfaceWindowSessionPhase::Active,
                generation: lease.generation,
                anchor: Some(lease.anchor),
                reason: None,
                terminal_ticket_count: 0,
                pending_terminal_ticket_count: 0,
                runtime_empty: None,
            },
            DockSurfaceWindowSessionState::ShuttingDown {
                lease,
                reason,
                runtime_empty,
                terminal_tickets,
            } => DockSurfaceWindowSessionStatus {
                phase: DockSurfaceWindowSessionPhase::ShuttingDown,
                generation: lease.generation,
                anchor: Some(lease.anchor),
                reason: Some(DockSurfaceWindowSessionReason::Shutdown(*reason)),
                terminal_ticket_count: terminal_tickets.len(),
                pending_terminal_ticket_count: terminal_tickets
                    .iter()
                    .filter(|ticket| ticket.terminal.is_none())
                    .count(),
                runtime_empty: Some(*runtime_empty),
            },
            DockSurfaceWindowSessionState::Closed {
                generation,
                anchor,
                reason,
                terminal_tickets,
            } => DockSurfaceWindowSessionStatus {
                phase: DockSurfaceWindowSessionPhase::Closed,
                generation: *generation,
                anchor: *anchor,
                reason: Some(*reason),
                terminal_ticket_count: terminal_tickets.len(),
                pending_terminal_ticket_count: 0,
                runtime_empty: Some(true),
            },
        }
    }
}
