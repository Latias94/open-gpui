use crate::{DockSpaceId, DockViewportAdapter};
use open_gpui::{AnyWindowHandle, App, Pixels, Point, Window, WindowId};

/// Result of resolving a screen point into a registered dock viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHit {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

/// A viewport hit with the runtime window that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHitCandidate {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// GPUI window currently rendering the logical dock space.
    pub window: AnyWindowHandle,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

impl DockViewportHitCandidate {
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit {
            space: self.space,
            host_position: self.host_position,
        }
    }
}

/// Platform facts used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockViewportTargetContext {
    /// Window currently owning the pointer, when known.
    pub hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    pub active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    pub window_stack: Vec<WindowId>,
}

impl DockViewportTargetContext {
    /// Creates an empty target context that falls back to deterministic adapter ordering.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a target context from GPUI application-level platform signals.
    pub fn from_app(cx: &App) -> Self {
        Self {
            hovered_window: None,
            active_window: cx.active_window().map(|window| window.window_id()),
            window_stack: cx
                .window_stack()
                .unwrap_or_default()
                .into_iter()
                .map(|window| window.window_id())
                .collect(),
        }
    }

    /// Builds a target context from GPUI application signals and treats this window as hovered.
    ///
    /// This is intended for pointer-event paths that already know the event window. GPUI app-level
    /// signals provide active-window and stack ordering; the current event window supplies the
    /// more specific hovered-window tie breaker.
    pub fn from_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_hovered_window(window.window_handle())
    }

    /// Adds the hovered window signal.
    pub fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.hovered_window = Some(window.into().window_id());
        self
    }

    /// Adds the active window signal.
    pub fn with_active_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.active_window = Some(window.into().window_id());
        self
    }

    /// Adds the front-to-back window stack signal.
    pub fn with_window_stack(
        mut self,
        windows: impl IntoIterator<Item = impl Into<AnyWindowHandle>>,
    ) -> Self {
        self.window_stack = windows
            .into_iter()
            .map(|window| window.into().window_id())
            .collect();
        self
    }
}

pub(crate) fn resolve_viewport_target(
    hits: Vec<DockViewportHitCandidate>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportHitCandidate> {
    hits.into_iter()
        .enumerate()
        .min_by_key(|(index, hit)| {
            let window_id = hit.window.window_id();
            (
                context
                    .hovered_window
                    .map(|hovered| usize::from(hovered != window_id))
                    .unwrap_or(1),
                context
                    .active_window
                    .map(|active| usize::from(active != window_id))
                    .unwrap_or(1),
                context
                    .window_stack
                    .iter()
                    .position(|stacked| *stacked == window_id)
                    .unwrap_or(usize::MAX),
                *index,
            )
        })
        .map(|(_, hit)| hit)
}

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
        resolve_viewport_target(hits, context)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockHost, DockItemId, DockNodeId, DockPolicy, DockPolicyError, DockViewportAdapter,
        DockViewportTearOffOutcome, DockViewportTearOffRequest,
    };
    use open_gpui::{
        AnyWindowHandle, Bounds, DisplayId, Pixels, WindowBounds, WindowHandle, WindowId, point,
        px, size,
    };
    use slotmap::Key;

    fn space(id: &str) -> DockSpaceId {
        DockSpaceId::from(id)
    }

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn handle(id: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(id)).into()
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn overlapping_viewport_hits_prefer_hovered_active_then_window_stack() {
        let mut adapter = DockViewportAdapter::new();
        let alpha = space("alpha");
        let zeta = space("zeta");
        let first = handle(1);
        let second = handle(2);
        adapter.register_viewport(zeta.clone(), second);
        adapter.register_viewport(alpha.clone(), first);
        for space in [&alpha, &zeta] {
            adapter.update_snapshot(
                space,
                None,
                WindowBounds::Windowed(bounds(100.0, 100.0, 300.0, 200.0)),
                bounds(0.0, 0.0, 300.0, 200.0),
            );
        }
        let position = point(px(125.0), px(150.0));

        assert_eq!(
            adapter.hit_test_screen(position).map(|hit| hit.space),
            Some(alpha.clone()),
            "default fallback should remain deterministic by registered space order"
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new().with_active_window(second),
                )
                .map(|hit| hit.space),
            Some(zeta.clone())
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new().with_window_stack([second, first]),
                )
                .map(|hit| hit.space),
            Some(zeta.clone())
        );
        assert_eq!(
            adapter
                .hit_test_screen_with_context(
                    position,
                    &DockViewportTargetContext::new()
                        .with_hovered_window(first)
                        .with_active_window(second)
                        .with_window_stack([second, first]),
                )
                .map(|hit| hit.space),
            Some(alpha)
        );
    }

    #[test]
    fn tear_off_release_inside_known_viewport_returns_hit() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::KnownViewport(DockViewportHit {
                space: main,
                host_position: point(px(5.0), px(5.0)),
            })
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_respects_platform_policy() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");

        assert_eq!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(900.0), px(900.0)),
                None,
                &DockPolicy::default(),
            ),
            DockViewportTearOffOutcome::Rejected(DockPolicyError::PlatformViewportsDisabled)
        );
    }

    #[test]
    fn tear_off_release_outside_viewports_emits_request_when_enabled() {
        let adapter = DockViewportAdapter::new();
        let main = space("main");
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert_eq!(
            adapter.resolve_tear_off_request(
                main.clone(),
                DockNodeId::null(),
                item.clone(),
                release_position,
                Some(suggested_window_bounds),
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(DockViewportTearOffRequest {
                source_space: main,
                source_tabs: DockNodeId::null(),
                item,
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }

    #[test]
    fn stale_viewport_bounds_do_not_block_tear_off_request() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        adapter.register_viewport(main.clone(), handle(1));
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        assert!(matches!(
            adapter.resolve_tear_off_request(
                main,
                DockNodeId::null(),
                item("a"),
                point(px(115.0), px(225.0)),
                None,
                &policy,
            ),
            DockViewportTearOffOutcome::Requested(_)
        ));
    }
}
