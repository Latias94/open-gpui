use crate::{DockSpaceId, DockViewportAdapter};
use open_gpui::{Bounds, DisplayId, Pixels, Point, WindowBounds, point};

impl DockViewportAdapter {
    /// Updates the display id snapshot for a logical dock space.
    ///
    /// Returns true when the stored snapshot changed.
    pub fn set_display_id(&mut self, space: &DockSpaceId, display_id: Option<DisplayId>) -> bool {
        let Some(snapshot) = self.snapshot_mut(space) else {
            return false;
        };
        if snapshot.display_id == display_id {
            return false;
        }

        snapshot.display_id = display_id;
        true
    }

    /// Updates the platform window bounds snapshot for a logical dock space.
    ///
    /// Returns true when the stored snapshot changed.
    pub fn set_window_bounds(&mut self, space: &DockSpaceId, bounds: WindowBounds) -> bool {
        let Some(snapshot) = self.snapshot_mut(space) else {
            return false;
        };
        let bounds = Some(bounds);
        if snapshot.window_bounds == bounds {
            return false;
        }

        snapshot.window_bounds = bounds;
        true
    }

    /// Updates the dock host bounds snapshot for a logical dock space.
    ///
    /// Returns true when the stored snapshot changed.
    pub fn set_host_bounds(&mut self, space: &DockSpaceId, bounds: Bounds<Pixels>) -> bool {
        let Some(snapshot) = self.snapshot_mut(space) else {
            return false;
        };
        let bounds = Some(bounds);
        if snapshot.host_bounds == bounds {
            return false;
        }

        snapshot.host_bounds = bounds;
        true
    }

    /// Updates display id, window bounds, and host bounds in one snapshot write.
    ///
    /// Returns true when the stored snapshot changed.
    pub fn update_snapshot(
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
    pub fn window_to_host(
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
    pub fn screen_to_host(
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

    /// Converts a host-local point into screen coordinates.
    pub fn host_to_screen(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        let window_bounds = snapshot.window_bounds?.get_bounds();
        let host_bounds = snapshot.host_bounds?;
        Some(point(
            window_bounds.origin.x + host_bounds.origin.x + position.x,
            window_bounds.origin.y + host_bounds.origin.y + position.y,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockHost, DockViewportAdapter, DockViewportHit};
    use open_gpui::{
        AnyWindowHandle, Bounds, DisplayId, Pixels, WindowBounds, WindowHandle, WindowId, point,
        px, size,
    };

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn handle(id: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(id)).into()
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

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
            adapter.host_to_screen(&main, point(px(5.0), px(5.0))),
            Some(point(px(115.0), px(225.0)))
        );
        assert_eq!(
            adapter.hit_test_screen(point(px(115.0), px(225.0))),
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
        assert!(!adapter.set_display_id(&missing, display));
        assert!(!adapter.set_window_bounds(&missing, window_bounds));
        assert!(!adapter.set_host_bounds(&missing, host_bounds));
        assert!(!adapter.update_snapshot(&missing, display, window_bounds, host_bounds));

        assert!(adapter.set_display_id(&main, display));
        assert!(!adapter.set_display_id(&main, display));
        assert!(adapter.set_window_bounds(&main, window_bounds));
        assert!(!adapter.set_window_bounds(&main, window_bounds));
        assert!(adapter.set_host_bounds(&main, host_bounds));
        assert!(!adapter.set_host_bounds(&main, host_bounds));
        assert!(!adapter.update_snapshot(&main, display, window_bounds, host_bounds));

        let next_host_bounds = bounds(10.0, 20.0, 320.0, 240.0);
        assert!(adapter.update_snapshot(&main, display, window_bounds, next_host_bounds));

        let snapshot = adapter
            .snapshot(&main)
            .expect("registered viewport should retain its snapshot");
        assert_eq!(snapshot.display_id, display);
        assert_eq!(snapshot.window_bounds, Some(window_bounds));
        assert_eq!(snapshot.host_bounds, Some(next_host_bounds));
    }
}
