use crate::surface::{DockSurfaceTransactionId, window_session::DockSurfaceWindowSessionLease};
use open_gpui::EntityId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DockViewportRuntimeLineage {
    Unmanaged,
    Surface(DockSurfaceWindowSessionLease),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockViewportRuntimeWorkContext {
    lineage: DockViewportRuntimeLineage,
    surface_transaction: Option<DockSurfaceTransactionId>,
}

impl DockViewportRuntimeWorkContext {
    pub(crate) const fn new(
        lineage: DockViewportRuntimeLineage,
        surface_transaction: Option<DockSurfaceTransactionId>,
    ) -> Self {
        Self {
            lineage,
            surface_transaction,
        }
    }

    pub(crate) const fn lineage(self) -> DockViewportRuntimeLineage {
        self.lineage
    }

    pub(crate) const fn surface_transaction(self) -> Option<DockSurfaceTransactionId> {
        self.surface_transaction
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockViewportRuntimeAuthority {
    Unmanaged,
    Surface {
        authority: EntityId,
        state: DockViewportSurfaceAdmissionState,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockViewportSurfaceAdmissionState {
    Vacant,
    Active(DockSurfaceWindowSessionLease),
    Frozen(DockSurfaceWindowSessionLease),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockViewportRuntimeLineageActivationOutcome {
    Activated,
    AlreadyActive,
    StaleAuthority,
    DifferentActiveGeneration,
    UnmanagedRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockViewportRuntimeLineageFreezeOutcome {
    Frozen,
    AlreadyFrozen,
    StaleLease,
    UnmanagedRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockViewportRuntimeAdmission {
    authority: DockViewportRuntimeAuthority,
}

impl DockViewportRuntimeAdmission {
    pub(crate) const fn unmanaged() -> Self {
        Self {
            authority: DockViewportRuntimeAuthority::Unmanaged,
        }
    }

    pub(crate) const fn surface(authority: EntityId) -> Self {
        Self {
            authority: DockViewportRuntimeAuthority::Surface {
                authority,
                state: DockViewportSurfaceAdmissionState::Vacant,
            },
        }
    }

    pub(crate) fn activate_surface(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        prior_generation_empty: bool,
    ) -> DockViewportRuntimeLineageActivationOutcome {
        match &mut self.authority {
            DockViewportRuntimeAuthority::Unmanaged => {
                DockViewportRuntimeLineageActivationOutcome::UnmanagedRuntime
            }
            DockViewportRuntimeAuthority::Surface { authority, state } => {
                if *authority != lease.authority() {
                    return DockViewportRuntimeLineageActivationOutcome::StaleAuthority;
                }
                match state {
                    DockViewportSurfaceAdmissionState::Active(current) if *current == lease => {
                        DockViewportRuntimeLineageActivationOutcome::AlreadyActive
                    }
                    DockViewportSurfaceAdmissionState::Active(_) => {
                        DockViewportRuntimeLineageActivationOutcome::DifferentActiveGeneration
                    }
                    DockViewportSurfaceAdmissionState::Frozen(current)
                        if lease.generation() <= current.generation() =>
                    {
                        DockViewportRuntimeLineageActivationOutcome::DifferentActiveGeneration
                    }
                    DockViewportSurfaceAdmissionState::Frozen(_) if !prior_generation_empty => {
                        DockViewportRuntimeLineageActivationOutcome::DifferentActiveGeneration
                    }
                    DockViewportSurfaceAdmissionState::Vacant
                    | DockViewportSurfaceAdmissionState::Frozen(_) => {
                        *state = DockViewportSurfaceAdmissionState::Active(lease);
                        DockViewportRuntimeLineageActivationOutcome::Activated
                    }
                }
            }
        }
    }

    pub(crate) fn freeze_surface(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
    ) -> DockViewportRuntimeLineageFreezeOutcome {
        match &mut self.authority {
            DockViewportRuntimeAuthority::Unmanaged => {
                DockViewportRuntimeLineageFreezeOutcome::UnmanagedRuntime
            }
            DockViewportRuntimeAuthority::Surface { authority, state } => {
                if *authority != lease.authority() {
                    return DockViewportRuntimeLineageFreezeOutcome::StaleLease;
                }
                match state {
                    DockViewportSurfaceAdmissionState::Active(current) if *current == lease => {
                        *state = DockViewportSurfaceAdmissionState::Frozen(lease);
                        DockViewportRuntimeLineageFreezeOutcome::Frozen
                    }
                    DockViewportSurfaceAdmissionState::Frozen(current) if *current == lease => {
                        DockViewportRuntimeLineageFreezeOutcome::AlreadyFrozen
                    }
                    DockViewportSurfaceAdmissionState::Vacant
                    | DockViewportSurfaceAdmissionState::Active(_)
                    | DockViewportSurfaceAdmissionState::Frozen(_) => {
                        DockViewportRuntimeLineageFreezeOutcome::StaleLease
                    }
                }
            }
        }
    }

    pub(crate) const fn frozen_surface_lease(self) -> Option<DockSurfaceWindowSessionLease> {
        match self.authority {
            DockViewportRuntimeAuthority::Surface {
                state: DockViewportSurfaceAdmissionState::Frozen(lease),
                ..
            } => Some(lease),
            DockViewportRuntimeAuthority::Unmanaged
            | DockViewportRuntimeAuthority::Surface {
                state:
                    DockViewportSurfaceAdmissionState::Vacant
                    | DockViewportSurfaceAdmissionState::Active(_),
                ..
            } => None,
        }
    }

    pub(crate) const fn default_lineage(self) -> Option<DockViewportRuntimeLineage> {
        match self.authority {
            DockViewportRuntimeAuthority::Unmanaged => Some(DockViewportRuntimeLineage::Unmanaged),
            DockViewportRuntimeAuthority::Surface {
                state: DockViewportSurfaceAdmissionState::Active(lease),
                ..
            } => Some(DockViewportRuntimeLineage::Surface(lease)),
            DockViewportRuntimeAuthority::Surface {
                state:
                    DockViewportSurfaceAdmissionState::Vacant
                    | DockViewportSurfaceAdmissionState::Frozen(_),
                ..
            } => None,
        }
    }

    pub(crate) fn admits(self, lineage: DockViewportRuntimeLineage) -> bool {
        match (self.authority, lineage) {
            (DockViewportRuntimeAuthority::Unmanaged, DockViewportRuntimeLineage::Unmanaged) => {
                true
            }
            (
                DockViewportRuntimeAuthority::Surface {
                    state: DockViewportSurfaceAdmissionState::Active(current),
                    ..
                },
                DockViewportRuntimeLineage::Surface(candidate),
            ) => current == candidate,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::window_session::{
        DockSurfaceWindowSession, DockSurfaceWindowSessionShutdownReason,
        DockSurfaceWindowSessionTerminalDisposition,
    };
    use open_gpui::WindowId;

    fn active_lease(
        session: &mut DockSurfaceWindowSession,
        anchor: WindowId,
    ) -> DockSurfaceWindowSessionLease {
        let opening = session
            .reserve_opening()
            .expect("the session should reserve");
        session
            .commit_opening(opening, anchor)
            .expect("the session should activate")
    }

    #[test]
    fn surface_runtime_never_admits_unmanaged_or_stale_lineage() {
        let authority = EntityId::from(11);
        let mut session = DockSurfaceWindowSession::new(authority);
        let g1 = active_lease(&mut session, WindowId::from(7));
        let mut admission = DockViewportRuntimeAdmission::surface(authority);

        assert_eq!(admission.default_lineage(), None);
        assert!(!admission.admits(DockViewportRuntimeLineage::Unmanaged));
        assert_eq!(
            admission.activate_surface(g1, true),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        assert!(admission.admits(DockViewportRuntimeLineage::Surface(g1)));
        assert!(!admission.admits(DockViewportRuntimeLineage::Unmanaged));

        let mut foreign = DockSurfaceWindowSession::new(EntityId::from(12));
        let foreign = active_lease(&mut foreign, WindowId::from(9));
        assert!(!admission.admits(DockViewportRuntimeLineage::Surface(foreign)));
        assert_eq!(
            admission.activate_surface(foreign, true),
            DockViewportRuntimeLineageActivationOutcome::StaleAuthority
        );
    }

    #[test]
    fn frozen_generation_rejects_work_until_exact_reopen() {
        let authority = EntityId::from(21);
        let mut session = DockSurfaceWindowSession::new(authority);
        let anchor = WindowId::from(17);
        let g1 = active_lease(&mut session, anchor);
        let mut admission = DockViewportRuntimeAdmission::surface(authority);
        assert_eq!(
            admission.activate_surface(g1, true),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        assert_eq!(
            admission.freeze_surface(g1),
            DockViewportRuntimeLineageFreezeOutcome::Frozen
        );
        assert!(!admission.admits(DockViewportRuntimeLineage::Surface(g1)));
        assert_eq!(
            admission.activate_surface(g1, true),
            DockViewportRuntimeLineageActivationOutcome::DifferentActiveGeneration
        );

        session.begin_shutdown(
            g1,
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
            [anchor],
        );
        session.mark_runtime_empty(g1);
        session.settle_terminal(
            g1,
            anchor,
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        );
        session.complete_shutdown(g1);
        let g2 = active_lease(&mut session, WindowId::from(18));

        assert_eq!(
            admission.activate_surface(g2, true),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        assert!(admission.admits(DockViewportRuntimeLineage::Surface(g2)));
        assert!(!admission.admits(DockViewportRuntimeLineage::Surface(g1)));
    }
}
