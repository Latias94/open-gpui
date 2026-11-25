#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameLinkGeneration(u64);

#[derive(Default)]
pub(super) struct FrameLinkState {
    generation: u64,
    attached: bool,
    updating: bool,
    woken: bool,
}

impl FrameLinkState {
    pub(super) fn attach(&mut self) -> FrameLinkGeneration {
        self.retire();
        self.attached = true;
        FrameLinkGeneration(self.generation)
    }

    pub(super) fn retire(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("AppKit frame link generation exhausted");
        self.attached = false;
        self.updating = false;
        self.woken = false;
    }

    pub(super) fn is_attached(&self) -> bool {
        self.attached
    }

    pub(super) fn is_woken(&self) -> bool {
        self.woken
    }

    fn is_current(&self, generation: FrameLinkGeneration) -> bool {
        self.attached && generation.0 == self.generation
    }

    pub(super) fn wake(&mut self) {
        self.woken = true;
    }

    pub(super) fn begin_frame(&mut self, generation: FrameLinkGeneration) -> bool {
        if !self.is_current(generation) || self.updating {
            return false;
        }
        self.updating = true;
        self.woken = false;
        true
    }

    pub(super) fn end_frame(
        &mut self,
        generation: FrameLinkGeneration,
        retry_pending: bool,
    ) -> bool {
        if !self.is_current(generation) || !self.updating {
            return false;
        }
        self.updating = false;
        !retry_pending && !self.woken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_pause_preserves_wakes_during_the_update() {
        let mut state = FrameLinkState::default();
        let generation = state.attach();
        assert!(state.is_attached());
        assert!(state.begin_frame(generation));
        assert!(!state.end_frame(generation, true));
        assert!(state.begin_frame(generation));
        state.wake();
        assert!(state.is_woken());
        assert!(state.is_woken());
        assert!(!state.end_frame(generation, false));
        state.wake();
        assert!(state.begin_frame(generation));
        assert!(!state.is_woken());
        assert!(state.end_frame(generation, false));
        assert!(state.begin_frame(generation));
        assert!(state.end_frame(generation, false));
    }

    #[test]
    fn retirement_rejects_late_and_nested_updates() {
        let mut state = FrameLinkState::default();
        let first = state.attach();
        assert!(state.begin_frame(first));
        assert!(!state.begin_frame(first));
        state.retire();
        assert!(!state.is_attached());
        assert!(!state.is_woken());
        assert!(!state.is_current(first));
        assert!(!state.begin_frame(first));
        assert!(!state.end_frame(first, false));
        let second = state.attach();
        assert_ne!(first, second);
        assert!(state.is_current(second));
        assert!(state.begin_frame(second));
        assert!(!state.end_frame(first, false));
        state.wake();
        assert!(!state.end_frame(second, false));
        assert!(state.begin_frame(second));
        assert!(state.end_frame(second, false));
    }
}
