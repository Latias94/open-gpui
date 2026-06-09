use crate::{
    DockViewportAdapter, DockViewportHit, DockViewportHitCandidate, DockViewportTargetContext,
    viewport_target_resolver::choose_viewport_target,
};
use open_gpui::{Pixels, Point};

impl DockViewportAdapter {
    /// Finds the registered viewport containing a screen point.
    pub fn hit_test_screen(&self, position: Point<Pixels>) -> Option<DockViewportHit> {
        self.hit_test_screen_with_context(position, &DockViewportTargetContext::new())
    }

    /// Finds the registered viewport containing a screen point using platform arbitration inputs.
    pub fn hit_test_screen_with_context(
        &self,
        position: Point<Pixels>,
        context: &DockViewportTargetContext,
    ) -> Option<DockViewportHit> {
        self.resolve_viewport_target(position, context)
            .map(DockViewportHitCandidate::into_hit)
    }

    /// Resolves a registered viewport target using explicit platform arbitration inputs.
    pub fn resolve_viewport_target(
        &self,
        position: Point<Pixels>,
        context: &DockViewportTargetContext,
    ) -> Option<DockViewportHitCandidate> {
        let hits = self.viewport_hits(position);
        choose_viewport_target(hits, context)
    }

    fn viewport_hits(&self, position: Point<Pixels>) -> Vec<DockViewportHitCandidate> {
        self.spaces()
            .into_iter()
            .filter_map(|space| {
                let window = self.snapshot(&space)?.window;
                let host_position = self.screen_to_host(&space, position)?;
                Some(DockViewportHitCandidate {
                    space,
                    window,
                    host_position,
                })
            })
            .collect()
    }
}
