use open_gpui::WindowId;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DockViewportWindowOpenAttemptKey {
    window_id: WindowId,
    ownership_generation: u64,
}

impl DockViewportWindowOpenAttemptKey {
    pub(crate) fn window_id(self) -> WindowId {
        self.window_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DockViewportWindowRetirementKey {
    window_id: WindowId,
    ownership_generation: u64,
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
    pub(crate) fn begin_open_attempt(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockViewportWindowOpenAttemptKey> {
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
                state: DockViewportWindowOwnershipState::Opening,
            },
        );
        Some(DockViewportWindowOpenAttemptKey {
            window_id,
            ownership_generation,
        })
    }

    pub(crate) fn claim_open_attempt(&mut self, key: DockViewportWindowOpenAttemptKey) -> bool {
        let Some(record) = self.windows.get_mut(&key.window_id) else {
            return false;
        };
        if record.generation != key.ownership_generation
            || record.state != DockViewportWindowOwnershipState::Opening
        {
            return false;
        }
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
        })
    }

    pub(crate) fn retire_claimed_open_attempt(
        &mut self,
        key: DockViewportWindowOpenAttemptKey,
    ) -> Option<DockViewportWindowRetirementKey> {
        let record = self.windows.get_mut(&key.window_id)?;
        if record.generation != key.ownership_generation
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
        })
    }

    pub(crate) fn register_runtime_window(&mut self, window_id: WindowId) {
        let generation = self.next_generation();
        self.windows.insert(
            window_id,
            DockViewportWindowOwnershipRecord {
                generation,
                state: DockViewportWindowOwnershipState::Owned,
            },
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

    fn next_generation(&mut self) -> u64 {
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("dock viewport window ownership generation overflow");
        self.next_generation
    }
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
