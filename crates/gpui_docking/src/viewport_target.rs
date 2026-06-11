use crate::{
    DockViewportAdapter, DockViewportTargetContext, DockViewportTargetHit,
    viewport_target_resolver::choose_viewport_target,
};
use open_gpui::{Pixels, Point};

impl DockViewportAdapter {
    /// Resolves a registered viewport target using explicit platform arbitration inputs.
    pub(crate) fn resolve_viewport_target(
        &self,
        position: Point<Pixels>,
        context: &DockViewportTargetContext,
    ) -> Option<DockViewportTargetHit> {
        let hits = self.viewport_hits(position);
        choose_viewport_target(hits, context)
    }

    fn viewport_hits(&self, position: Point<Pixels>) -> Vec<DockViewportTargetHit> {
        self.spaces_by_fallback_priority()
            .into_iter()
            .filter_map(|space| {
                let window = self.snapshot(&space)?.window;
                let host_position = self.screen_to_host(&space, position)?;
                Some(DockViewportTargetHit::new(space, window, host_position))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{bounds, handle, space};
    use open_gpui::{WindowBounds, point, px};

    #[test]
    fn hit_testing_uses_context_before_deterministic_space_order() {
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(alpha.clone(), alpha_window);
        adapter.register_viewport(zeta.clone(), zeta_window);

        for space in [&alpha, &zeta] {
            adapter.update_snapshot(
                space,
                None,
                WindowBounds::Windowed(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
            );
        }

        let position = point(px(120.0), px(140.0));
        assert_eq!(
            adapter
                .resolve_viewport_target(position, &DockViewportTargetContext::new())
                .map(|target| target.space().clone()),
            Some(alpha.clone()),
            "empty context uses stable space order as the final fallback"
        );
        assert_eq!(
            adapter
                .resolve_viewport_target(
                    position,
                    &DockViewportTargetContext::new().with_active_window(zeta_window),
                )
                .map(|target| target.space().clone()),
            Some(zeta.clone()),
            "active-window context should beat stable space order"
        );
        assert_eq!(
            adapter
                .resolve_viewport_target(
                    position,
                    &DockViewportTargetContext::new()
                        .with_hovered_window(alpha_window)
                        .with_active_window(zeta_window),
                )
                .map(|target| target.space().clone()),
            Some(alpha),
            "hovered-window context should beat active-window context"
        );
    }
}
