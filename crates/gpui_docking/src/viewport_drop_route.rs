use crate::DockViewportTargetContext;
use crate::{
    DockActionApplyError, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId,
    DockViewportAdapter, DockViewportDropPayload, DockViewportPlatformSignals,
    DockViewportTargetHit, DockViewportTearOffRequest,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_target::DockResolvedDropTarget,
    interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{Pixels, Point, WindowBounds, WindowId};

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
    TearOff,
    /// The release landed in a registered viewport that has no current dock target.
    Unavailable,
    /// The release landed outside all registered viewports, but policy forbids opening one.
    Rejected(DockPolicyError),
}

/// All routing and payload facts needed to route one rendered drop release.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportDropRouteRequest {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    drag_session: Option<DockRuntimeDragSession>,
    release_position: Point<Pixels>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    suggested_window_bounds: Option<WindowBounds>,
    target_context: DockViewportTargetContext,
    has_global_window_bounds: bool,
}

/// Delivery facts for a resolved viewport drop route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropDelivery {
    source: DockDropDeliverySource,
    kind: DockDropDeliveryKind,
}

/// Variant-specific delivery target selected for a resolved viewport drop route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDropDeliveryKind {
    /// Commit a route into an already registered viewport host scene.
    Workspace(DockDropWorkspaceTarget),
    /// Open and commit into a new platform viewport.
    TearOff(DockViewportTearOffRequest),
}

/// Route and delivery facts resolved from the same release snapshot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportResolvedDropRoute {
    route: DockViewportDropRoute,
    delivery: Option<DockDropDelivery>,
}

impl DockViewportResolvedDropRoute {
    pub(crate) fn new(route: DockViewportDropRoute, delivery: Option<DockDropDelivery>) -> Self {
        Self { route, delivery }
    }

    pub(crate) fn route(&self) -> &DockViewportDropRoute {
        &self.route
    }

    pub(crate) fn delivery(&self) -> Option<&DockDropDelivery> {
        self.delivery.as_ref()
    }

    pub(crate) fn delivery_result(&self) -> Result<&DockDropDelivery, DockActionApplyError> {
        self.delivery.as_ref().ok_or_else(|| match &self.route {
            DockViewportDropRoute::Rejected(error) => DockActionApplyError::Policy(error.clone()),
            DockViewportDropRoute::Unavailable
            | DockViewportDropRoute::Local { .. }
            | DockViewportDropRoute::KnownViewport { .. }
            | DockViewportDropRoute::TearOff => DockActionApplyError::DropTargetUnavailable,
        })
    }

    #[cfg(test)]
    pub(crate) fn expect_delivery(&self) -> &DockDropDelivery {
        self.delivery
            .as_ref()
            .expect("resolved route should carry a delivery")
    }
}

/// Source facts shared by every resolved drop delivery variant.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropDeliverySource {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    drag_session: Option<DockRuntimeDragSession>,
}

/// Destination facts for an existing-viewport workspace delivery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDropWorkspaceTarget {
    /// Commit against a resolved target snapshot if it still matches current runtime facts.
    Resolved(DockViewportResolvedDropTargetSnapshot),
}

/// Resolved target snapshot captured from a concrete host-scene frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportResolvedDropTargetSnapshot {
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    target: DockResolvedDropTarget,
}

impl DockDropWorkspaceTarget {
    fn resolved(resolved_target: DockViewportResolvedDropTargetSnapshot) -> Self {
        DockDropWorkspaceTarget::Resolved(resolved_target)
    }

    fn routed_preview_target(&self) -> Option<(&DockSpaceId, WindowId, &DockResolvedDropTarget)> {
        match self {
            DockDropWorkspaceTarget::Resolved(target) => target.routed_preview_target(),
        }
    }

    fn accepts_hovered_host_window(
        &self,
        _source_space: &DockSpaceId,
        host_space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        match self {
            DockDropWorkspaceTarget::Resolved(target) => {
                target.accepts_hovered_host_window(host_space, window_id)
            }
        }
    }
}

impl DockDropDeliverySource {
    fn from_request(request: &DockViewportDropRouteRequest) -> Self {
        Self {
            source_space: request.source_space().clone(),
            source_node: request.source_node(),
            payload: request.payload().clone(),
            drag_session: request.drag_session().cloned(),
        }
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    #[cfg(test)]
    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source_node
    }

