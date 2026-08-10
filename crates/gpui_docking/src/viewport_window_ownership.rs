use crate::{
    DockViewportRuntimeLineage,
    surface::live_undock::DockLiveUndockOpeningKey,
    surface::window_session::{
        DockSurfaceWindowSessionLease, DockSurfaceWindowSessionOpeningToken,
    },
};
use open_gpui::{AnyWindowHandle, WindowId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockViewportWindowAuthority {
    Unmanaged,
    SurfaceOpening(DockSurfaceWindowSessionOpeningToken),
    Surface(DockSurfaceWindowSessionLease),
}

impl DockViewportWindowAuthority {
    pub(crate) const fn active_lineage(self) -> Option<DockViewportRuntimeLineage> {
        match self {
            Self::Unmanaged => Some(DockViewportRuntimeLineage::Unmanaged),
            Self::Surface(lease) => Some(DockViewportRuntimeLineage::Surface(lease)),
            Self::SurfaceOpening(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockViewportWindowRole {
    PrimaryAnchor,
    ProvisionalViewport(DockLiveUndockOpeningKey),
    ManagedViewport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DockViewportWindowOpenAttemptKey {
    window_id: WindowId,
    ownership_generation: u64,
    authority: DockViewportWindowAuthority,
}

impl DockViewportWindowOpenAttemptKey {
    pub(crate) fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn active_lineage(self) -> Option<DockViewportRuntimeLineage> {
        self.authority.active_lineage()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DockViewportWindowRetirementKey {
    window_id: WindowId,
    ownership_generation: u64,
    authority: DockViewportWindowAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DockProvisionalWindowOwnershipKey {
    window_id: WindowId,
    ownership_generation: u64,
    opening: DockLiveUndockOpeningKey,
}

#[derive(Clone, Debug)]
pub(crate) struct DockPreparedProvisionalWindowPromotion {
    key: DockProvisionalWindowOwnershipKey,
    lease: DockSurfaceWindowSessionLease,
}

impl DockProvisionalWindowOwnershipKey {
    pub(crate) const fn window_id(self) -> WindowId {
        self.window_id
    }
}

impl DockViewportWindowRetirementKey {
    pub(crate) fn window_id(self) -> WindowId {
        self.window_id
    }

    #[cfg(test)]
    pub(crate) fn for_test(window_id: WindowId) -> Self {
        Self {
            window_id,
            ownership_generation: 1,
            authority: DockViewportWindowAuthority::Unmanaged,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockViewportWindowOwnershipState {
    Opening,
    Provisional,
    Owned,
    Retired { close_settled: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DockViewportWindowOwnershipRecord {
    generation: u64,
    window: AnyWindowHandle,
    authority: DockViewportWindowAuthority,
    role: DockViewportWindowRole,
    state: DockViewportWindowOwnershipState,
}

#[derive(Debug, Default)]
pub(crate) struct DockViewportWindowOwnership {
    next_generation: u64,
    windows: HashMap<WindowId, DockViewportWindowOwnershipRecord>,
    render_passthrough_windows: HashSet<WindowId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportWindowRetirement {
    RetiredNow(DockViewportWindowRetirementKey),
    AlreadyRetired(DockViewportWindowRetirementKey),
    Unowned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportProvisionalOpenAttemptCompletion {
    Admitted(DockProvisionalWindowOwnershipKey),
    RetirementRequired(DockProvisionalWindowOwnershipKey),
    ShutdownOwned(DockProvisionalWindowOwnershipKey),
    Stale,
}

impl DockViewportProvisionalOpenAttemptCompletion {
    pub(crate) const fn is_admitted(self) -> bool {
        matches!(self, Self::Admitted(_))
    }

    #[cfg(test)]
    pub(crate) const fn admitted_for_test(
        window_id: WindowId,
        opening: DockLiveUndockOpeningKey,
    ) -> Self {
        Self::Admitted(DockProvisionalWindowOwnershipKey {
            window_id,
            ownership_generation: 1,
            opening,
        })
    }
}

impl DockViewportWindowRetirement {
    pub(crate) fn key(self) -> Option<DockViewportWindowRetirementKey> {
        match self {
            Self::RetiredNow(key) | Self::AlreadyRetired(key) => Some(key),
            Self::Unowned => None,
        }
    }

    #[cfg(test)]
    fn changed(self) -> bool {
        matches!(self, Self::RetiredNow(_))
    }
}

impl DockViewportWindowOwnership {
    pub(crate) fn status(
        &self,
    ) -> crate::viewport_runtime_status::DockViewportWindowOwnershipStatus {
        let mut status = crate::viewport_runtime_status::DockViewportWindowOwnershipStatus {
            owned_window_count: self.windows.len(),
            ..Default::default()
        };
        for record in self.windows.values() {
            match record.state {
                DockViewportWindowOwnershipState::Opening => {
                    status.opening_window_count += 1;
                }
                DockViewportWindowOwnershipState::Provisional => {
                    status.opening_window_count += 1;
                }
                DockViewportWindowOwnershipState::Owned => {
                    status.active_window_count += 1;
                }
                DockViewportWindowOwnershipState::Retired { .. } => {
                    status.retiring_window_count += 1;
                }
            }
        }
        status
    }

    pub(crate) fn begin_open_attempt_with_authority(
        &mut self,
        window: AnyWindowHandle,
        authority: DockViewportWindowAuthority,
        role: DockViewportWindowRole,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        let window_id = window.window_id();
        if self.windows.get(&window_id).is_some_and(|record| {
            matches!(
                record.state,
                DockViewportWindowOwnershipState::Opening
                    | DockViewportWindowOwnershipState::Provisional
                    | DockViewportWindowOwnershipState::Owned
            )
        }) {
            return None;
        }
        let ownership_generation = self.next_generation();
        self.windows.insert(
            window_id,
            DockViewportWindowOwnershipRecord {
                generation: ownership_generation,
                window,
                authority,
                role,
                state: DockViewportWindowOwnershipState::Opening,
            },
        );
        Some(DockViewportWindowOpenAttemptKey {
            window_id,
            ownership_generation,
            authority,
        })
    }

    #[cfg(test)]
    pub(crate) fn begin_open_attempt(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        self.begin_open_attempt_with_authority(
            test_window(window_id),
            DockViewportWindowAuthority::Unmanaged,
            DockViewportWindowRole::ManagedViewport,
        )
    }

    pub(crate) fn claim_open_attempt(&mut self, key: DockViewportWindowOpenAttemptKey) -> bool {
        let Some(record) = self.windows.get_mut(&key.window_id) else {
            return false;
        };
        if record.generation != key.ownership_generation
            || record.authority != key.authority
            || record.state != DockViewportWindowOwnershipState::Opening
        {
            return false;
        }
        record.state = DockViewportWindowOwnershipState::Owned;
        true
    }

    pub(crate) fn begin_provisional_open_attempt(
        &mut self,
        window: AnyWindowHandle,
        opening: DockLiveUndockOpeningKey,
        shutdown_owned: bool,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
        let attempt = self.begin_open_attempt_with_authority(
            window,
            DockViewportWindowAuthority::Surface(opening.lease()),
            DockViewportWindowRole::ProvisionalViewport(opening),
        )?;
        if shutdown_owned {
            let record = self
                .windows
                .get_mut(&attempt.window_id)
                .expect("a newly registered provisional opening must remain exact");
            record.state = DockViewportWindowOwnershipState::Retired {
                close_settled: true,
            };
        }
        Some(attempt)
    }

    pub(crate) fn complete_provisional_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        opening: DockLiveUndockOpeningKey,
        admit: bool,
    ) -> DockViewportProvisionalOpenAttemptCompletion {
        let Some(record) = self.windows.get_mut(&key.window_id) else {
            return DockViewportProvisionalOpenAttemptCompletion::Stale;
        };
        let authority = DockViewportWindowAuthority::Surface(opening.lease());
        if record.generation != key.ownership_generation
            || key.authority != authority
            || record.authority != authority
            || record.role != DockViewportWindowRole::ProvisionalViewport(opening)
        {
            return DockViewportProvisionalOpenAttemptCompletion::Stale;
        }
        let ownership = DockProvisionalWindowOwnershipKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            opening,
        };
        match record.state {
            DockViewportWindowOwnershipState::Opening if admit => {
                record.state = DockViewportWindowOwnershipState::Provisional;
                DockViewportProvisionalOpenAttemptCompletion::Admitted(ownership)
            }
            DockViewportWindowOwnershipState::Opening => {
                record.state = DockViewportWindowOwnershipState::Retired {
                    close_settled: false,
                };
                DockViewportProvisionalOpenAttemptCompletion::RetirementRequired(ownership)
            }
            DockViewportWindowOwnershipState::Retired {
                close_settled: false,
            } => DockViewportProvisionalOpenAttemptCompletion::RetirementRequired(ownership),
            DockViewportWindowOwnershipState::Retired {
                close_settled: true,
            } => DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(ownership),
            DockViewportWindowOwnershipState::Provisional
            | DockViewportWindowOwnershipState::Owned => {
                DockViewportProvisionalOpenAttemptCompletion::Stale
            }
        }
    }

    pub(crate) fn retire_provisional_window(
        &mut self,
        key: DockProvisionalWindowOwnershipKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let record = self.windows.get_mut(&key.window_id)?;
        let authority = DockViewportWindowAuthority::Surface(key.opening.lease());
        if record.generation != key.ownership_generation
            || record.authority != authority
            || record.role != DockViewportWindowRole::ProvisionalViewport(key.opening)
            || record.state != DockViewportWindowOwnershipState::Provisional
        {
            return None;
        }
        record.state = DockViewportWindowOwnershipState::Retired {
            close_settled: false,
        };
        Some(DockViewportWindowRetirementKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            authority,
        })
    }

    pub(crate) fn provisional_window_retirement(
        &self,
        key: DockProvisionalWindowOwnershipKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let record = self.windows.get(&key.window_id)?;
        let authority = DockViewportWindowAuthority::Surface(key.opening.lease());
        if record.generation != key.ownership_generation
            || record.authority != authority
            || record.role != DockViewportWindowRole::ProvisionalViewport(key.opening)
            || !matches!(
                record.state,
                DockViewportWindowOwnershipState::Retired {
                    close_settled: false
                }
            )
        {
            return None;
        }
        Some(DockViewportWindowRetirementKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            authority,
        })
    }

    pub(crate) fn provisional_window_is_shutdown_owned(
        &self,
        key: DockProvisionalWindowOwnershipKey,
    ) -> bool {
        let authority = DockViewportWindowAuthority::Surface(key.opening.lease());
        self.windows.get(&key.window_id).is_some_and(|record| {
            record.generation == key.ownership_generation
                && record.authority == authority
                && record.role == DockViewportWindowRole::ProvisionalViewport(key.opening)
                && matches!(
                    record.state,
                    DockViewportWindowOwnershipState::Retired {
                        close_settled: true
                    }
                )
        })
    }

    pub(crate) fn reclaim_shutdown_owned_provisional_window(
        &mut self,
        key: DockProvisionalWindowOwnershipKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let authority = DockViewportWindowAuthority::Surface(key.opening.lease());
        let record = self.windows.get_mut(&key.window_id)?;
        if record.generation != key.ownership_generation
            || record.authority != authority
            || record.role != DockViewportWindowRole::ProvisionalViewport(key.opening)
            || !matches!(
                record.state,
                DockViewportWindowOwnershipState::Retired {
                    close_settled: true
                }
            )
        {
            return None;
        }
        record.state = DockViewportWindowOwnershipState::Retired {
            close_settled: false,
        };
        Some(DockViewportWindowRetirementKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            authority,
        })
    }

    pub(crate) fn prepare_provisional_window_promotion(
        &self,
        window_id: WindowId,
        opening: DockLiveUndockOpeningKey,
    ) -> Option<DockPreparedProvisionalWindowPromotion> {
        let record = self.windows.get(&window_id)?;
        if record.authority != DockViewportWindowAuthority::Surface(opening.lease())
            || record.role != DockViewportWindowRole::ProvisionalViewport(opening)
            || record.state != DockViewportWindowOwnershipState::Provisional
        {
            return None;
        }
        Some(DockPreparedProvisionalWindowPromotion {
            key: DockProvisionalWindowOwnershipKey {
                window_id,
                ownership_generation: record.generation,
                opening,
            },
            lease: opening.lease(),
        })
    }

    pub(crate) fn can_commit_provisional_window_promotion(
        &self,
        prepared: &DockPreparedProvisionalWindowPromotion,
    ) -> bool {
        self.windows
            .get(&prepared.key.window_id)
            .is_some_and(|record| {
                record.generation == prepared.key.ownership_generation
                    && record.authority == DockViewportWindowAuthority::Surface(prepared.lease)
                    && record.role
                        == DockViewportWindowRole::ProvisionalViewport(prepared.key.opening)
                    && record.state == DockViewportWindowOwnershipState::Provisional
            })
    }

    pub(crate) fn commit_provisional_window_promotion(
        &mut self,
        prepared: DockPreparedProvisionalWindowPromotion,
    ) {
        if self.has_committed_provisional_window_promotion(&prepared) {
            return;
        }
        assert!(
            self.can_commit_provisional_window_promotion(&prepared),
            "prepared provisional ownership must remain exact until commit"
        );
        let record = self
            .windows
            .get_mut(&prepared.key.window_id)
            .expect("prepared provisional ownership must remain registered until commit");
        record.role = DockViewportWindowRole::ManagedViewport;
        record.state = DockViewportWindowOwnershipState::Owned;
    }

    pub(crate) fn has_committed_provisional_window_promotion(
        &self,
        prepared: &DockPreparedProvisionalWindowPromotion,
    ) -> bool {
        self.windows
            .get(&prepared.key.window_id)
            .is_some_and(|record| {
                record.generation == prepared.key.ownership_generation
                    && record.authority == DockViewportWindowAuthority::Surface(prepared.lease)
                    && record.role == DockViewportWindowRole::ManagedViewport
                    && record.state == DockViewportWindowOwnershipState::Owned
            })
    }

    pub(crate) fn promote_primary_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        lease: DockSurfaceWindowSessionLease,
    ) -> bool {
        let Some(record) = self.windows.get_mut(&key.window_id) else {
            return false;
        };
        if record.generation != key.ownership_generation
            || record.authority != key.authority
            || record.role != DockViewportWindowRole::PrimaryAnchor
            || record.state != DockViewportWindowOwnershipState::Opening
        {
            return false;
        }
        let DockViewportWindowAuthority::SurfaceOpening(opening) = record.authority else {
            return false;
        };
        if !lease.activates(opening, key.window_id) {
            return false;
        }
        record.authority = DockViewportWindowAuthority::Surface(lease);
        record.state = DockViewportWindowOwnershipState::Owned;
        true
    }

    pub(crate) fn is_opening(&self, window_id: WindowId) -> bool {
        self.windows
            .get(&window_id)
            .is_some_and(|record| record.state == DockViewportWindowOwnershipState::Opening)
    }

    pub(crate) fn abort_open_attempt(&mut self, key: DockViewportWindowOpenAttemptKey) -> bool {
        if !self.windows.get(&key.window_id).is_some_and(|record| {
            record.generation == key.ownership_generation
                && record.authority == key.authority
                && record.state == DockViewportWindowOwnershipState::Opening
        }) {
            return false;
        }
        self.windows.remove(&key.window_id);
        self.render_passthrough_windows.remove(&key.window_id);
        true
    }

    pub(crate) fn abort_provisional_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
        opening: DockLiveUndockOpeningKey,
    ) -> bool {
        let authority = DockViewportWindowAuthority::Surface(opening.lease());
        if !self.windows.get(&key.window_id).is_some_and(|record| {
            record.generation == key.ownership_generation
                && key.authority == authority
                && record.authority == authority
                && record.role == DockViewportWindowRole::ProvisionalViewport(opening)
                && matches!(
                    record.state,
                    DockViewportWindowOwnershipState::Opening
                        | DockViewportWindowOwnershipState::Retired { .. }
                )
        }) {
            return false;
        }
        self.windows.remove(&key.window_id);
        self.render_passthrough_windows.remove(&key.window_id);
        true
    }

    pub(crate) fn retire_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let record = self.windows.get_mut(&key.window_id)?;
        if record.generation != key.ownership_generation
            || record.authority != key.authority
            || record.state != DockViewportWindowOwnershipState::Opening
        {
            return None;
        }
        record.state = DockViewportWindowOwnershipState::Retired {
            close_settled: false,
        };
        Some(DockViewportWindowRetirementKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            authority: key.authority,
        })
    }

    pub(crate) fn retire_claimed_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let record = self.windows.get_mut(&key.window_id)?;
        if record.generation != key.ownership_generation
            || record.authority != key.authority
            || record.state != DockViewportWindowOwnershipState::Owned
        {
            return None;
        }
        record.state = DockViewportWindowOwnershipState::Retired {
            close_settled: false,
        };
        Some(DockViewportWindowRetirementKey {
            window_id: key.window_id,
            ownership_generation: key.ownership_generation,
            authority: key.authority,
        })
    }

    pub(crate) fn register_runtime_window_with_lineage(
        &mut self,
        window: AnyWindowHandle,
        lineage: DockViewportRuntimeLineage,
        role: DockViewportWindowRole,
    ) {
        let window_id = window.window_id();
        let authority = match lineage {
            DockViewportRuntimeLineage::Unmanaged => DockViewportWindowAuthority::Unmanaged,
            DockViewportRuntimeLineage::Surface(lease) => {
                DockViewportWindowAuthority::Surface(lease)
            }
        };
        let generation = self.next_generation();
        self.windows.insert(
            window_id,
            DockViewportWindowOwnershipRecord {
                generation,
                window,
                authority,
                role,
                state: DockViewportWindowOwnershipState::Owned,
            },
        );
    }

    #[cfg(test)]
    pub(crate) fn register_runtime_window(&mut self, window_id: WindowId) {
        self.register_runtime_window_with_lineage(
            test_window(window_id),
            DockViewportRuntimeLineage::Unmanaged,
            DockViewportWindowRole::ManagedViewport,
        );
    }

    pub(crate) fn retire_window(&mut self, window_id: WindowId) -> DockViewportWindowRetirement {
        let Some(record) = self.windows.get_mut(&window_id) else {
            return DockViewportWindowRetirement::Unowned;
        };
        self.render_passthrough_windows.remove(&window_id);
        let key = DockViewportWindowRetirementKey {
            window_id,
            ownership_generation: record.generation,
            authority: record.authority,
        };
        match record.state {
            DockViewportWindowOwnershipState::Opening
            | DockViewportWindowOwnershipState::Provisional
            | DockViewportWindowOwnershipState::Owned => {
                record.state = DockViewportWindowOwnershipState::Retired {
                    close_settled: false,
                };
                DockViewportWindowRetirement::RetiredNow(key)
            }
            DockViewportWindowOwnershipState::Retired { .. } => {
                DockViewportWindowRetirement::AlreadyRetired(key)
            }
        }
    }

    pub(crate) fn settle_retirement(&mut self, key: DockViewportWindowRetirementKey) -> bool {
        let Some(record) = self.windows.get_mut(&key.window_id) else {
            return false;
        };
        if record.generation != key.ownership_generation {
            return false;
        }
        if record.authority != key.authority {
            return false;
        }
        let DockViewportWindowOwnershipState::Retired { close_settled } = &mut record.state else {
            return false;
        };
        if *close_settled {
            return false;
        }
        *close_settled = true;
        true
    }

    pub(crate) fn is_retired(&self, window_id: WindowId) -> bool {
        self.windows.get(&window_id).is_some_and(|record| {
            matches!(
                record.state,
                DockViewportWindowOwnershipState::Retired { .. }
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn is_owned(&self, window_id: WindowId) -> bool {
        self.windows
            .get(&window_id)
            .is_some_and(|record| record.state == DockViewportWindowOwnershipState::Owned)
    }

    pub(crate) fn record_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.render_passthrough_windows.insert(window_id)
    }

    pub(crate) fn take_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.render_passthrough_windows.remove(&window_id)
    }

    pub(crate) fn clear_window_state(&mut self, window_id: WindowId) {
        self.render_passthrough_windows.remove(&window_id);
    }

    pub(crate) fn windows_for_surface(
        &self,
        lease: DockSurfaceWindowSessionLease,
    ) -> Vec<(DockViewportWindowRole, AnyWindowHandle)> {
        self.windows
            .values()
            .filter(|record| record.authority == DockViewportWindowAuthority::Surface(lease))
            .map(|record| (record.role, record.window))
            .collect()
    }

    pub(crate) fn provisional_peer_window_ids(
        &self,
        opening: DockLiveUndockOpeningKey,
        destination: WindowId,
    ) -> Option<Vec<WindowId>> {
        let authority = DockViewportWindowAuthority::Surface(opening.lease());
        let destination_record = self.windows.get(&destination)?;
        if destination_record.authority != authority
            || destination_record.role != DockViewportWindowRole::ProvisionalViewport(opening)
            || destination_record.state != DockViewportWindowOwnershipState::Provisional
        {
            return None;
        }

        let mut peers = self
            .windows
            .iter()
            .filter_map(|(window_id, record)| {
                (*window_id != destination
                    && record.authority == authority
                    && matches!(
                        record.state,
                        DockViewportWindowOwnershipState::Provisional
                            | DockViewportWindowOwnershipState::Owned
                    ))
                .then_some(*window_id)
            })
            .collect::<Vec<_>>();
        peers.sort_unstable_by_key(WindowId::as_u64);
        Some(peers)
    }

    pub(crate) fn freeze_surface(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
    ) -> Vec<(DockViewportWindowRole, AnyWindowHandle)> {
        let authority = DockViewportWindowAuthority::Surface(lease);
        let mut windows = Vec::new();
        for record in self.windows.values_mut() {
            if record.authority != authority {
                continue;
            }
            windows.push((record.role, record.window));
            record.state = DockViewportWindowOwnershipState::Retired {
                close_settled: true,
            };
            self.render_passthrough_windows
                .remove(&record.window.window_id());
        }
        windows
    }

    pub(crate) fn settle_surface_window_terminal(
        &mut self,
        lease: DockSurfaceWindowSessionLease,
        window_id: WindowId,
    ) -> bool {
        let authority = DockViewportWindowAuthority::Surface(lease);
        if !self
            .windows
            .get(&window_id)
            .is_some_and(|record| record.authority == authority)
        {
            return false;
        }
        self.windows.remove(&window_id);
        self.render_passthrough_windows.remove(&window_id);
        true
    }

    pub(crate) fn settle_native_window_terminal(&mut self, window_id: WindowId) -> bool {
        if !self.windows.get(&window_id).is_some_and(|record| {
            matches!(
                record.state,
                DockViewportWindowOwnershipState::Retired { .. }
            )
        }) {
            return false;
        }
        self.windows.remove(&window_id);
        self.render_passthrough_windows.remove(&window_id);
        true
    }

    pub(crate) fn abort_surface_opening(
        &mut self,
        opening: DockSurfaceWindowSessionOpeningToken,
    ) -> Vec<AnyWindowHandle> {
        let authority = DockViewportWindowAuthority::SurfaceOpening(opening);
        let window_ids = self
            .windows
            .iter()
            .filter_map(|(window_id, record)| (record.authority == authority).then_some(*window_id))
            .collect::<Vec<_>>();
        let mut windows = Vec::with_capacity(window_ids.len());
        for window_id in window_ids {
            if let Some(record) = self.windows.remove(&window_id) {
                windows.push(record.window);
            }
            self.render_passthrough_windows.remove(&window_id);
        }
        windows
    }

    pub(crate) fn owns_window(
        &self,
        window_id: WindowId,
        lineage: DockViewportRuntimeLineage,
    ) -> bool {
        let authority = match lineage {
            DockViewportRuntimeLineage::Unmanaged => DockViewportWindowAuthority::Unmanaged,
            DockViewportRuntimeLineage::Surface(lease) => {
                DockViewportWindowAuthority::Surface(lease)
            }
        };
        self.windows.get(&window_id).is_some_and(|record| {
            record.authority == authority && record.state == DockViewportWindowOwnershipState::Owned
        })
    }

    fn next_generation(&mut self) -> u64 {
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("dock viewport window ownership generation overflow");
        self.next_generation
    }
}

#[cfg(test)]
fn test_window(window_id: WindowId) -> AnyWindowHandle {
    open_gpui::WindowHandle::<crate::DockHost>::new(window_id).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::window_session::DockSurfaceWindowSession;
    use open_gpui::EntityId;

    fn surface_lease(authority: u64, anchor: WindowId) -> DockSurfaceWindowSessionLease {
        let mut session = DockSurfaceWindowSession::new(EntityId::from(authority));
        let opening = session
            .reserve_opening()
            .expect("surface test lease should reserve");
        session
            .commit_opening(opening, anchor)
            .expect("surface test lease should activate")
    }

    #[test]
    fn runtime_windows_are_retired_only_after_owned_discard() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(7);

        assert!(!ownership.is_owned(window_id));
        assert_eq!(
            ownership.retire_window(window_id),
            DockViewportWindowRetirement::Unowned
        );
        assert!(!ownership.is_retired(window_id));

        ownership.register_runtime_window(window_id);
        assert!(ownership.is_owned(window_id));
        assert!(!ownership.is_retired(window_id));

        let retired = ownership.retire_window(window_id);
        assert!(retired.changed());
        let retirement_key = retired.key().expect("owned window should issue retirement");
        assert!(ownership.is_retired(window_id));
        assert!(!ownership.is_owned(window_id));
        assert_eq!(
            ownership.retire_window(window_id),
            DockViewportWindowRetirement::AlreadyRetired(retirement_key)
        );

        assert!(ownership.settle_retirement(retirement_key));
        assert!(!ownership.settle_retirement(retirement_key));
        assert!(ownership.settle_native_window_terminal(window_id));
        assert!(!ownership.settle_native_window_terminal(window_id));
        assert_eq!(ownership.status().owned_window_count, 0);

        ownership.register_runtime_window(window_id);
        assert!(ownership.is_owned(window_id));
        assert!(!ownership.is_retired(window_id));
    }

    #[test]
    fn stale_retirement_cannot_close_reowned_window_with_same_id() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(8);

        ownership.register_runtime_window(window_id);
        let first_retirement = ownership
            .retire_window(window_id)
            .key()
            .expect("first generation should retire");

        ownership.register_runtime_window(window_id);

        assert!(!ownership.settle_retirement(first_retirement));
        assert!(ownership.is_owned(window_id));
    }

    #[test]
    fn older_retirements_cannot_close_latest_retired_generation() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(10);

        ownership.register_runtime_window(window_id);
        let first = ownership
            .retire_window(window_id)
            .key()
            .expect("first generation should retire");
        ownership.register_runtime_window(window_id);
        let second = ownership
            .retire_window(window_id)
            .key()
            .expect("second generation should retire");
        ownership.register_runtime_window(window_id);
        let latest = ownership
            .retire_window(window_id)
            .key()
            .expect("latest generation should retire");

        assert!(!ownership.settle_retirement(first));
        assert!(!ownership.settle_retirement(second));
        assert!(ownership.settle_retirement(latest));
        assert!(!ownership.settle_retirement(latest));
    }

    #[test]
    fn stale_open_attempt_cannot_retire_reowned_window() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(11);
        let attempt = ownership
            .begin_open_attempt(window_id)
            .expect("unowned window should begin an open attempt");

        ownership.register_runtime_window(window_id);

        assert_eq!(attempt.window_id(), window_id);
        assert!(ownership.retire_open_attempt(attempt).is_none());
        assert!(ownership.is_owned(window_id));
    }

    #[test]
    fn open_attempt_can_be_claimed_or_retired_exactly_once() {
        let mut ownership = DockViewportWindowOwnership::default();
        let claimed_window = WindowId::from(12);
        let claimed = ownership
            .begin_open_attempt(claimed_window)
            .expect("unowned window should begin an open attempt");

        assert!(ownership.claim_open_attempt(claimed));
        assert!(!ownership.claim_open_attempt(claimed));
        assert!(ownership.retire_open_attempt(claimed).is_none());
        assert!(ownership.is_owned(claimed_window));

        let retired_window = WindowId::from(13);
        let retired = ownership
            .begin_open_attempt(retired_window)
            .expect("another unowned window should begin an open attempt");
        let retirement = ownership
            .retire_open_attempt(retired)
            .expect("current open attempt should retire");

        assert!(ownership.retire_open_attempt(retired).is_none());
        assert!(ownership.settle_retirement(retirement));
        assert!(!ownership.settle_retirement(retirement));
    }

    #[test]
    fn opening_query_and_abort_are_exact_to_the_attempt_generation() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(15);
        let first = ownership
            .begin_open_attempt(window_id)
            .expect("unowned window should begin an opening generation");
        assert!(ownership.is_opening(window_id));
        assert!(ownership.abort_open_attempt(first));
        assert!(!ownership.is_opening(window_id));

        let replacement = ownership
            .begin_open_attempt(window_id)
            .expect("aborted window should accept a replacement opening generation");
        assert!(!ownership.abort_open_attempt(first));
        assert!(ownership.is_opening(window_id));
        assert!(ownership.claim_open_attempt(replacement));
        assert!(!ownership.is_opening(window_id));
    }

    #[test]
    fn status_counts_opening_active_and_retiring_handles_from_one_authority() {
        let mut ownership = DockViewportWindowOwnership::default();
        let opening_window = WindowId::from(41);
        let active_window = WindowId::from(42);
        let retiring_window = WindowId::from(43);

        ownership
            .begin_open_attempt(opening_window)
            .expect("opening handle should reserve");
        let active = ownership
            .begin_open_attempt(active_window)
            .expect("active handle should reserve");
        assert!(ownership.claim_open_attempt(active));
        ownership.register_runtime_window(retiring_window);
        assert!(ownership.retire_window(retiring_window).changed());

        let status = ownership.status();
        assert_eq!(status.owned_window_count, 3);
        assert_eq!(status.opening_window_count, 1);
        assert_eq!(status.active_window_count, 1);
        assert_eq!(status.retiring_window_count, 1);
    }

    #[test]
    fn claimed_open_attempt_can_retire_only_its_own_generation() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(14);
        let attempt = ownership
            .begin_open_attempt(window_id)
            .expect("open attempt should reserve an unowned window");

        assert!(ownership.claim_open_attempt(attempt));
        let retirement = ownership
            .retire_claimed_open_attempt(attempt)
            .expect("the claimed generation should retire exactly once");
        assert!(ownership.settle_retirement(retirement));

        let replacement = ownership
            .begin_open_attempt(window_id)
            .expect("retired window should accept a replacement generation");
        assert!(ownership.claim_open_attempt(replacement));
        assert!(
            ownership.retire_claimed_open_attempt(attempt).is_none(),
            "an old open attempt must not retire a reowned window"
        );
        assert!(ownership.is_owned(window_id));
    }

    #[test]
    fn provisional_open_attempt_is_runtime_owned_before_admission() {
        let mut ownership = DockViewportWindowOwnership::default();
        let lease = surface_lease(51, WindowId::from(510));
        let opening = DockLiveUndockOpeningKey::for_test(lease, 1);
        let window = test_window(WindowId::from(511));

        let attempt = ownership
            .begin_provisional_open_attempt(window, opening, false)
            .expect("builder-time provisional opening should reserve one runtime generation");
        assert_eq!(
            ownership.windows_for_surface(lease),
            vec![(DockViewportWindowRole::ProvisionalViewport(opening), window)]
        );

        let completion = ownership.complete_provisional_open_attempt(attempt, opening, true);
        assert!(matches!(
            completion,
            DockViewportProvisionalOpenAttemptCompletion::Admitted(ownership_key)
                if ownership_key.window_id() == window.window_id()
        ));
        assert!(
            ownership
                .prepare_provisional_window_promotion(window.window_id(), opening)
                .is_some()
        );
    }

    #[test]
    fn surface_freeze_owns_provisional_attempt_that_returns_late() {
        let mut ownership = DockViewportWindowOwnership::default();
        let lease = surface_lease(52, WindowId::from(520));
        let opening = DockLiveUndockOpeningKey::for_test(lease, 1);
        let window = test_window(WindowId::from(521));
        let attempt = ownership
            .begin_provisional_open_attempt(window, opening, false)
            .expect("builder-time provisional opening should reserve one runtime generation");

        assert_eq!(
            ownership.freeze_surface(lease),
            vec![(DockViewportWindowRole::ProvisionalViewport(opening), window)]
        );
        let completion = ownership.complete_provisional_open_attempt(attempt, opening, true);
        assert!(matches!(
            completion,
            DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(ownership_key)
                if ownership_key.window_id() == window.window_id()
        ));
        assert!(ownership.is_retired(window.window_id()));
    }

    #[test]
    fn provisional_attempt_started_after_freeze_is_shutdown_owned_and_abortable() {
        let mut ownership = DockViewportWindowOwnership::default();
        let lease = surface_lease(54, WindowId::from(540));
        let opening = DockLiveUndockOpeningKey::for_test(lease, 1);
        let window = test_window(WindowId::from(541));
        let attempt = ownership
            .begin_provisional_open_attempt(window, opening, true)
            .expect("a late builder must still reserve one exact runtime generation");

        let completion = ownership.complete_provisional_open_attempt(attempt, opening, false);
        assert!(matches!(
            completion,
            DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(ownership_key)
                if ownership_key.window_id() == window.window_id()
        ));
        assert!(ownership.abort_provisional_open_attempt(attempt, opening));
        assert!(ownership.windows_for_surface(lease).is_empty());
    }

    #[test]
    fn rejected_provisional_attempt_retires_the_exact_generation() {
        let mut ownership = DockViewportWindowOwnership::default();
        let lease = surface_lease(53, WindowId::from(530));
        let opening = DockLiveUndockOpeningKey::for_test(lease, 1);
        let window = test_window(WindowId::from(531));
        let attempt = ownership
            .begin_provisional_open_attempt(window, opening, false)
            .expect("builder-time provisional opening should reserve one runtime generation");

        let completion = ownership.complete_provisional_open_attempt(attempt, opening, false);
        let DockViewportProvisionalOpenAttemptCompletion::RetirementRequired(ownership_key) =
            completion
        else {
            panic!("rejected provisional opening must enter exact retirement");
        };
        let retirement = ownership
            .provisional_window_retirement(ownership_key)
            .expect("rejected provisional opening must retain its exact retirement ticket");
        assert!(ownership.settle_retirement(retirement));
        assert!(!ownership.settle_retirement(retirement));
    }

    #[test]
    fn provisional_peer_window_ids_include_only_exact_live_surface_peers() {
        let mut ownership = DockViewportWindowOwnership::default();
        let lease = surface_lease(55, WindowId::from(550));
        let other_lease = surface_lease(56, WindowId::from(560));
        let destination_opening = DockLiveUndockOpeningKey::for_test(lease, 1);
        let peer_opening = DockLiveUndockOpeningKey::for_test(lease, 2);
        let opening_only = DockLiveUndockOpeningKey::for_test(lease, 3);

        let anchor = test_window(WindowId::from(550));
        let managed = test_window(WindowId::from(552));
        ownership.register_runtime_window_with_lineage(
            anchor,
            DockViewportRuntimeLineage::Surface(lease),
            DockViewportWindowRole::PrimaryAnchor,
        );
        ownership.register_runtime_window_with_lineage(
            managed,
            DockViewportRuntimeLineage::Surface(lease),
            DockViewportWindowRole::ManagedViewport,
        );

        let peer = test_window(WindowId::from(551));
        let peer_attempt = ownership
            .begin_provisional_open_attempt(peer, peer_opening, false)
            .expect("same-surface provisional peer should reserve");
        assert!(matches!(
            ownership.complete_provisional_open_attempt(peer_attempt, peer_opening, true),
            DockViewportProvisionalOpenAttemptCompletion::Admitted(_)
        ));

        let destination = test_window(WindowId::from(553));
        let destination_attempt = ownership
            .begin_provisional_open_attempt(destination, destination_opening, false)
            .expect("destination provisional should reserve");
        let destination_key = match ownership.complete_provisional_open_attempt(
            destination_attempt,
            destination_opening,
            true,
        ) {
            DockViewportProvisionalOpenAttemptCompletion::Admitted(key) => key,
            outcome => panic!("destination should be admitted, got {outcome:?}"),
        };

        let opening_window = test_window(WindowId::from(554));
        ownership
            .begin_provisional_open_attempt(opening_window, opening_only, false)
            .expect("opening-only provisional should reserve");
        let retired = test_window(WindowId::from(555));
        ownership.register_runtime_window_with_lineage(
            retired,
            DockViewportRuntimeLineage::Surface(lease),
            DockViewportWindowRole::ManagedViewport,
        );
        assert!(ownership.retire_window(retired.window_id()).changed());
        ownership.register_runtime_window_with_lineage(
            test_window(WindowId::from(556)),
            DockViewportRuntimeLineage::Surface(other_lease),
            DockViewportWindowRole::ManagedViewport,
        );
        ownership.register_runtime_window(WindowId::from(557));

        assert_eq!(
            ownership.provisional_peer_window_ids(destination_opening, destination.window_id(),),
            Some(vec![
                anchor.window_id(),
                peer.window_id(),
                managed.window_id()
            ])
        );
        assert!(
            ownership
                .provisional_peer_window_ids(
                    DockLiveUndockOpeningKey::for_test(lease, 99),
                    destination.window_id(),
                )
                .is_none(),
            "a stale opening identity must not inherit the destination peer band"
        );

        let prepared = ownership
            .prepare_provisional_window_promotion(destination.window_id(), destination_opening)
            .expect("exact destination provisional should prepare promotion");
        ownership.commit_provisional_window_promotion(prepared);
        assert!(
            ownership
                .provisional_peer_window_ids(destination_opening, destination_key.window_id())
                .is_none(),
            "a committed destination is no longer a provisional peer-band authority"
        );
    }

    #[test]
    fn clearing_window_state_removes_render_passthrough_record() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(9);

        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        assert!(!ownership.record_render_passthrough_pointer_input(window_id));
        assert!(ownership.take_render_passthrough_pointer_input(window_id));
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));

        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        ownership.clear_window_state(window_id);
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));

        ownership.register_runtime_window(window_id);
        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        assert!(matches!(
            ownership.retire_window(window_id),
            DockViewportWindowRetirement::RetiredNow(_)
        ));
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));
    }
}
