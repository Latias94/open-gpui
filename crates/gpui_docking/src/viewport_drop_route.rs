use crate::DockViewportTargetContext;
#[cfg(test)]
use crate::{DockNodeId, DockSpaceId, DockViewportDropPayload, DockViewportPlatformSignals};
use crate::{DockPolicy, DockViewportAdapter};
#[cfg(test)]
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowBounds};

mod event_receiver;
mod model;
mod planner;
mod request;

pub(crate) use model::{
    DockViewportDropRoute, DockViewportDropRouteResolution, DockViewportDropRouteUnavailableReason,
};
pub(crate) use request::{
    DockViewportDropReleasePoint, DockViewportDropRouteRequest, DockViewportPointerCoordinateSpace,
};

impl DockViewportAdapter {
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
    ) -> DockViewportDropRoute {
        self.resolve_payload_drop_route_resolution(request, policy)
            .into_route()
    }

    pub(crate) fn resolve_payload_drop_route_resolution(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
    ) -> DockViewportDropRouteResolution {
        let target_context = self.normalize_target_context(request.target_context());
        self.resolve_payload_drop_route_resolution_with_target_context(
            request,
            policy,
            target_context,
        )
    }

    fn resolve_payload_drop_route_resolution_with_target_context(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
        target_context: DockViewportTargetContext,
    ) -> DockViewportDropRouteResolution {
        let target_context = self.normalize_target_context(target_context);
        self.resolve_payload_drop_route_plan(request, &target_context)
            .into_resolution(policy, request.supports_platform_viewport_windows())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockPolicy, DockPolicyError, DockViewportRouteSelectionSource, DockViewportTargetHit,
        DockViewportWindowFacts,
        interaction::DockPayloadDropReleaseOrigin,
        viewport_drop_scene::DockViewportHostSceneFrame,
        viewport_registry::{DockViewportInputMask, DockViewportWindowBoundsFrame},
        viewport_test_support::{bounds, handle, item, register_viewport, space},
    };
    use open_gpui::{DisplayId, WindowBounds, point, px};
    use slotmap::Key;

    fn signals_with_receiver(
        target_context: DockViewportTargetContext,
        receiver: AnyWindowHandle,
    ) -> DockViewportPlatformSignals {
        DockViewportPlatformSignals::from_target_context(target_context)
            .with_event_receiver_window(receiver)
    }

    fn scene_proof(
        space: &DockSpaceId,
        window: AnyWindowHandle,
        generation: u64,
    ) -> DockViewportHostSceneFrame {
        DockViewportHostSceneFrame::new_for_test(space.clone(), window.window_id(), generation)
    }

    #[test]
    fn hovered_host_global_drop_requires_explicit_route_selection() {
        let main = space("main");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, main.clone(), window);
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_display_id(Some(DisplayId::new(7))),
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
            DockViewportDropRoute::Unavailable,
            "a lone geometry hit is diagnostic-only without backend hovered-window or stack route selection"
        );
        assert_eq!(
            adapter.resolve_payload_drop_route_with_context(
                main.clone(),
                DockNodeId::null(),
                DockViewportDropPayload::Item(item("a")),
                point(px(115.0), px(225.0)),
                None,
                &DockPolicy::default(),
                DockViewportTargetContext::new().with_trusted_hovered_window(window),
            ),
            DockViewportDropRoute::Local {
                host_position: point(px(5.0), px(5.0)),
                window_id: window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            }
        );
    }

    #[test]
    fn global_drop_inside_viewport_window_but_outside_host_is_unavailable() {
        let main = space("main");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, main.clone(), window);
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(40.0, 40.0, 100.0, 100.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            main,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(110.0), px(110.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new(),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "a release inside an existing viewport window but outside its dock host must not fall through to tear-off"
        );
    }

    #[test]
    fn window_stack_front_viewport_outside_host_blocks_underlay_host_hit() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);
        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(40.0, 40.0, 100.0, 100.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(120.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new().with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "platform window route selection must stop at the front viewport window instead of tunneling to an underlay host hit"
        );
    }

    #[test]
    fn window_stack_front_stale_viewport_blocks_underlay_host_hit() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);
        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        assert!(adapter.mark_window_snapshot_stale(top_window.window_id()));

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(120.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new().with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "stale front viewport windows remain opaque blockers until a fresh host frame republishes route facts"
        );
    }

    #[test]
    fn source_only_global_drop_rejects_geometry_only_source_fallback() {
        let main = space("main");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, main.clone(), window);
        adapter.update_snapshot(
            &main,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            main,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(115.0), px(225.0)),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "source-only release should not infer a route selection source from a lone geometry hit"
        );
    }

    #[test]
    fn drop_route_selects_window_stack_fallback_when_hovered_backend_is_unavailable() {
        let source = space("source");
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, alpha.clone(), alpha_window);
        register_viewport(&mut adapter, zeta.clone(), zeta_window);

        for space in [&alpha, &zeta] {
            adapter.update_snapshot(
                space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
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
            DockViewportTargetContext::new().with_window_stack([zeta_window, alpha_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    zeta.clone(),
                    zeta_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "front-to-back window stack fallback selects a route when the hovered-window backend is unavailable"
        );
    }

    #[test]
    fn hovered_host_global_drop_keeps_event_receiver_diagnostic_under_window_stack_fallback() {
        let source = space("source");
        let target = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            signals_with_receiver(DockViewportTargetContext::new(), target_window),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "event receiver remains diagnostic-only; a lone geometry hit cannot select a viewport route"
        );
    }

    #[test]
    fn source_only_global_drop_rejects_window_stack_fallback_for_cross_viewport_route() {
        let source = space("source");
        let target_space = space("target");
        let receiver_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            signals_with_receiver(DockViewportTargetContext::new(), receiver_window),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "source-only global releases require current route facts; backend fallback must not grant cross-viewport delivery"
        );
    }

    #[test]
    fn source_only_global_drop_rejects_window_stack_source_for_cross_viewport_route() {
        let source = space("source");
        let target_space = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target_space.clone(), target_window);

        for space in [&source, &target_space] {
            adapter.update_snapshot(
                space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
                bounds(0.0, 0.0, 320.0, 240.0),
            );
        }

        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "source-only releases must not use window-stack fallback as release authority; window stack fallback is preview route selection only"
        );
    }

    #[test]
    fn source_only_global_drop_accepts_trusted_hovered_cross_viewport_route() {
        let source = space("source");
        let target_space = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            ),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    point(px(20.0), px(30.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "source-only release should still accept current trusted hovered-window route facts"
        );
    }

    #[test]
    fn trusted_hovered_none_rejects_geometry_hit() {
        let source = space("source");
        let target_space = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
        );

        let resolution = adapter.resolve_payload_drop_route_resolution(&request, &policy);

        assert_eq!(
            resolution.route_ref(),
            &DockViewportDropRoute::Unavailable,
            "trusted hovered=None must override geometry-only app hits"
        );
        assert_eq!(
            resolution.unavailable_reason(),
            Some(DockViewportDropRouteUnavailableReason::TrustedHoveredNone),
            "trusted hovered=None should stay distinct from an unavailable hovered backend"
        );
    }

    #[test]
    fn trusted_hovered_none_vetoes_same_event_receiver_window_hit() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(130.0), px(250.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "trusted hovered=None is an explicit backend signal and must not be replaced by the event receiver"
        );
    }

    #[test]
    fn trusted_hovered_none_vetoes_floating_payload_source_window_hit() {
        let source = space("source");
        let source_window = handle(1);
        let floating = DockNodeId::null();
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            floating,
            DockViewportDropPayload::Floating(floating),
            point(px(130.0), px(250.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "trusted hovered=None is an explicit backend signal; floating payloads must rely on current no-input/fallback routing instead of event-receiver guesses"
        );
    }

    #[test]
    fn trusted_hovered_none_allows_event_receiver_with_local_scene_proof() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(130.0), px(250.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            ),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
                window_id: source_window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
            },
            "explicit event-receiver scene proof may produce a same-window candidate; workspace delivery still requires the current local target snapshot"
        );
    }

    #[test]
    fn event_receiver_local_allows_same_window_route_with_local_scene_proof() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let signals = signals_with_receiver(
            DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            source_window,
        )
        .with_global_window_bounds(false);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals.clone(),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
                window_id: source_window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
            },
            "local-coordinate backends may use explicit event-receiver scene proof for same-window drops"
        );

        let request_without_scene_proof = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals,
        );
        assert_eq!(
            adapter
                .resolve_payload_drop_route(&request_without_scene_proof, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "event-receiver local coordinates without scene proof must not become a route"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_allows_stale_route_facts_for_event_receiver_local() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        assert!(adapter.mark_window_snapshot_stale(source_window.window_id()));
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            )
            .with_global_window_bounds(false),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
                window_id: source_window.window_id(),
                facts_generation: 2,
                source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
            },
            "same-window scene proof should keep host-local routing alive while adapter route facts wait for the next render"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_rejects_wrong_window() {
        let source = space("source");
        let source_window = handle(1);
        let other_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            )
            .with_global_window_bounds(false),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(&source, other_window, 1)));

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "event-receiver proof must belong to the same window that delivered the event"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_is_ignored_for_source_only_releases() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            )
            .with_global_window_bounds(false),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        assert!(request.event_receiver_local_scene_proof().is_none());
        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
            "event-receiver scene proof belongs to hovered-host render paths; source-only captured release should continue through tear-off policy"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_accepts_no_input_when_scene_generation_matches() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            )
            .with_global_window_bounds(false),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
                window_id: source_window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
            },
            "native no-input is an input mask, not stale route facts, when a matching scene proof exists"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_rejects_minimized_window() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 800.0, 600.0,
            )))
            .with_input_mask(DockViewportInputMask::Minimized),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                source_window,
            )
            .with_global_window_bounds(false),
        )
        .with_event_receiver_local_scene_proof(Some(scene_proof(
            &source,
            source_window,
            1,
        )));

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "event-receiver scene proof must not bypass minimized route readiness"
        );
    }

    #[test]
    fn trusted_hovered_none_allows_tear_off_without_geometry_hit() {
        let source = space("source");
        let target_space = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(40.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &policy);

        assert_eq!(
            route,
            DockViewportDropRoute::TearOff,
            "trusted hovered=None still allows tear-off when no app viewport geometry is hit"
        );
    }

    #[test]
    fn host_release_request_uses_screen_coordinates_when_bounds_are_global() {
        let request = DockViewportDropRouteRequest::from_host_release(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );

        assert_eq!(request.release_position(), point(px(430.0), px(350.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::GlobalScreen
        );
    }

    #[test]
    fn host_release_request_keeps_host_coordinates_without_global_bounds() {
        let request = DockViewportDropRouteRequest::from_host_release(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::EventReceiverLocal
        );
    }

    #[test]
    fn host_release_request_rejects_global_coordinate_space_for_window_local_bounds() {
        let request = DockViewportDropRouteRequest::from_host_release(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local_with_bounds_frame(
                point(px(30.0), px(50.0)),
                DockViewportWindowBoundsFrame::WindowLocal(bounds(400.0, 300.0, 320.0, 240.0)),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::EventReceiverLocal
        );
    }

    #[test]
    fn host_release_request_selects_local_coordinate_space_from_release_origin() {
        let source_window = handle(1);
        let local_signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
        )
        .with_event_receiver_window(source_window)
        .with_global_window_bounds(false);
        let hovered = DockViewportDropRouteRequest::from_host_release(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            local_signals.clone(),
            DockPayloadDropReleaseOrigin::HoveredHost,
        );
        let source_only = DockViewportDropRouteRequest::from_host_release(
            space("source"),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            local_signals,
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        assert_eq!(
            hovered.coordinate_space(),
            DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal
        );
        assert_eq!(
            source_only.coordinate_space(),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
        );
    }

    #[test]
    fn drop_route_rejects_window_stack_when_hovered_window_is_known_empty() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target, target_window);

        for space in [&source, &space("target")] {
            adapter.update_snapshot(
                space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
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
            DockViewportTargetContext::new()
                .with_trusted_hovered_window_known_empty()
                .with_window_stack([target_window, source_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "when the platform can report hovered windows, hovered=None means a foreign or no window is under the pointer"
        );
    }

    #[test]
    fn drop_route_rejects_active_only_overlap_arbitration_as_unavailable() {
        let source = space("source");
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, alpha, alpha_window);
        register_viewport(&mut adapter, zeta, zeta_window);

        for space in [space("alpha"), space("zeta")] {
            adapter.update_snapshot(
                &space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
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
            DockViewportTargetContext::new(),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "active-window alone is only a diagnostic fallback and must not select overlap routing"
        );
    }

    #[test]
    fn drop_route_rejects_overlapping_fallback_only_viewports_as_unavailable() {
        let source = space("source");
        let alpha = space("alpha");
        let zeta = space("zeta");
        let alpha_window = handle(1);
        let zeta_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, alpha, alpha_window);
        register_viewport(&mut adapter, zeta, zeta_window);

        for space in [space("alpha"), space("zeta")] {
            adapter.update_snapshot(
                &space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
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
            DockViewportTargetContext::new(),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "overlapping routes must not be chosen by stable fallback ordering alone"
        );
    }

    #[test]
    fn hovered_host_overlap_route_selects_window_stack_fallback_when_backend_is_unavailable() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);

        for space in [&source, &target] {
            adapter.update_snapshot(
                space,
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 100.0, 320.0, 240.0,
                ))),
                bounds(0.0, 0.0, 320.0, 240.0),
            );
        }

        let route = adapter.resolve_payload_drop_route_with_context(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "hovered-host global releases may use front-to-back window stack fallback when the backend lacks hovered-window signal"
        );
    }

    #[test]
    fn no_input_hovered_viewport_uses_window_stack_fallback_source() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "when backend hover reports a no-input viewport, ImGui-style stack fallback selects the underlay route"
        );
    }

    #[test]
    fn no_input_hovered_viewport_falls_back_to_stack_source() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new()
                    .with_trusted_hovered_window(top_window)
                    .with_window_stack([top_window, underlay_window]),
            ),
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "a backend that still reports a no-input viewport as hovered should be treated as a fallback case, not as a trusted-hovered route target"
        );
    }

    #[test]
    fn no_input_source_requires_fallback_corroboration_when_hovered_signal_is_trusted() {
        let source = space("source");
        let underlay = space("underlay");
        let source_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let signals_without_no_input_hover = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(underlay_window)
                .with_window_stack([underlay_window, source_window]),
        );
        let request_without_no_input_hover = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            signals_without_no_input_hover,
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(
                &request_without_no_input_hover,
                &DockPolicy::default()
            ),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "a trusted hovered-window signal for the route-ready underlay keeps trusted-hovered route selection"
        );

        let signals_without_fallback = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(underlay_window),
        );
        let request_without_fallback = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            signals_without_fallback,
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(&request_without_fallback, &DockPolicy::default()),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "a trusted hovered-window signal keeps trusted-hovered route selection even when no-input fallback is disabled"
        );

        let signals_with_no_input_hover = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new().with_trusted_hovered_window(underlay_window),
        );
        let request_with_no_input_hover = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            signals_with_no_input_hover,
        );

        assert_eq!(
            adapter
                .resolve_payload_drop_route(&request_with_no_input_hover, &DockPolicy::default()),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "a trusted hovered-window signal for the underlay remains authoritative regardless of source no-input facts"
        );
    }

    #[test]
    fn window_stack_fallback_skips_no_input_viewports_from_registry_facts() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let target_context =
            DockViewportTargetContext::new().with_window_stack([top_window, underlay_window]);
        let signals_without_no_input_hover =
            DockViewportPlatformSignals::from_target_context(target_context.clone());
        let request_without_no_input_hover = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            signals_without_no_input_hover,
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(
                &request_without_no_input_hover,
                &DockPolicy::default()
            ),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "front-to-back window stack fallback derives its target from route-ready registry facts and geometry"
        );

        let signals_with_no_input_hover =
            DockViewportPlatformSignals::from_target_context(target_context);
        let request_with_no_input_hover = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            signals_with_no_input_hover,
        );

        assert_eq!(
            adapter
                .resolve_payload_drop_route(&request_with_no_input_hover, &DockPolicy::default()),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "registry no-input facts use the same ImGui-style stack fallback underlay target"
        );
    }

    #[test]
    fn no_input_hovered_stack_fallback_skips_non_routable_entries() {
        let source = space("source");
        let top = space("top");
        let blocker = space("blocker");
        let deeper = space("deeper");
        let top_window = handle(1);
        let blocker_window = handle(2);
        let deeper_window = handle(3);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, blocker.clone(), blocker_window);
        register_viewport(&mut adapter, deeper.clone(), deeper_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &blocker,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::Minimized),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &deeper,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, blocker_window, deeper_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    deeper.clone(),
                    deeper_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "the ImGui-style fallback search skips non-routable viewports and selects the first route-ready underlay"
        );
    }

    #[test]
    fn minimized_hovered_viewport_does_not_inherit_no_input_fallback() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::Minimized),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "minimized windows are not ImGui _NoInputs windows; a minimized hovered signal is rejected instead of being rewritten"
        );
    }

    #[test]
    fn no_input_hovered_stack_fallback_uses_registry_facts_to_select_underlay() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay.clone(),
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "registered no-input route facts should trigger ImGui-style fallback underlay routing"
        );
    }

    #[test]
    fn no_input_hovered_window_stack_fallback_can_resolve_back_to_source() {
        let source = space("source");
        let top = space("top");
        let top_window = handle(1);
        let source_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, source.clone(), source_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, source_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(40.0)),
                window_id: source_window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "no-input hovered signal falls back to the route-ready source viewport when that is the underlay"
        );
    }

    #[test]
    fn registered_not_ready_hovered_viewport_does_not_get_no_input_rewrite() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([top_window, underlay_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "generic not-ready viewports must refresh route facts before routing"
        );
    }

    #[test]
    fn no_input_hovered_stack_fallback_uses_frontmost_route_ready_entry() {
        let source = space("source");
        let top = space("top");
        let underlay = space("underlay");
        let top_window = handle(1);
        let underlay_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, top.clone(), top_window);
        register_viewport(&mut adapter, underlay.clone(), underlay_window);

        adapter.update_snapshot(
            &top,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            )))
            .with_input_mask(DockViewportInputMask::NoInputPassThrough),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &underlay,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let route = adapter.resolve_payload_drop_route_with_context(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(120.0), px(140.0)),
            None,
            &DockPolicy::default(),
            DockViewportTargetContext::new()
                .with_trusted_hovered_window(top_window)
                .with_window_stack([underlay_window, top_window]),
        );

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    underlay,
                    underlay_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "a no-input hovered id should fall back to the frontmost route-ready stack entry"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_does_not_use_rectangle_hits() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                0.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(420.0), px(20.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
            )
            .with_global_window_bounds(false),
        );
        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);

        let route = adapter.resolve_payload_drop_route(&request, &policy);

        assert_eq!(route, DockViewportDropRoute::Unavailable);
    }

    #[test]
    fn drop_route_without_global_window_bounds_keeps_hovered_source_local() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 300.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
                source_window,
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
                window_id: source_window.window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            }
        );
    }

    #[test]
    fn source_only_release_without_global_bounds_applies_tear_off_policy_when_not_local() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 300.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_host_release(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            DockViewportDropReleasePoint::host_local(
                point(px(30.0), px(50.0)),
                bounds(400.0, 300.0, 320.0, 240.0),
            ),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new())
                .with_global_window_bounds(false),
            DockPayloadDropReleaseOrigin::SourceOnly,
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
            "source-only release without a trusted local hit must still honor platform viewport policy"
        );

        let mut policy = DockPolicy::default();
        policy.set_allow_platform_viewports(true);
        let route = adapter.resolve_payload_drop_route(&request, &policy);

        assert_eq!(
            route,
            DockViewportDropRoute::TearOff,
            "source-only release without a trusted local hit should still tear off instead of dropping the release"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_hovered_source_without_local_position_proof()
    {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 300.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(source_window),
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "a hovered-window id alone does not prove that receiver-local coordinates target that window"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_hovered_target_with_source_receiver() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                0.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
                source_window,
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "event-receiver-local coordinates from the source window cannot be applied to a different trusted hovered viewport"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_uses_hovered_target_local_when_receiver_matches() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                0.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
                target_window,
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    point(px(20.0), px(30.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            }
        );
    }

    #[test]
    fn platform_matrix_global_hovered_backend_selects_cross_viewport_route() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::trusted_global_window_bounds_for_test(WindowBounds::Windowed(
                bounds(0.0, 0.0, 320.0, 240.0),
            )),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::trusted_global_window_bounds_for_test(WindowBounds::Windowed(
                bounds(400.0, 0.0, 320.0, 240.0),
            )),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    point(px(20.0), px(30.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "global-bounds backends with trusted hovered-window signal may route cross-viewport"
        );
    }

    #[test]
    fn platform_matrix_global_stack_without_hovered_backend_selects_window_stack_fallback() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::trusted_global_window_bounds_for_test(WindowBounds::Windowed(
                bounds(0.0, 0.0, 320.0, 240.0),
            )),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::trusted_global_window_bounds_for_test(WindowBounds::Windowed(
                bounds(400.0, 0.0, 320.0, 240.0),
            )),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_window_stack([target_window, source_window]),
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    point(px(20.0), px(30.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            "global-bounds backends may use window-stack fallback when hovered-window signal is unavailable"
        );
    }

    #[test]
    fn platform_matrix_wayland_local_hovered_requires_receiver_match() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(
                bounds(0.0, 0.0, 320.0, 240.0),
            )),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::local_only_window_bounds_for_test(WindowBounds::Windowed(
                bounds(0.0, 0.0, 320.0, 240.0),
            )),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let trusted_receiver = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
                target_window,
            )
            .with_global_window_bounds(false),
        );
        let mismatched_receiver = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
                source_window,
            )
            .with_global_window_bounds(false),
        );

        assert_eq!(
            adapter.resolve_payload_drop_route(&trusted_receiver, &DockPolicy::default()),
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    point(px(20.0), px(30.0)),
                    1,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            "Wayland-style local coordinates may route only when hovered window also received the event"
        );
        assert_eq!(
            adapter.resolve_payload_drop_route(&mismatched_receiver, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "hovered id alone does not prove event-receiver-local coordinates target that hovered window"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_event_receiver_source_without_hover() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 300.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(DockViewportTargetContext::new(), source_window)
                .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "receiver-local coordinates do not select a route without hovered-window signal"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_hovered_non_source_without_event_receiver() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                0.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(20.0), px(30.0)),
            None,
            DockViewportPlatformSignals::from_target_context(
                DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(route, DockViewportDropRoute::Unavailable);
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_event_receiver_target_without_hover() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source.clone(), source_window);
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &source,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                0.0, 0.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(DockViewportTargetContext::new(), target_window)
                .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "receiver-local coordinates do not select a cross-viewport route without hovered-window signal"
        );
    }

    #[test]
    fn global_drop_route_rejects_event_receiver_single_hit_when_hovered_window_is_known_empty() {
        let source = space("source");
        let target = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(430.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                target_window,
            ),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "with global bounds, reliable hovered=None means no app viewport is hovered and the event receiver cannot become hovered-window signal"
        );
    }

    #[test]
    fn local_coordinate_drop_route_rejects_event_receiver_when_hovered_window_is_known_empty() {
        let source = space("source");
        let target = space("target");
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target.clone(), target_window);
        adapter.update_snapshot(
            &target,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                400.0, 0.0, 320.0, 240.0,
            ))),
            bounds(10.0, 20.0, 300.0, 200.0),
        );
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals_with_receiver(
                DockViewportTargetContext::new().with_trusted_hovered_window_known_empty(),
                target_window,
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Unavailable,
            "hovered=None blocks receiver-local coordinates from becoming route selection"
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
            DockViewportDropRoute::TearOff
        );
    }

    #[test]
    fn drop_route_request_carries_target_context_from_platform_signals() {
        let source = space("source");
        let source_window = handle(1);
        let target_window = handle(2);
        let target_context = DockViewportTargetContext::new()
            .with_trusted_hovered_window(target_window)
            .with_window_stack([target_window, source_window]);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Tabs,
            point(px(120.0), px(140.0)),
            None,
            DockViewportPlatformSignals::from_target_context(target_context.clone()),
        );

        assert_eq!(request.target_context(), target_context);
    }
}
