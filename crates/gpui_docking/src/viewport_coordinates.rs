use crate::{
    DockSpaceId, DockViewportAdapter, DockViewportWindowFacts,
    viewport_registry::DockViewportStaleReason,
};
use open_gpui::{Bounds, Pixels, Point, WindowId, point};

impl DockViewportAdapter {
    /// Updates live window facts and host bounds in one snapshot write.
    ///
    /// Returns true when the stored snapshot changed.
    pub(crate) fn update_snapshot(
        &mut self,
        space: &DockSpaceId,
        window_facts: DockViewportWindowFacts,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(snapshot) = self.snapshot_mut(space) else {
            return false;
        };
        snapshot.update_route_facts(window_facts, host_bounds)
    }

    /// Marks a registered window's live facts stale until its next render frame publishes them.
    ///
    /// Returns true when the runtime snapshot changed.
    pub(crate) fn mark_window_snapshot_stale(&mut self, window_id: WindowId) -> bool {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return false;
        };
        let Some(snapshot) = self.snapshot_mut(&space) else {
            return false;
        };
        snapshot.mark_route_facts_stale(DockViewportStaleReason::WindowFactsChanged)
    }

    /// Marks a registered window as closing until the platform close callback unregisters it.
    ///
    /// This keeps the space/window mapping available for close attribution while removing the
    /// route authority of a viewport whose contents were already merged back during should-close.
    pub(crate) fn mark_window_close_requested(&mut self, window_id: WindowId) -> bool {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return false;
        };
        let Some(snapshot) = self.snapshot_mut(&space) else {
            return false;
        };
        snapshot.mark_route_facts_stale(DockViewportStaleReason::PlatformCloseRequested)
    }

    pub(crate) fn snapshot_facts_generation(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<u64> {
        let snapshot = self.snapshot(space)?;
        snapshot.facts_generation_if_current(window_id)
    }

    /// Converts a window-local point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, host bounds are stale, or the point is outside
    /// the host bounds.
    pub(crate) fn window_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        if !snapshot.is_route_ready() {
            return None;
        }
        let host_bounds = snapshot.host_bounds?;
        if !host_bounds.contains(&position) {
            return None;
        }

        Some(point(
            position.x - host_bounds.origin.x,
            position.y - host_bounds.origin.y,
        ))
    }

    /// Converts a screen point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, bounds snapshots are stale, or the point is
    /// outside the host bounds.
    pub(crate) fn screen_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        if !snapshot.is_route_ready() {
            return None;
        }
        let screen_bounds = snapshot.screen_bounds?;
        let window_position = point(
            position.x - screen_bounds.origin.x,
            position.y - screen_bounds.origin.y,
        );
        self.window_to_host(space, window_position)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DockViewportAdapter, DockViewportHit, DockViewportTargetContext, DockViewportWindowFacts,
        viewport_registry::{
            DockViewportLifecycleState, DockViewportRouteUnavailableReason, DockViewportStaleReason,
        },
        viewport_test_support::{bounds, handle, space},
    };
    use open_gpui::{DisplayId, WindowBounds, point, px};

    #[test]
    fn coordinate_conversion_requires_current_bounds_snapshots() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));

        assert!(
            adapter
                .screen_to_host(&main, point(px(115.0), px(225.0)))
                .is_none()
        );

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(100.0, 200.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        assert_eq!(
            adapter.window_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter.screen_to_host(&main, point(px(115.0), px(225.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert_eq!(
            adapter
                .resolve_diagnostic_viewport_target(
                    point(px(115.0), px(225.0)),
                    &DockViewportTargetContext::new()
                )
                .map(|target| target.into_hit()),
            Some(DockViewportHit::new(main.clone(), point(px(5.0), px(5.0))))
        );
        assert!(
            adapter
                .screen_to_host(&main, point(px(500.0), px(500.0)))
                .is_none()
        );
    }

    #[test]
    fn screen_conversion_uses_current_bounds_not_restore_bounds() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Maximized(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(0.0, 0.0, 1440.0, 900.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        assert_eq!(
            adapter.screen_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0))),
            "hit testing must use the live maximized screen rect, not the saved restore rect"
        );
        assert!(
            adapter
                .screen_to_host(&main, point(px(115.0), px(205.0)))
                .is_some(),
            "points are still valid when they also happen to overlap the restore rect"
        );
        assert_eq!(
            adapter
                .resolve_diagnostic_viewport_target(
                    point(px(15.0), px(25.0)),
                    &DockViewportTargetContext::new()
                )
                .map(|target| target.into_hit()),
            Some(DockViewportHit::new(main, point(px(5.0), px(5.0))))
        );
    }

    #[test]
    fn window_bounds_change_marks_snapshot_stale_until_next_live_update() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        adapter.register_viewport(main.clone(), window);
        assert!(!adapter.route_ready(&main));
        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::RegisteredNotReady)
        );

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(100.0, 200.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        assert!(adapter.route_ready(&main));
        assert_eq!(adapter.route_unavailable_reason(&main), None);
        let generation = adapter
            .snapshot_facts_generation(&main, window.window_id())
            .expect("fresh snapshot should expose its generation");
        assert_eq!(
            adapter.screen_to_host(&main, point(px(115.0), px(225.0))),
            Some(point(px(5.0), px(5.0)))
        );

        assert!(adapter.mark_window_snapshot_stale(window.window_id()));
        assert!(!adapter.route_ready(&main));
        assert_eq!(
            adapter
                .snapshot(&main)
                .expect("stale viewport should remain registered")
                .lifecycle_state(),
            DockViewportLifecycleState::Stale(DockViewportStaleReason::WindowFactsChanged)
        );
        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::WindowFactsChanged
            ))
        );
        assert_ne!(
            adapter.snapshot_facts_generation(&main, window.window_id()),
            Some(generation),
            "stale snapshots must not validate against cached route generations"
        );
        assert_eq!(
            adapter.screen_to_host(&main, point(px(115.0), px(225.0))),
            None,
            "screen-to-host conversion must wait for fresh platform facts"
        );

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(120.0, 220.0, 800.0, 600.0)),
                bounds(120.0, 220.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        assert!(adapter.route_ready(&main));
        assert_eq!(adapter.route_unavailable_reason(&main), None);
        assert_eq!(
            adapter.screen_to_host(&main, point(px(135.0), px(245.0))),
            Some(point(px(5.0), px(5.0)))
        );
    }

    #[test]
    fn snapshot_updates_report_only_real_changes() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let missing = space("missing");
        adapter.register_viewport(main.clone(), handle(1));

        let display = Some(DisplayId::new(7));
        let window_bounds = WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0));
        let screen_bounds = bounds(100.0, 200.0, 800.0, 600.0);
        let host_bounds = bounds(10.0, 20.0, 300.0, 200.0);
        assert!(!adapter.update_snapshot(
            &missing,
            DockViewportWindowFacts::new(display, window_bounds, screen_bounds),
            host_bounds
        ));

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(display, window_bounds, screen_bounds),
            host_bounds
        ));
        assert!(!adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(display, window_bounds, screen_bounds),
            host_bounds
        ));

        let next_display = Some(DisplayId::new(8));
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(next_display, window_bounds, screen_bounds),
            host_bounds
        ));

        let next_window_bounds = WindowBounds::Windowed(bounds(120.0, 220.0, 800.0, 600.0));
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(next_display, next_window_bounds, screen_bounds),
            host_bounds
        ));

        let next_screen_bounds = bounds(120.0, 220.0, 800.0, 600.0);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(next_display, next_window_bounds, next_screen_bounds),
            host_bounds
        ));

        let next_host_bounds = bounds(10.0, 20.0, 320.0, 240.0);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(next_display, next_window_bounds, next_screen_bounds),
            next_host_bounds
        ));

        let snapshot = adapter
            .snapshot(&main)
            .expect("registered viewport should retain its snapshot");
        assert_eq!(snapshot.display_id, next_display);
        assert_eq!(snapshot.window_bounds, Some(next_window_bounds));
        assert_eq!(snapshot.screen_bounds, Some(next_screen_bounds));
        assert_eq!(snapshot.host_bounds, Some(next_host_bounds));
    }
}