    #[cfg(test)]
    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        DockSpaceId,
        DockNodeId,
        DockViewportDropPayload,
        Option<DockRuntimeDragSession>,
    ) {
        (
            self.source_space,
            self.source_node,
            self.payload,
            self.drag_session,
        )
    }

    fn accepts_drag_payload(&self, payload: &DockDragPayload) -> bool {
        self.source_space == payload.source_space
            && self.source_node == payload.source_node
            && self.payload == DockViewportDropPayload::from_drag_payload(payload)
    }

    fn payload_mismatch_error(&self) -> DockActionApplyError {
        DockActionApplyError::DropPayloadMismatch {
            space: self.source_space.clone(),
            tabs: self.source_node,
        }
    }
}

impl DockViewportResolvedDropTargetSnapshot {
    pub(crate) fn new(
        target_space: DockSpaceId,
        target_window_id: Option<WindowId>,
        frame: DockViewportHostSceneFrame,
        facts_generation: Option<u64>,
        target: DockResolvedDropTarget,
    ) -> Self {
        Self {
            target_space,
            target_window_id,
            frame,
            facts_generation,
            target,
        }
    }

    pub(crate) fn frame(&self) -> &DockViewportHostSceneFrame {
        &self.frame
    }

    pub(crate) fn facts_generation(&self) -> Option<u64> {
        self.facts_generation
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn target_window_id(&self) -> Option<WindowId> {
        self.target_window_id
    }

    pub(crate) fn into_target(self) -> DockResolvedDropTarget {
        self.target
    }

    fn target(&self) -> &DockResolvedDropTarget {
        &self.target
    }

    fn routed_preview_target(&self) -> Option<(&DockSpaceId, WindowId, &DockResolvedDropTarget)> {
        Some((&self.target_space, self.target_window_id?, self.target()))
    }

    fn accepts_hovered_host_window(&self, host_space: &DockSpaceId, window_id: WindowId) -> bool {
        target_accepts_hovered_host_window(
            &self.target_space,
            self.target_window_id,
            host_space,
            window_id,
        )
    }
}

fn target_accepts_hovered_host_window(
    target_space: &DockSpaceId,
    target_window_id: Option<WindowId>,
    host_space: &DockSpaceId,
    window_id: WindowId,
) -> bool {
    match target_window_id {
        Some(target_window_id) => target_window_id == window_id,
        None => target_space == host_space,
    }
}

impl DockDropDelivery {
    #[cfg(test)]
    pub(crate) fn from_route_request(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
    ) -> Option<Self> {
        Self::from_route_request_with_resolved_target(request, route, None)
    }

    pub(crate) fn from_route_request_with_resolved_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        resolved_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Option<Self> {
        let source = DockDropDeliverySource::from_request(request);
        let kind = match route {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                DockDropDeliveryKind::Workspace(DockDropWorkspaceTarget::resolved(resolved_target?))
            }
            DockViewportDropRoute::TearOff => {
                DockDropDeliveryKind::TearOff(tear_off_request_from_drop_route_request(request))
            }
            DockViewportDropRoute::Unavailable | DockViewportDropRoute::Rejected(_) => {
                return None;
            }
        };
        Some(Self { source, kind })
    }

    pub(crate) fn routed_preview_target(
        &self,
    ) -> Option<(&DockSpaceId, WindowId, &DockResolvedDropTarget)> {
        match &self.kind {
            DockDropDeliveryKind::Workspace(delivery) => delivery.routed_preview_target(),
            DockDropDeliveryKind::TearOff(_) => None,
        }
    }

    pub(crate) fn drag_session_id(&self) -> Option<u64> {
        self.source.drag_session().map(DockRuntimeDragSession::id)
    }

    #[cfg(test)]
    pub(crate) fn source_space(&self) -> &DockSpaceId {
        self.source.source_space()
    }

