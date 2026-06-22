use crate::{
    DockSpaceId, DockViewportAdapter, DockViewportWindowFacts,
    viewport_registry::{DockViewportInputMask, DockViewportStaleReason},
};
use open_gpui::{AnyWindowHandle, AppContext, Bounds, Pixels, Point, WindowId, point};

impl DockViewportAdapter {
    /// Collects all registered platform viewport windows containing a screen point.
    ///
    /// Input-eligible hits may authorize a host target. Stale/not-ready hits are retained as
    /// blockers so fallback authority cannot pass through an opaque viewport window. Native
    /// no-input and minimized windows are skipped like ImGui's viewport fallback.
    pub(crate) fn global_screen_viewport_window_hits(
        &self,
        position: Point<Pixels>,
    ) -> Vec<crate::DockViewportWindowHit> {
        self.spaces()
            .into_iter()
            .filter_map(|space| {
                let snapshot = self.snapshot(&space)?;
                let screen_bounds = snapshot.global_screen_bounds()?;
                if !screen_bounds.contains(&position) {
                    return None;
                }
                let window = snapshot.window;
                if !snapshot.input_mask.participates_in_hover_hit_testing() {
                    return None;
                }
                if let Some(reason) = snapshot.route_unavailable_reason() {
                    return Some(crate::DockViewportWindowHit::blocking(
                        space, window, reason,
                    ));
                }
                let facts_generation = snapshot.facts_generation();
                let window_position = point(
                    position.x - screen_bounds.origin.x,
                    position.y - screen_bounds.origin.y,
                );
                let host_position = self.window_to_host(&space, window_position);
                Some(crate::DockViewportWindowHit::with_facts_generation(
                    space,
                    window,
                    host_position,
                    facts_generation,
                ))
            })
            .collect()
    }

