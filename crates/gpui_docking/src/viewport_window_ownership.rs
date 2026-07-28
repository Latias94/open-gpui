use crate::{
    DockViewportRuntimeLineage,
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
                DockViewportWindowOwnershipState::Opening | DockViewportWindowOwnershipState::Owned
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
            DockViewportWindowOwnershipState::Opening | DockViewportWindowOwnershipState::Owned => {
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