    #[cfg(test)]
    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source.source_node()
    }

    #[cfg(test)]
    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        self.source.payload()
    }

    fn accepts_drag_payload(&self, payload: &DockDragPayload) -> bool {
        self.source.accepts_drag_payload(payload)
    }

    fn accepts_hovered_host_window(&self, host_space: &DockSpaceId, window_id: WindowId) -> bool {
        match &self.kind {
            DockDropDeliveryKind::Workspace(delivery) => delivery.accepts_hovered_host_window(
                self.source.source_space(),
                host_space,
                window_id,
            ),
            DockDropDeliveryKind::TearOff(_) => false,
        }
    }

    fn payload_mismatch_error(&self) -> DockActionApplyError {
        self.source.payload_mismatch_error()
    }

    #[cfg(test)]
    pub(crate) fn kind(&self) -> &DockDropDeliveryKind {
        &self.kind
    }

    pub(crate) fn into_parts(self) -> (DockDropDeliverySource, DockDropDeliveryKind) {
        (self.source, self.kind)
    }

    #[cfg(test)]
    pub(crate) fn tear_off_request(&self) -> Option<DockViewportTearOffRequest> {
        match &self.kind {
            DockDropDeliveryKind::TearOff(request) => Some(request.clone()),
            DockDropDeliveryKind::Workspace(_) => None,
        }
    }

    pub(crate) fn into_tear_off_request(self) -> Result<DockViewportTearOffRequest, Self> {
        let Self { source, kind } = self;
        match kind {
            DockDropDeliveryKind::TearOff(request) => Ok(request),
            kind => Err(Self { source, kind }),
        }
    }

    pub(crate) fn release_authority_for_cached_preview(
        &self,
        origin: DockPayloadDropReleaseOrigin,
        host_space: &DockSpaceId,
        window_id: WindowId,
        payload: &DockDragPayload,
    ) -> Result<bool, DockActionApplyError> {
        if origin != DockPayloadDropReleaseOrigin::HoveredHost {
            return Ok(false);
        }
        if !self.accepts_hovered_host_window(host_space, window_id) {
            return Ok(false);
        }
        if !self.accepts_drag_payload(payload) {
            return Err(self.payload_mismatch_error());
        }
        Ok(true)
    }
}

fn tear_off_request_from_drop_route_request(
    request: &DockViewportDropRouteRequest,
) -> DockViewportTearOffRequest {
    DockViewportTearOffRequest::new(
        request.source_space().clone(),
        request.source_node(),
        request.payload().clone(),
        request.release_position(),
        request.suggested_window_bounds(),
    )
    .with_drag_session(request.drag_session().cloned())
    .with_tear_off_geometry(request.tear_off_geometry())
}

impl DockViewportDropRouteRequest {
    fn new(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: DockViewportTargetContext,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_node,
            payload,
            drag_session: None,
            release_position,
            tear_off_geometry: None,
            suggested_window_bounds,
            target_context,
            has_global_window_bounds: true,
        }
    }

    pub(crate) fn from_platform_signals(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        let has_global_window_bounds = platform_signals.has_global_window_bounds();
        let mut request = Self::new(
            source_space,
            source_node,
            payload,
            release_position,
            suggested_window_bounds,
            platform_signals.into(),
        );
        request.has_global_window_bounds = has_global_window_bounds;
        request
    }

    pub(crate) fn with_drag_session(
        mut self,
        drag_session: Option<DockRuntimeDragSession>,
    ) -> Self {
        self.drag_session = drag_session;
        self
    }

    pub(crate) fn with_tear_off_geometry(
        mut self,
        tear_off_geometry: Option<DockDragTearOffGeometry>,
    ) -> Self {
        self.tear_off_geometry = tear_off_geometry;
        self
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source_node
    }

    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.payload
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
    }

    pub(crate) fn release_position(&self) -> Point<Pixels> {
        self.release_position
    }

    pub(crate) fn tear_off_geometry(&self) -> Option<DockDragTearOffGeometry> {
        self.tear_off_geometry
    }

    pub(crate) fn suggested_window_bounds(&self) -> Option<WindowBounds> {
        self.suggested_window_bounds
    }

    pub(crate) fn target_context(&self) -> &DockViewportTargetContext {
        &self.target_context
    }

    pub(crate) fn has_global_window_bounds(&self) -> bool {
        self.has_global_window_bounds
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        target_context: DockViewportTargetContext,
    ) -> Self {
        Self::new(
            source_space,
            source_node,
            payload,
            release_position,
            suggested_window_bounds,
            target_context,
        )
    }
}

