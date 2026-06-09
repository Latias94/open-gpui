use crate::{DockSpaceId, DockViewportAdapter};
use open_gpui::{Bounds, DisplayId, Pixels, Point, WindowBounds, point};

impl DockViewportAdapter {
    /// Updates display id, window bounds, and host bounds in one snapshot write.
    ///
    /// Returns true when the stored snapshot changed.
    pub(crate) fn update_snapshot(
        &mut self,
        space: &DockSpaceId,
        display_id: Option<DisplayId>,
        window_bounds: WindowBounds,
        host_bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(snapshot) = self.snapshot_mut(space) else {
            return false;
        };
        let window_bounds = Some(window_bounds);
        let host_bounds = Some(host_bounds);
        if snapshot.display_id == display_id
            && snapshot.window_bounds == window_bounds
            && snapshot.host_bounds == host_bounds
        {
            return false;
        }

        snapshot.display_id = display_id;
        snapshot.window_bounds = window_bounds;
        snapshot.host_bounds = host_bounds;
        true
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
        let host_bounds = self.snapshot(space)?.host_bounds?;
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
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let window_position = point(
            position.x - window_bounds.origin.x,
            position.y - window_bounds.origin.y,
        );
        self.window_to_host(space, window_position)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DockViewportAdapter, DockViewportHit, DockViewportTargetContext,
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
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
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
                .resolve_viewport_target(
                    point(px(115.0), px(225.0)),
                    &DockViewportTargetContext::new()
                )
                .map(|target| target.into_hit()),
            Some(DockViewportHit {
                space: main.clone(),
                host_position: point(px(5.0), px(5.0)),
            })
        );
        assert!(
            adapter
                .screen_to_host(&main, point(px(500.0), px(500.0)))
                .is_none()
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
        let host_bounds = bounds(10.0, 20.0, 300.0, 200.0);
        assert!(!adapter.update_snapshot(&missing, display, window_bounds, host_bounds));

        assert!(adapter.update_snapshot(&main, display, window_bounds, host_bounds));
        assert!(!adapter.update_snapshot(&main, display, window_bounds, host_bounds));

        let next_display = Some(DisplayId::new(8));
        assert!(adapter.update_snapshot(&main, next_display, window_bounds, host_bounds));

        let next_window_bounds = WindowBounds::Windowed(bounds(120.0, 220.0, 800.0, 600.0));
        assert!(adapter.update_snapshot(&main, next_display, next_window_bounds, host_bounds));

        let next_host_bounds = bounds(10.0, 20.0, 320.0, 240.0);
        assert!(adapter.update_snapshot(&main, next_display, next_window_bounds, next_host_bounds));

        let snapshot = adapter
            .snapshot(&main)
            .expect("registered viewport should retain its snapshot");
        assert_eq!(snapshot.display_id, next_display);
        assert_eq!(snapshot.window_bounds, Some(next_window_bounds));
        assert_eq!(snapshot.host_bounds, Some(next_host_bounds));
    }
}