    /// Collects all registered dock-host hits for a screen point.
    #[cfg(test)]
    pub(crate) fn global_screen_viewport_hits(
        &self,
        position: Point<Pixels>,
    ) -> Vec<crate::DockViewportTargetHit> {
        self.global_screen_viewport_window_hits(position)
            .into_iter()
            .filter_map(crate::DockViewportWindowHit::into_target_hit)
            .collect()
    }
}

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

    /// Refreshes live input-mask facts while avoiding a window that is already in the
    /// current render/update callback.
    pub(crate) fn refresh_registered_window_facts_except_window<C: AppContext>(
        &mut self,
        cx: &mut C,
        skip_window_id: Option<WindowId>,
    ) -> Vec<AnyWindowHandle> {
        let viewports = self
            .registry
            .snapshots()
            .map(|(space, snapshot)| (space.clone(), snapshot.window))
            .collect::<Vec<_>>();
        let mut changed_windows = Vec::new();
        for (space, window) in viewports {
            if Some(window.window_id()) == skip_window_id {
                continue;
            }
            let Ok(input_mask) = window.update(cx, |_, window, _| {
                if window.is_minimized() {
                    DockViewportInputMask::Minimized
                } else if !window.accepts_pointer_input() {
                    DockViewportInputMask::NoInputPassThrough
                } else {
                    DockViewportInputMask::ReceivesInput
                }
            }) else {
                if self.mark_window_snapshot_stale(window.window_id()) {
                    changed_windows.push(window);
                }
                continue;
            };
            let Some(snapshot) = self.snapshot_mut(&space) else {
                continue;
            };
            if snapshot.refresh_input_mask(input_mask) {
                changed_windows.push(window);
            }
        }
        changed_windows
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

    /// Imports a platform-driven bounds change without assuming host layout changed.
    ///
    /// Pure window moves keep the rendered host-local facts current. Resizes still demote the
    /// snapshot until a host render republishes layout facts for the new content size.
    pub(crate) fn apply_platform_window_facts(
        &mut self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
    ) -> bool {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return false;
        };
        let Some(snapshot) = self.snapshot_mut(&space) else {
            return false;
        };
        snapshot.apply_platform_window_facts(window_facts)
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

    /// Cancels a previously accepted platform close request without restoring stale route facts.
    ///
    /// The next rendered host scene must publish fresh route facts before the window can route
    /// drops again. This mirrors ImGui's per-frame request flags without guessing that a render
    /// implies the platform close was aborted.
    pub(crate) fn cancel_window_close_requested(&mut self, window_id: WindowId) -> bool {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return false;
        };
        let Some(snapshot) = self.snapshot_mut(&space) else {
            return false;
        };
        snapshot.cancel_platform_close_request()
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

    /// Converts a global screen point into host-local coordinates.
    ///
    /// Returns `None` when the viewport is unknown, bounds snapshots are stale, the backend did
    /// not publish global window bounds, or the point is outside the host bounds.
    pub(crate) fn global_screen_to_host(
        &self,
        space: &DockSpaceId,
        position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        let snapshot = self.snapshot(space)?;
        if !snapshot.is_route_ready() {
            return None;
        }
        let screen_bounds = snapshot.global_screen_bounds()?;
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
        host_test_support::test_view,
        viewport_registry::{
            DockViewportLifecycleState, DockViewportRouteUnavailableReason, DockViewportStaleReason,
        },
        viewport_target_resolver::choose_diagnostic_viewport_target,
        viewport_test_support::{bounds, handle, register_viewport, space},
    };
    use open_gpui::{
        AnyWindowHandle, DisplayId, TestAppContext, WindowBounds, WindowOptions, point, px,
    };

    #[test]
    fn coordinate_conversion_requires_current_bounds_snapshots() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        register_viewport(&mut adapter, main.clone(), handle(1));

        assert!(
            adapter
                .global_screen_to_host(&main, point(px(115.0), px(225.0)))
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
            adapter.global_screen_to_host(&main, point(px(115.0), px(225.0))),
            Some(point(px(5.0), px(5.0)))
        );
        let hits = adapter.global_screen_viewport_hits(point(px(115.0), px(225.0)));
        assert_eq!(
            choose_diagnostic_viewport_target(hits, &DockViewportTargetContext::new())
                .map(|target| target.into_hit()),
            Some(DockViewportHit::new(main.clone(), point(px(5.0), px(5.0))))
        );
        assert!(
            adapter
                .global_screen_to_host(&main, point(px(500.0), px(500.0)))
                .is_none()
        );
    }

    #[test]
    fn screen_conversion_uses_current_bounds_not_restore_bounds() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        register_viewport(&mut adapter, main.clone(), handle(1));

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
            adapter.global_screen_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0))),
            "hit testing must use the live maximized screen rect, not the saved restore rect"
        );
        assert!(
            adapter
                .global_screen_to_host(&main, point(px(115.0), px(205.0)))
                .is_some(),
            "points are still valid when they also happen to overlap the restore rect"
        );
        let hits = adapter.global_screen_viewport_hits(point(px(15.0), px(25.0)));
        assert_eq!(
            choose_diagnostic_viewport_target(hits, &DockViewportTargetContext::new())
                .map(|target| target.into_hit()),
            Some(DockViewportHit::new(main, point(px(5.0), px(5.0))))
        );
    }

    #[open_gpui::test]
    fn refresh_registered_window_facts_marks_unreachable_window_stale(cx: &mut TestAppContext) {
        let root = test_view(cx, "A");
        let window_bounds = WindowBounds::Windowed(bounds(100.0, 200.0, 320.0, 240.0));
        let window: AnyWindowHandle = cx
            .update(|app| {
                app.open_window(
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        focus: false,
                        ..Default::default()
                    },
                    |_, _| root.clone(),
                )
            })
            .expect("test window should open")
            .into();
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        register_viewport(&mut adapter, main.clone(), window);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                window_bounds,
                window_bounds.get_bounds(),
            ),
            bounds(0.0, 0.0, 320.0, 240.0),
        ));
        assert_eq!(adapter.route_unavailable_reason(&main), None);

        window
            .update(cx, |_, window, _| window.remove_window())
            .expect("test window should still be live");
        cx.run_until_parked();

        let changed_windows = adapter.refresh_registered_window_facts_except_window(cx, None);
        assert_eq!(
            changed_windows
                .into_iter()
                .map(|window| window.window_id())
                .collect::<Vec<_>>(),
            vec![window.window_id()],
        );
        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::WindowFactsChanged
            ))
        );
        let hits = adapter.global_screen_viewport_window_hits(point(px(110.0), px(210.0)));
        assert_eq!(hits.len(), 1);
        assert!(
            hits[0].blocks_host_target(),
            "stale live-window facts must block fallback instead of authorizing a stale host target"
        );
    }

    #[test]
    fn platform_move_updates_global_origin_without_staling_host_facts() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(100.0, 200.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        let generation = adapter
            .snapshot_facts_generation(&main, window.window_id())
            .expect("snapshot should be route-ready before move");

        assert!(adapter.apply_platform_window_facts(
            window.window_id(),
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(200.0, 300.0, 800.0, 600.0)),
                bounds(200.0, 300.0, 800.0, 600.0),
            ),
        ));

        assert_eq!(adapter.route_unavailable_reason(&main), None);
        assert_eq!(
            adapter.snapshot_facts_generation(&main, window.window_id()),
            Some(generation),
            "pure platform move preserves host-scene facts generation"
        );
        assert_eq!(
            adapter.global_screen_to_host(&main, point(px(215.0), px(325.0))),
            Some(point(px(5.0), px(5.0)))
        );
        assert!(
            adapter
                .global_screen_to_host(&main, point(px(115.0), px(225.0)))
                .is_none(),
            "old global origin must no longer authorize hit testing"
        );
    }

    #[test]
    fn platform_resize_stales_route_until_host_scene_republishes() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(100.0, 200.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));

        assert!(adapter.apply_platform_window_facts(
            window.window_id(),
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 900.0, 650.0)),
                bounds(100.0, 200.0, 900.0, 650.0),
            ),
        ));

        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::WindowFactsChanged
            ))
        );
        assert_eq!(
            adapter.snapshot_facts_generation(&main, window.window_id()),
            None,
            "resized viewport must wait for a fresh host-scene generation"
        );
        assert!(
            adapter
                .global_screen_to_host(&main, point(px(115.0), px(225.0)))
                .is_none()
        );
    }

    #[test]
    fn platform_resize_before_first_host_scene_records_pending_resize_request() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);

        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::RegisteredNotReady)
        );
        assert!(adapter.apply_platform_window_facts(
            window.window_id(),
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 900.0, 650.0)),
                bounds(100.0, 200.0, 900.0, 650.0),
            ),
        ));

        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::WindowFactsChanged
            ))
        );
        assert!(
            adapter.platform_requests_for_space(&main).resize_requested,
            "backend resize before first host scene should still suppress one reverse resize"
        );

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 900.0, 650.0)),
                bounds(100.0, 200.0, 900.0, 650.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        assert_eq!(adapter.route_unavailable_reason(&main), None);
        assert!(
            !adapter.platform_requests_for_space(&main).resize_requested,
            "fresh host scene consumes the backend resize request"
        );
    }

    #[test]
    fn window_bounds_change_marks_snapshot_stale_until_next_live_update() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);
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
            adapter.global_screen_to_host(&main, point(px(115.0), px(225.0))),
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
            adapter.global_screen_to_host(&main, point(px(115.0), px(225.0))),
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
            adapter.global_screen_to_host(&main, point(px(135.0), px(245.0))),
            Some(point(px(5.0), px(5.0)))
        );
    }

    #[test]
    fn cancel_window_close_request_requires_next_live_update_before_routing() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        let window_facts = DockViewportWindowFacts::new(
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(100.0, 200.0, 800.0, 600.0),
        );
        let host_bounds = bounds(10.0, 20.0, 300.0, 200.0);
        register_viewport(&mut adapter, main.clone(), window);
        assert!(adapter.update_snapshot(&main, window_facts, host_bounds));
        assert!(adapter.route_ready(&main));

        assert!(adapter.mark_window_close_requested(window.window_id()));
        assert!(!adapter.route_ready(&main));
        assert!(adapter.cancel_window_close_requested(window.window_id()));
        assert!(!adapter.route_ready(&main));
        assert_eq!(
            adapter.route_unavailable_reason(&main),
            Some(DockViewportRouteUnavailableReason::Stale(
                DockViewportStaleReason::WindowFactsChanged
            ))
        );

        assert!(adapter.update_snapshot(&main, window_facts, host_bounds));
        assert!(adapter.route_ready(&main));
        assert!(!adapter.cancel_window_close_requested(window.window_id()));
    }

    #[test]
    fn input_mask_refresh_to_no_input_preserves_route_facts_generation() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);
        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::new(
                Some(DisplayId::new(7)),
                WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
                bounds(100.0, 200.0, 800.0, 600.0),
            ),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        let generation = adapter
            .snapshot_facts_generation(&main, window.window_id())
            .expect("fresh route facts should expose generation");

        assert!(
            adapter.apply_platform_window_facts(
                window.window_id(),
                DockViewportWindowFacts::new(
                    Some(DisplayId::new(7)),
                    WindowBounds::Windowed(bounds(120.0, 220.0, 800.0, 600.0)),
                    bounds(120.0, 220.0, 800.0, 600.0),
                )
                .with_input_mask(
                    crate::viewport_registry::DockViewportInputMask::NoInputPassThrough
                ),
            )
        );

        assert!(adapter.route_ready(&main));
        assert_eq!(adapter.route_unavailable_reason(&main), None);
        assert_eq!(
            adapter.snapshot_facts_generation(&main, window.window_id()),
            Some(generation),
            "input-mask-only changes do not advance route facts generation"
        );
        assert_eq!(
            adapter.global_screen_to_host(&main, point(px(135.0), px(245.0))),
            Some(point(px(5.0), px(5.0))),
            "coordinate conversion can still use current route facts"
        );
        assert!(
            adapter
                .global_screen_viewport_window_hits(point(px(135.0), px(245.0)))
                .is_empty(),
            "hover hit testing skips native no-input viewports"
        );
    }

    #[test]
    fn snapshot_updates_report_only_real_changes() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let missing = space("missing");
        register_viewport(&mut adapter, main.clone(), handle(1));

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
        assert_eq!(snapshot.global_screen_bounds(), Some(next_screen_bounds));
        assert_eq!(snapshot.host_bounds, Some(next_host_bounds));
    }

    #[test]
    fn screen_conversion_rejects_window_local_bounds() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        register_viewport(&mut adapter, main.clone(), handle(1));

        assert!(adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(
                bounds(100.0, 200.0, 800.0, 600.0,)
            )),
            bounds(10.0, 20.0, 300.0, 200.0),
        ));
        assert!(adapter.route_ready(&main));
        assert_eq!(
            adapter.window_to_host(&main, point(px(15.0), px(25.0))),
            Some(point(px(5.0), px(5.0))),
            "receiver-local routing can still use host-local geometry"
        );
        assert_eq!(
            adapter.global_screen_to_host(&main, point(px(115.0), px(225.0))),
            None,
            "window-local platform bounds must not authorize global screen conversion"
        );
        assert_eq!(
            choose_diagnostic_viewport_target(
                adapter.global_screen_viewport_hits(point(px(115.0), px(225.0))),
                &DockViewportTargetContext::new(),
            ),
            None,
            "global viewport hit testing must not treat window-local bounds as screen bounds"
        );
    }
}