impl DockViewportAdapter {
    pub(crate) fn resolve_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &DockPolicy,
    ) -> DockViewportDropRoute {
        let target_context = request.target_context();
        if let Some(hovered_window) = target_context.hovered_window()
            && self.window_route_ready(hovered_window) == Some(false)
        {
            return DockViewportDropRoute::Unavailable;
        }
        if !request.has_global_window_bounds() {
            let route_local_window = target_context
                .hovered_window()
                .or_else(|| target_context.event_receiver_window());
            if let Some(hovered_window) = route_local_window
                && self
                    .window_for_space(request.source_space())
                    .is_some_and(|window| {
                        window.window_id() == hovered_window
                            && self.window_route_ready(hovered_window) == Some(true)
                    })
                && let Some(host_position) =
                    self.window_to_host(request.source_space(), request.release_position())
            {
                return DockViewportDropRoute::Local { host_position };
            }
            return DockViewportDropRoute::Unavailable;
        }
        if let Some(resolution) =
            self.resolve_viewport_route_target(request.release_position(), target_context)
        {
            if !resolution.is_trusted() {
                return DockViewportDropRoute::Unavailable;
            }

            let target = resolution.into_target();
            if target.space() == request.source_space() {
                return DockViewportDropRoute::Local {
                    host_position: target.host_position(),
                };
            }

            return DockViewportDropRoute::KnownViewport { target };
        }

        if let Err(reason) = policy.validate_platform_viewports() {
            return DockViewportDropRoute::Rejected(reason);
        }

        DockViewportDropRoute::TearOff
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
        DockPolicy, DockViewportWindowFacts,
        drag::{DockDragPayload, DockDragTearOffGeometry},
        drop_target::{DockDropResolveSource, DockResolvedDropTarget, DockResolvedDropTargetKind},
        interaction::DockRuntimeDragSession,
        viewport_drop_scene::{DockViewportHostSceneRegistry, DockViewportHostSceneSnapshot},
        viewport_test_support::{bounds, handle, item, space},
    };
    use open_gpui::{DisplayId, WindowBounds, WindowId, point, px};
    use slotmap::Key;

    #[test]
    fn drop_route_treats_source_viewport_hit_as_local() {
        let main = space("main");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(main.clone(), window);
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
            DockViewportDropRoute::Local {
                host_position: point(px(5.0), px(5.0)),
            }
        );
    }

    #[test]
    fn drop_route_uses_trusted_window_stack_arbitration_for_known_viewport() {
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
                    zeta,
                    zeta_window,
                    point(px(20.0), px(40.0)),
                    1,
                ),
            }
        );
    }

    #[test]
    fn drop_route_rejects_window_stack_when_hovered_window_is_known_empty() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source.clone(), source_window);
        adapter.register_viewport(target, target_window);

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
                .with_hovered_window_known_empty()
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
        adapter.register_viewport(alpha, alpha_window);
        adapter.register_viewport(zeta, zeta_window);

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
            "active-window alone is only a diagnostic fallback and must not authorize overlap commits"
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
        adapter.register_viewport(alpha, alpha_window);
        adapter.register_viewport(zeta, zeta_window);

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
            "overlapping route commits must not be chosen by stable fallback ordering alone"
        );
    }

    #[test]
    fn source_only_overlap_route_uses_window_stack_without_hovered_source() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source.clone(), source_window);
        adapter.register_viewport(target.clone(), target_window);

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
            },
            "source-only routes must not treat the source event window as hovered"
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_does_not_use_rectangle_hits() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source.clone(), source_window);
        adapter.register_viewport(target.clone(), target_window);
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
        adapter.register_viewport(source.clone(), source_window);
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
                DockViewportTargetContext::new().with_hovered_window(source_window),
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
            }
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_keeps_event_receiver_source_local() {
        let source = space("source");
        let source_window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source.clone(), source_window);
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
                DockViewportTargetContext::new().with_event_receiver_window(source_window),
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(
            route,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(30.0)),
            }
        );
    }

    #[test]
    fn drop_route_without_global_window_bounds_rejects_hovered_non_source() {
        let source = space("source");
        let target = space("target");
        let source_window = handle(1);
        let target_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        adapter.register_viewport(source.clone(), source_window);
        adapter.register_viewport(target.clone(), target_window);
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
                DockViewportTargetContext::new().with_hovered_window(target_window),
            )
            .with_global_window_bounds(false),
        );

        let route = adapter.resolve_payload_drop_route(&request, &DockPolicy::default());

        assert_eq!(route, DockViewportDropRoute::Unavailable);
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
    fn local_drop_delivery_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let local_position = point(px(5.0), px(7.0));

        let delivery = DockDropDelivery::from_route_request(
            &request,
            DockViewportDropRoute::Local {
                host_position: local_position,
            },
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn known_viewport_drop_delivery_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));

        let delivery = DockDropDelivery::from_route_request(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::new(target.clone(), target_window, known_position),
            },
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn known_viewport_drop_delivery_uses_resolved_snapshot() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            point(px(900.0), px(900.0)),
            None,
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));
        let target_facts_generation = 41;
        let resolved_target = resolved_drop_target_snapshot(
            target.clone(),
            target_window.window_id(),
            target_facts_generation,
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::with_facts_generation(
                    target,
                    target_window,
                    known_position,
                    target_facts_generation,
                ),
            },
            Some(resolved_target.clone()),
        )
        .expect("resolved known viewport route should derive a delivery");
        let DockDropDeliveryKind::Workspace(known) = delivery.kind() else {
            panic!("resolved known viewport route should derive a workspace commit");
        };
        assert_eq!(delivery.drag_session_id(), Some(drag_session.id()));
        assert_eq!(delivery.source_space(), &source);
        assert_eq!(delivery.source_node(), source_tabs);
        assert_eq!(delivery.payload(), &DockViewportDropPayload::Item(item));
        assert_eq!(known, &DockDropWorkspaceTarget::Resolved(resolved_target));
    }

    #[test]
    fn drop_delivery_derives_tear_off_request_from_route_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let release_position = point(px(900.0), px(900.0));
        let suggested_window_bounds = WindowBounds::Windowed(bounds(880.0, 880.0, 360.0, 240.0));
        let tear_off_geometry = DockDragTearOffGeometry::from_source_bounds(
            bounds(200.0, 120.0, 480.0, 300.0),
            point(px(260.0), px(150.0)),
        );
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(21, &drag_payload);
        let request = DockViewportDropRouteRequest::from_target_context(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item.clone()),
            release_position,
            Some(suggested_window_bounds),
            DockViewportTargetContext::new(),
        )
        .with_drag_session(Some(drag_session.clone()))
        .with_tear_off_geometry(Some(tear_off_geometry));
        let route = DockViewportDropRoute::TearOff;

        assert_eq!(
            DockDropDelivery::from_route_request(&request, route)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .as_ref(),
            Some(
                &DockViewportTearOffRequest::new(
                    source,
                    source_tabs,
                    DockViewportDropPayload::Item(item),
                    release_position,
                    Some(suggested_window_bounds),
                )
                .with_drag_session(Some(drag_session))
                .with_tear_off_geometry(Some(tear_off_geometry))
            )
        );
    }

    #[test]
    fn drop_route_request_carries_target_context_from_platform_signals() {
        let source = space("source");
        let source_window = handle(1);
        let target_window = handle(2);
        let target_context = DockViewportTargetContext::new()
            .with_hovered_window(target_window)
            .with_window_stack([target_window, source_window]);
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Tabs,
            point(px(120.0), px(140.0)),
            None,
            DockViewportPlatformSignals::from_target_context(target_context.clone()),
        );

        assert_eq!(request.target_context(), &target_context);
    }

    fn resolved_drop_target_snapshot(
        target_space: DockSpaceId,
        target_window_id: WindowId,
        facts_generation: u64,
    ) -> DockViewportResolvedDropTargetSnapshot {
        let mut registry = DockViewportHostSceneRegistry::default();
        let frame = registry
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window_id,
                bounds(0.0, 0.0, 320.0, 240.0),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(0.0), px(0.0)),
            ))
            .frame;
        DockViewportResolvedDropTargetSnapshot::new(
            target_space.clone(),
            Some(target_window_id),
            frame,
            Some(facts_generation),
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::EmptyDockSpace {
                    space: target_space,
                    is_central: false,
                },
                source: DockDropResolveSource::EmptyDockSpace,
                drop_box: None,
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                is_central_region: false,
            },
        )
    }
}
