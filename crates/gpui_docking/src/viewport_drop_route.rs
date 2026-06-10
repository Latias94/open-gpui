use crate::{
    DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportHit, DockViewportTargetContext,
    DockViewportTearOffRequest,
};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowBounds};

/// Runtime route for a rendered drag release before workspace mutation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRoute {
    /// The release is still in the source viewport, so the source host should commit locally.
    Local {
        /// Source viewport space that owns the local target.
        space: DockSpaceId,
        /// Local host position for the release.
        host_position: Point<Pixels>,
    },
    /// The release landed inside another registered viewport.
    KnownViewport {
        /// Destination viewport hit.
        hit: DockViewportHit,
        /// Runtime window that owns the destination host.
        window: AnyWindowHandle,
    },
    /// The release landed outside all registered viewports and may open a new platform viewport.
    TearOff(DockViewportTearOffRequest),
    /// The release landed outside all registered viewports, but policy forbids opening one.
    Rejected(DockPolicyError),
}

/// All platform and payload facts needed to route one rendered drop release.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportDropRouteRequest<'a> {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) payload: DockViewportDropPayload,
    pub(crate) release_position: Point<Pixels>,
    pub(crate) suggested_window_bounds: Option<WindowBounds>,
    pub(crate) target_context: &'a DockViewportTargetContext,
}

impl<'a> DockViewportDropRouteRequest<'a> {
    pub(crate) fn new(
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: &'a DockViewportTargetContext,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            target_context,
        }
    }
}

/// All facts needed to resolve and commit one rendered drop release.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportDropCommitRequest<'a> {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) payload: DockViewportDropPayload,
    pub(crate) release_position: Point<Pixels>,
    pub(crate) suggested_window_bounds: Option<WindowBounds>,
    pub(crate) target_context: &'a DockViewportTargetContext,
}

impl<'a> DockViewportDropCommitRequest<'a> {
    pub(crate) fn new(
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: &'a DockViewportTargetContext,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            target_context,
        }
    }

    pub(crate) fn route_request(&self) -> DockViewportDropRouteRequest<'a> {
        DockViewportDropRouteRequest::new(
            self.source_space.clone(),
            self.source_tabs,
            self.payload.clone(),
            self.release_position,
            self.suggested_window_bounds,
            self.target_context,
        )
    }
}

impl DockViewportAdapter {
    pub(crate) fn resolve_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest<'_>,
        policy: &DockPolicy,
    ) -> DockViewportDropRoute {
        if let Some(candidate) =
            self.resolve_viewport_target(request.release_position, request.target_context)
        {
            if candidate.space == request.source_space {
                return DockViewportDropRoute::Local {
                    space: candidate.space,
                    host_position: candidate.host_position,
                };
            }

            let window = candidate.window;
            return DockViewportDropRoute::KnownViewport {
                hit: candidate.into_hit(),
                window,
            };
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportDropRoute::Rejected(reason);
        }

        DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
            source_space: request.source_space.clone(),
            source_tabs: request.source_tabs,
            payload: request.payload.clone(),
            release_position: request.release_position,
            suggested_window_bounds: request.suggested_window_bounds,
        })
    }

    /// Resolves a rendered payload release into a runtime route without mutating the graph.
    ///
    /// The route contains viewport-level information only. The payload is carried only when the
    /// route becomes a tear-off request; local and known-viewport commits receive the payload from
    /// the caller when the route is committed.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_payload_drop_route_with_context(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        policy: &DockPolicy,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        let request = DockViewportDropRouteRequest::new(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            target_context,
        );
        self.resolve_payload_drop_route(&request, policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockPolicy,
        viewport_test_support::{bounds, handle, item, space},
    };
    use open_gpui::{DisplayId, WindowBounds, point, px};
    use slotmap::Key;

    #[test]
    fn drop_route_treats_source_viewport_hit_as_local() {
        let main = space("main");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), window);
        adapter.update_snapshot(
            &main,
            Some(DisplayId::new(7)),
            WindowBounds::Windowed(bounds(100.0, 200.0, 800.0, 600.0)),
            bounds(10.0, 20.0, 300.0, 200.0),
        );

        assert_eq!(
            adapter.resolve_payload_drop_route_with_context(
                main.clone(),
                DockNodeId::null(),
                DockViewportDropPayload::Item(item("a")),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
                &DockViewportTargetContext::new(),
            ),
            DockViewportDropRoute::Local {
                space: main,
                host_position: point(px(5.0), px(5.0)),
            }
        );
    }

    #[test]
    fn drop_route_uses_viewport_arbitration_for_known_viewport() {
        let source = space("source");
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

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            &DockViewportTargetContext::new().with_active_window(zeta_window),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                hit: DockViewportHit {
                    space: zeta,
                    host_position: point(px(20.0), px(40.0)),
                },
                window: zeta_window,
            }
        );
    }

    #[test]
    fn drop_route_outside_all_viewports_uses_tear_off_policy() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let adapter = DockViewportAdapter::new();

        assert_eq!(
            adapter.resolve_payload_drop_route_with_context(
                source.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item.clone()),
                release_position,
                Some(suggested_window_bounds),
                &DockPolicy::default(),
                &DockViewportTargetContext::new(),
            ),
            DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled)
        );

        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);
        assert_eq!(
            adapter.resolve_payload_drop_route_with_context(
                source.clone(),
                source_tabs,
                DockViewportDropPayload::Item(item.clone()),
                release_position,
                Some(suggested_window_bounds),
                &policy,
                &DockViewportTargetContext::new(),
            ),
            DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
                source_space: source,
                source_tabs,
                payload: DockViewportDropPayload::Item(item),
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }
}
