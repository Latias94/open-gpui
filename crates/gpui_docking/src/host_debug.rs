use crate::{
    DockHost,
    debug::{DockDebugRegion, DockVisualAffordanceDebugSummary},
};
use open_gpui::WindowId;

impl DockHost {
    /// Returns a compact summary of the last rendered visual affordance scene.
    pub(crate) fn visual_affordance_debug_summary(&self) -> DockVisualAffordanceDebugSummary {
        let motion_state = self
            .visual_affordance_transition_executor_for_debug()
            .current_state_for_debug()
            .map(|state| format!("{state:?}"));
        DockVisualAffordanceDebugSummary::from_scene(
            self.last_visual_affordance_scene(),
            motion_state,
        )
    }

    pub(crate) fn publish_visual_affordance_debug_summary(&self, window_id: WindowId) {
        self.viewport_runtime().record_visual_affordance_status(
            self.space().clone(),
            window_id,
            self.visual_affordance_debug_summary(),
        );
    }

    pub(crate) fn clear_visual_affordance_debug_summary(&self, window_id: WindowId) {
        self.viewport_runtime()
            .clear_visual_affordance_status(self.space(), window_id);
    }

    /// Returns a debug selector emitted for a test region during the most recent render.
    #[cfg(test)]
    pub(crate) fn debug_selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.debug_instrumentation().selector(region)
    }

    pub(crate) fn clear_debug_selectors(&mut self) {
        #[cfg(test)]
        self.debug_instrumentation_mut().clear();
    }

    pub(crate) fn record_debug_selector(
        &mut self,
        region: DockDebugRegion,
        selector: String,
    ) -> String {
        #[cfg(test)]
        {
            if self.debug_recording_suppression_depth > 0 {
                return selector;
            }
            self.debug_instrumentation_mut().record(region, selector)
        }
        #[cfg(not(test))]
        {
            let _ = region;
            selector
        }
    }

    pub(crate) fn with_debug_selector_recording_suppressed<R>(
        &mut self,
        render: impl FnOnce(&mut Self) -> R,
    ) -> R {
        #[cfg(test)]
        {
            self.debug_recording_suppression_depth += 1;
            let result = render(self);
            self.debug_recording_suppression_depth -= 1;
            result
        }
        #[cfg(not(test))]
        {
            render(self)
        }
    }
}
