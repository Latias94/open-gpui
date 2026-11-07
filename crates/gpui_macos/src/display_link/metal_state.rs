#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MetalFrameGeneration(u64);

#[derive(Default)]
pub(crate) struct MetalFrameState {
    generation: u64,
    attached: bool,
    updating: bool,
    woken: bool,
}

impl MetalFrameState {
    pub(crate) fn attach(&mut self) -> MetalFrameGeneration {
        self.retire();
        self.attached = true;
        MetalFrameGeneration(self.generation)
    }

    pub(crate) fn retire(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Metal frame generation exhausted");
        self.attached = false;
        self.updating = false;
        self.woken = false;
    }

    pub(crate) fn is_attached(&self) -> bool {
        self.attached
    }

    pub(crate) fn is_current(&self, generation: MetalFrameGeneration) -> bool {
        self.attached && generation.0 == self.generation
    }

    pub(crate) fn wake(&mut self) {
        self.woken = true;
    }

    pub(crate) fn begin_frame(&mut self, generation: MetalFrameGeneration) -> bool {
        if !self.is_current(generation) || self.updating {
            return false;
        }
        self.updating = true;
        self.woken = false;
        true
    }

    pub(crate) fn end_frame(
        &mut self,
        generation: MetalFrameGeneration,
        used_drawable: bool,
    ) -> bool {
        if !self.is_current(generation) || !self.updating {
            return false;
        }
        self.updating = false;
        !used_drawable && !self.woken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_pause_preserves_wakes_during_the_update() {
        let mut state = MetalFrameState::default();
        let generation = state.attach();
        assert!(state.is_attached());
        assert!(state.begin_frame(generation));
        assert!(!state.end_frame(generation, true));
        assert!(state.begin_frame(generation));
        state.wake();
        assert!(!state.end_frame(generation, false));
        state.wake();
        assert!(state.begin_frame(generation));
        assert!(state.end_frame(generation, false));
        assert!(state.begin_frame(generation));
        assert!(state.end_frame(generation, false));
    }

    #[test]
    fn retirement_rejects_late_and_nested_updates() {
        let mut state = MetalFrameState::default();
        let first = state.attach();
        assert!(state.begin_frame(first));
        assert!(!state.begin_frame(first));
        state.retire();
        assert!(!state.is_attached());
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
