#[cfg(test)]
use crate::DockViewportTargetContext;
use crate::{
    DockNodeId, DockPolicy, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportPlatformSignals, DockViewportTargetHit,
    DockViewportTearOffRequest,
};
use open_gpui::{Pixels, Point, WindowBounds};

/// Runtime route for a rendered drag release before workspace mutation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRoute {
    /// The release is still in the source viewport, so the source host should commit locally.
    Local {
        /// Local host position for the release.
        host_position: Point<Pixels>,
    },
    /// The release landed inside another registered viewport.
    KnownViewport {
        /// Destination viewport hit and its owning runtime window.
        target: DockViewportTargetHit,
    },
    /// The release landed outside all registered viewports and may open a new platform viewport.
    TearOff(DockViewportTearOffRequest),
    /// The release landed outside all registered viewports, but policy forbids opening one.
    Rejected(DockPolicyError),
}

/// All platform and payload facts needed to route one rendered drop release.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportDropRouteRequest {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) payload: DockViewportDropPayload,
    pub(crate) release_position: Point<Pixels>,
    pub(crate) suggested_window_bounds: Option<WindowBounds>,
    pub(crate) platform_signals: DockViewportPlatformSignals,
}

/// Commit-time facts for a resolved viewport drop route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRouteCommit {
    /// Commit a route back into the source viewport host.
    Local {
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        host_position: Point<Pixels>,
    },
    /// Commit a route into another registered viewport.
    KnownViewport {
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        target: DockViewportTargetHit,
    },
    /// Open and commit into a new platform viewport.
    TearOff(DockViewportTearOffRequest),
    /// Reject the commit for the same policy reason as routing.
    Rejected(DockPolicyError),
}

impl DockViewportDropRouteCommit {
    pub(crate) fn from_route_request(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
    ) -> Self {
        match route {
            DockViewportDropRoute::Local { host_position } => Self::Local {
                source_space: request.source_space.clone(),
                source_tabs: request.source_tabs,
                payload: request.payload.clone(),
                host_position,
            },
            DockViewportDropRoute::KnownViewport { target } => Self::KnownViewport {
                source_space: request.source_space.clone(),
                source_tabs: request.source_tabs,
                payload: request.payload.clone(),
                target,
            },
            DockViewportDropRoute::TearOff(_) => {
                Self::TearOff(tear_off_request_from_route_request(request))
            }
            DockViewportDropRoute::Rejected(error) => Self::Rejected(error),
        }
    }
}

impl DockViewportDropRouteRequest {
    pub(crate) fn from_platform_signals(
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: DockViewportTargetContext,
    ) -> Self {
        Self::from_platform_signals(
            source_space,
            source_tabs,
            payload,
            release_position,
            suggested_window_bounds,
            DockViewportPlatformSignals::from_target_context(target_context),
        )
    }
}

impl DockViewportAdapter {
    pub(crate) fn resolve_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
    ) -> DockViewportDropRoute {
        let target_context = request.platform_signals.target_context();
        if let Some(candidate) =
            self.resolve_viewport_target(request.release_position, &target_context)
        {
            if candidate.space == request.source_space {
                return DockViewportDropRoute::Local {
                    host_position: candidate.host_position,
                };
            }

            return DockViewportDropRoute::KnownViewport { target: candidate };
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportDropRoute::Rejected(reason);
        }

        DockViewportDropRoute::TearOff(tear_off_request_from_route_request(request))
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
        target_context: DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        let request = DockViewportDropRouteRequest::from_target_context(
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

fn tear_off_request_from_route_request(
    request: &DockViewportDropRouteRequest,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest {
        source_space: request.source_space.clone(),
        source_tabs: request.source_tabs,
        payload: request.payload.clone(),
        release_position: request.release_position,
        suggested_window_bounds: request.suggested_window_bounds,
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
                DockViewportTargetContext::new(),
            ),
            DockViewportDropRoute::Local {
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
            DockViewportTargetContext::new().with_active_window(zeta_window),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit {
                    space: zeta,
                    window: zeta_window,
                    host_position: point(px(20.0), px(40.0)),
                },
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
                DockViewportTargetContext::new(),
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
                DockViewportTargetContext::new(),
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

    #[test]
    fn drop_route_commit_derives_tear_off_request_from_route_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            release_position,
            Some(suggested_window_bounds),
            DockViewportTargetContext::new(),
        );
        let mismatched_route = DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
            source_space: space("other"),
            source_tabs,
            payload: DockViewportDropPayload::Tabs,
            release_position: point(px(1.0), px(2.0)),
            suggested_window_bounds: None,
        });

        assert_eq!(
            DockViewportDropRouteCommit::from_route_request(&request, mismatched_route),
            DockViewportDropRouteCommit::TearOff(DockViewportTearOffRequest {
                source_space: source,
                source_tabs,
                payload: DockViewportDropPayload::Item(item),
                release_position,
                suggested_window_bounds: Some(suggested_window_bounds),
            })
        );
    }
}
