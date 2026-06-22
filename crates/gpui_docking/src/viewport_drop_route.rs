use crate::DockViewportTargetContext;
use crate::{
    DockActionApplyError, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId,
    DockViewportAdapter, DockViewportAuthorizedRouteAuthority, DockViewportDropPayload,
    DockViewportPlatformSignals, DockViewportTargetHit, DockViewportTearOffRequest,
    DockViewportWindowHit,
    drag::DockDragTearOffGeometry,
    drop_target::{DockDropTargetKey, DockResolvedDropTarget},
    interaction::{DockPayloadDropReleaseOrigin, DockRuntimeDragSession},
    viewport_drop_scene::DockViewportHostSceneFrame,
    viewport_registry::{DockViewportInputMask, DockViewportWindowBoundsFrame},
    viewport_target_resolver::{
        DockAuthorizedViewportRouteTarget, resolve_authorized_viewport_route_target,
    },
};
use open_gpui::{AnyWindowHandle, Bounds, Pixels, Point, Size, WindowBounds, WindowId};

/// Runtime route for a rendered drag release before workspace mutation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockViewportDropRoute {
    /// The release is still in the source viewport, so the source host should commit locally.
    Local {
        /// Local host position for the release.
        host_position: Point<Pixels>,
        /// GPUI window that rendered the local source viewport.
        window_id: WindowId,
        /// Route-facts generation that was current for the local source viewport.
        facts_generation: u64,
        /// Authority that selected the local source viewport.
        authority: DockViewportAuthorizedRouteAuthority,
    },
    /// The release landed inside another registered viewport.
    KnownViewport {
        /// Destination viewport hit and its owning runtime window.
        target: DockViewportTargetHit,
        /// Authority that selected the destination viewport.
        authority: DockViewportAuthorizedRouteAuthority,
    },
    /// The release landed outside all registered viewports and may open a new platform viewport.
    TearOff,
    /// The release landed in a registered viewport that has no current dock target.
    Unavailable,
    /// The release landed outside all registered viewports, but policy forbids opening one.
    Rejected(DockPolicyError),
}

/// Why a route resolved to `DockViewportDropRoute::Unavailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportDropRouteUnavailableReason {
    /// The pointer is inside a registered viewport window, but that window cannot currently provide
    /// a host target. The release must not replay an underlay preview through this opaque window.
    BlockedByViewportWindow,
    /// A viewport window or host target was present, but no current backend authority selected a
    /// commit-capable route.
    NoViewportAuthority,
    /// The backend explicitly reported hovered=None for this snapshot. This must not be treated as
    /// an unavailable backend signal; hovered-host releases cannot replay through it.
    TrustedHoveredNone,
}

/// A viewport drop route plus internal routing diagnostics used by runtime replay policy.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportDropRouteResolution {
    route: DockViewportDropRoute,
    unavailable_reason: Option<DockViewportDropRouteUnavailableReason>,
}

#[derive(Debug, Clone, PartialEq)]
enum DockViewportDropRoutePlan {
    Route(DockViewportDropRoute),
    Unavailable(DockViewportDropRouteUnavailableReason),
    OutsideRegisteredViewport,
}

impl DockViewportDropRoutePlan {
    fn route(route: DockViewportDropRoute) -> Self {
        Self::Route(route)
    }

    fn unavailable(reason: DockViewportDropRouteUnavailableReason) -> Self {
        Self::Unavailable(reason)
    }

    fn into_resolution(self, policy: &DockPolicy) -> DockViewportDropRouteResolution {
        match self {
            Self::Route(route) => DockViewportDropRouteResolution::route(route),
            Self::Unavailable(reason) => DockViewportDropRouteResolution::unavailable(reason),
            Self::OutsideRegisteredViewport => match policy.validate_platform_viewports() {
                Ok(()) => DockViewportDropRouteResolution::route(DockViewportDropRoute::TearOff),
                Err(reason) => {
                    DockViewportDropRouteResolution::route(DockViewportDropRoute::Rejected(reason))
                }
            },
        }
    }
}

impl DockViewportDropRouteResolution {
    fn route(route: DockViewportDropRoute) -> Self {
        Self {
            route,
            unavailable_reason: None,
        }
    }

    fn unavailable(reason: DockViewportDropRouteUnavailableReason) -> Self {
        Self {
            route: DockViewportDropRoute::Unavailable,
            unavailable_reason: Some(reason),
        }
    }

    pub(crate) fn into_route(self) -> DockViewportDropRoute {
        self.route
    }

    pub(crate) fn route_ref(&self) -> &DockViewportDropRoute {
        &self.route
    }

    pub(crate) fn target_window(&self, adapter: &DockViewportAdapter) -> Option<AnyWindowHandle> {
        self.route.target_window(adapter)
    }

    pub(crate) fn unavailable_reason(&self) -> Option<DockViewportDropRouteUnavailableReason> {
        self.unavailable_reason
    }
}

impl DockViewportDropRoute {
    fn target_window(&self, adapter: &DockViewportAdapter) -> Option<AnyWindowHandle> {
        match self {
            DockViewportDropRoute::Local { window_id, .. } => adapter
                .space_for_window_id(*window_id)
                .and_then(|space| adapter.window_for_space(space)),
            DockViewportDropRoute::KnownViewport { target, .. } => {
                let window = adapter.window_for_space(target.space())?;
                (window.window_id() == target.window_id()).then_some(window)
            }
            DockViewportDropRoute::TearOff
            | DockViewportDropRoute::Unavailable
            | DockViewportDropRoute::Rejected(_) => None,
        }
    }
}

/// Coordinate space used by `DockViewportDropRouteRequest::release_position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPointerCoordinateSpace {
    /// `release_position` is a screen-space point and may be geometry hit-tested globally.
    GlobalScreen,
    /// `release_position` is local to the trusted hovered window.
    TrustedHoveredWindowLocal,
    /// `release_position` is local to the event-receiver window, but no trusted hovered-window
    /// signal proves that the receiver is the hovered window.
    EventReceiverLocal,
    /// `release_position` is local to the source host only.
    SourceLocalOnly,
}

/// All routing and payload facts needed to route one rendered drop release.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportDropRouteRequest {
    source_space: DockSpaceId,
    source_node: DockNodeId,
    payload: DockViewportDropPayload,
    drag_session: Option<DockRuntimeDragSession>,
    tear_off_geometry: Option<DockDragTearOffGeometry>,
    suggested_window_bounds: Option<WindowBounds>,
    release_position: Point<Pixels>,
    coordinate_space: DockViewportPointerCoordinateSpace,
    release_origin: DockPayloadDropReleaseOrigin,
    event_receiver_local_scene_proof: Option<DockViewportHostSceneFrame>,
    platform_signals: DockViewportPlatformSignals,
}

/// Raw pointer release facts before viewport routing normalizes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportDropReleasePoint {
    host_position: Point<Pixels>,
    host_window_bounds: DockViewportWindowBoundsFrame,
}

/// Hovered-window-local drop facts authorized by a trusted hovered-window signal.
#[derive(Debug, Clone, PartialEq)]
struct DockTrustedHoveredWindowLocalDropTarget {
    target_space: DockSpaceId,
    target_window: AnyWindowHandle,
    host_position: Point<Pixels>,
    facts_generation: u64,
}

enum DockEventReceiverLocalSceneAuthorityMode {
    HitTestedScene,
    ReceiverSceneProof,
}

struct DockEventReceiverLocalSceneAuthority {
    receiver_window: WindowId,
    facts_generation: u64,
    host_bounds: Bounds<Pixels>,
    global_screen_bounds: Option<Bounds<Pixels>>,
}

impl DockEventReceiverLocalSceneAuthority {
    fn host_position_from_window_position(
        &self,
        window_position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        if !self.host_bounds.contains(&window_position) {
            return None;
        }
        Some(open_gpui::point(
            window_position.x - self.host_bounds.origin.x,
            window_position.y - self.host_bounds.origin.y,
        ))
    }

    fn local_route(&self, host_position: Point<Pixels>) -> DockViewportDropRoute {
        DockViewportDropRoute::Local {
            host_position,
            window_id: self.receiver_window,
            facts_generation: self.facts_generation,
            authority: DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene,
        }
    }
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
    Workspace(DockViewportResolvedDropTargetSnapshot),
    /// Open and commit into a new platform viewport.
    TearOff(DockViewportTearOffRequest),
}

/// Route and delivery facts resolved from the same release snapshot.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportResolvedDropRoute {
    route: DockViewportDropRoute,
    delivery: Option<DockDropDelivery>,
    preview_target: Option<DockViewportResolvedDropTargetSnapshot>,
}

impl DockViewportResolvedDropRoute {
    #[cfg(test)]
    pub(crate) fn new(route: DockViewportDropRoute, delivery: Option<DockDropDelivery>) -> Self {
        Self {
            route,
            delivery,
            preview_target: None,
        }
    }

    pub(crate) fn with_preview_target(
        route: DockViewportDropRoute,
        delivery: Option<DockDropDelivery>,
        preview_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Self {
        debug_assert!(
            delivery.is_none() || crate::delivery_authority_for_route(&route).is_some(),
            "resolved viewport routes may carry delivery only when the route has release authority"
        );
        Self {
            route,
            delivery,
            preview_target,
        }
    }

    pub(crate) fn route(&self) -> &DockViewportDropRoute {
        &self.route
    }

    pub(crate) fn delivery(&self) -> Option<&DockDropDelivery> {
        self.delivery.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn preview_target(&self) -> Option<&DockViewportResolvedDropTargetSnapshot> {
        self.preview_target.as_ref()
    }

    pub(crate) fn routed_preview_target_snapshot(
        &self,
    ) -> Option<&DockViewportResolvedDropTargetSnapshot> {
        self.preview_target.as_ref().or_else(|| {
            self.delivery
                .as_ref()
                .and_then(|delivery| match &delivery.kind {
                    DockDropDeliveryKind::Workspace(target) => Some(target),
                    DockDropDeliveryKind::TearOff(_) => None,
                })
        })
    }

    pub(crate) fn without_delivery(self) -> Self {
        Self {
            delivery: None,
            ..self
        }
    }

    pub(crate) fn into_authorized_delivery(self) -> Result<DockDropDelivery, DockActionApplyError> {
        if crate::delivery_authority_for_route(&self.route).is_none() {
            return Err(self.route.delivery_error());
        }
        self.delivery.ok_or_else(|| self.route.delivery_error())
    }

    #[cfg(test)]
    pub(crate) fn expect_delivery(&self) -> &DockDropDelivery {
        self.delivery
            .as_ref()
            .expect("resolved route should carry a delivery")
    }
}

impl PartialEq for DockViewportResolvedDropRoute {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
            && self.delivery == other.delivery
            && self.preview_target == other.preview_target
    }
}

impl DockViewportDropRoute {
    fn delivery_error(&self) -> DockActionApplyError {
        match self {
            Self::Rejected(error) => DockActionApplyError::Policy(error.clone()),
            Self::Unavailable | Self::Local { .. } | Self::KnownViewport { .. } | Self::TearOff => {
                DockActionApplyError::DropTargetUnavailable
            }
        }
    }
}

fn unavailable_route_authority_reason(
    target_context: &DockViewportTargetContext,
) -> DockViewportDropRouteUnavailableReason {
    match target_context.trusted_hovered_signal() {
        crate::DockViewportTrustedHoveredSignal::TrustedNone => {
            DockViewportDropRouteUnavailableReason::TrustedHoveredNone
        }
        crate::DockViewportTrustedHoveredSignal::Unavailable
        | crate::DockViewportTrustedHoveredSignal::Trusted(_) => {
            DockViewportDropRouteUnavailableReason::NoViewportAuthority
        }
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

/// Resolved target snapshot captured from a concrete host-scene frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportResolvedDropTargetSnapshot {
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    host_position: Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    target_key: DockDropTargetKey,
    target: DockResolvedDropTarget,
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

    #[cfg(test)]
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
}

impl DockViewportResolvedDropTargetSnapshot {
    pub(crate) fn new(
        target_space: DockSpaceId,
        target_window_id: Option<WindowId>,
        frame: DockViewportHostSceneFrame,
        facts_generation: Option<u64>,
        host_position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        target: DockResolvedDropTarget,
    ) -> Self {
        let target_key = target.target_key();
        Self {
            target_space,
            target_window_id,
            frame,
            facts_generation,
            host_position,
            payload_size,
            target_key,
            target,
        }
    }

    pub(crate) fn facts_generation(&self) -> Option<u64> {
        self.facts_generation
    }

    pub(crate) fn frame(&self) -> &DockViewportHostSceneFrame {
        &self.frame
    }

    pub(crate) fn host_position(&self) -> Point<Pixels> {
        self.host_position
    }

    pub(crate) fn payload_size(&self) -> Option<Size<Pixels>> {
        self.payload_size
    }

    pub(crate) fn target_key(&self) -> &DockDropTargetKey {
        &self.target_key
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

    pub(crate) fn target(&self) -> &DockResolvedDropTarget {
        &self.target
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
        let _authority = crate::delivery_authority_for_route(&route)?;
        let source = DockDropDeliverySource::from_request(request);
        let kind = match route {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                DockDropDeliveryKind::Workspace(resolved_target?)
            }
            DockViewportDropRoute::TearOff => {
                DockDropDeliveryKind::TearOff(tear_off_request_from_drop_route_request(request))
            }
            DockViewportDropRoute::Unavailable | DockViewportDropRoute::Rejected(_) => return None,
        };
        Some(Self { source, kind })
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

    #[cfg(test)]
    pub(crate) fn kind(&self) -> &DockDropDeliveryKind {
        &self.kind
    }

    pub(crate) fn into_parts(self) -> (DockDropDeliverySource, DockDropDeliveryKind) {
        (self.source, self.kind)
    }

    #[cfg(test)]
    pub(crate) fn parts(&self) -> (&DockDropDeliverySource, &DockDropDeliveryKind) {
        (&self.source, &self.kind)
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

    pub(crate) fn from_resolution(
        resolution: DockViewportResolvedDropRoute,
    ) -> Result<Self, DockActionApplyError> {
        resolution.into_authorized_delivery()
    }
}

fn tear_off_request_from_drop_route_request(
    request: &DockViewportDropRouteRequest,
) -> DockViewportTearOffRequest {
    // Only preserve an authoritative screen-space release point for tear-off placement.
    // Local coordinates remain routing facts only and must not anchor a new viewport.
    let release_position = match request.coordinate_space() {
        DockViewportPointerCoordinateSpace::GlobalScreen => Some(request.release_position()),
        DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal
        | DockViewportPointerCoordinateSpace::EventReceiverLocal
        | DockViewportPointerCoordinateSpace::SourceLocalOnly => None,
    };
    DockViewportTearOffRequest::new(
        request.source_space().clone(),
        request.source_node(),
        request.payload().clone(),
        release_position,
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
        suggested_window_bounds: Option<WindowBounds>,
        release_position: Point<Pixels>,
        coordinate_space: DockViewportPointerCoordinateSpace,
        release_origin: DockPayloadDropReleaseOrigin,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            source_node,
            payload,
            drag_session: None,
            tear_off_geometry: None,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            event_receiver_local_scene_proof: None,
            platform_signals,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_platform_signals(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        let release_origin = DockPayloadDropReleaseOrigin::HoveredHost;
        let coordinate_space = if platform_signals.has_global_window_bounds() {
            DockViewportPointerCoordinateSpace::GlobalScreen
        } else {
            Self::local_coordinate_space_for_origin(
                release_origin,
                &platform_signals.target_context(),
                platform_signals.event_receiver_window(),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_platform_signals_with_origin(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_position: Point<Pixels>,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
    ) -> Self {
        let coordinate_space = if platform_signals.has_global_window_bounds() {
            DockViewportPointerCoordinateSpace::GlobalScreen
        } else {
            Self::local_coordinate_space_for_origin(
                release_origin,
                &platform_signals.target_context(),
                platform_signals.event_receiver_window(),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
    }

    pub(crate) fn from_host_release(
        source_space: impl Into<DockSpaceId>,
        source_node: DockNodeId,
        payload: DockViewportDropPayload,
        release_point: DockViewportDropReleasePoint,
        suggested_window_bounds: Option<WindowBounds>,
        platform_signals: DockViewportPlatformSignals,
        release_origin: DockPayloadDropReleaseOrigin,
    ) -> Self {
        let (release_position, coordinate_space) = if platform_signals.has_global_window_bounds() {
            if let Some(host_window_bounds) =
                release_point.host_window_bounds.global_screen_bounds()
            {
                (
                    open_gpui::point(
                        host_window_bounds.origin.x + release_point.host_position.x,
                        host_window_bounds.origin.y + release_point.host_position.y,
                    ),
                    DockViewportPointerCoordinateSpace::GlobalScreen,
                )
            } else {
                (
                    release_point.host_position,
                    Self::local_coordinate_space_for_origin(
                        release_origin,
                        &platform_signals.target_context(),
                        platform_signals.event_receiver_window(),
                    ),
                )
            }
        } else {
            (
                release_point.host_position,
                Self::local_coordinate_space_for_origin(
                    release_origin,
                    &platform_signals.target_context(),
                    platform_signals.event_receiver_window(),
                ),
            )
        };
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            coordinate_space,
            release_origin,
            platform_signals,
        )
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

    pub(crate) fn with_event_receiver_local_scene_proof(
        mut self,
        proof: Option<DockViewportHostSceneFrame>,
    ) -> Self {
        self.event_receiver_local_scene_proof =
            if self.release_origin == DockPayloadDropReleaseOrigin::HoveredHost {
                proof
            } else {
                None
            };
        self
    }

    pub(crate) fn with_resampled_platform_target_context_from_app(
        mut self,
        cx: &open_gpui::App,
    ) -> Self {
        self.platform_signals = self
            .platform_signals
            .with_resampled_target_context_from_app(cx);
        self
    }

    pub(crate) fn with_last_hovered_viewport_window(mut self, window_id: WindowId) -> Self {
        self.platform_signals = self
            .platform_signals
            .with_last_hovered_viewport_window(window_id);
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

    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        self.platform_signals.target_context()
    }

    pub(crate) fn event_receiver_window(&self) -> Option<WindowId> {
        self.platform_signals.event_receiver_window()
    }

    pub(crate) fn coordinate_space(&self) -> DockViewportPointerCoordinateSpace {
        self.coordinate_space
    }

    pub(crate) fn release_origin(&self) -> DockPayloadDropReleaseOrigin {
        self.release_origin
    }

    pub(crate) fn event_receiver_local_scene_proof(&self) -> Option<&DockViewportHostSceneFrame> {
        self.event_receiver_local_scene_proof.as_ref()
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
        let platform_signals =
            DockViewportPlatformSignals::from_target_context(target_context.clone());
        Self::new(
            source_space,
            source_node,
            payload,
            suggested_window_bounds,
            release_position,
            DockViewportPointerCoordinateSpace::GlobalScreen,
            DockPayloadDropReleaseOrigin::HoveredHost,
            platform_signals,
        )
    }

    fn local_coordinate_space_for_origin(
        release_origin: DockPayloadDropReleaseOrigin,
        target_context: &DockViewportTargetContext,
        event_receiver_window: Option<WindowId>,
    ) -> DockViewportPointerCoordinateSpace {
        match release_origin {
            DockPayloadDropReleaseOrigin::HoveredHost => {
                if target_context
                    .trusted_hovered_window_matches_event_receiver(event_receiver_window)
                {
                    DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal
                } else {
                    DockViewportPointerCoordinateSpace::EventReceiverLocal
                }
            }
            DockPayloadDropReleaseOrigin::SourceOnly => {
                DockViewportPointerCoordinateSpace::SourceLocalOnly
            }
        }
    }
}

impl DockViewportDropReleasePoint {
    #[cfg(test)]
    pub(crate) fn host_local(
        host_position: Point<Pixels>,
        host_window_bounds: Bounds<Pixels>,
    ) -> Self {
        Self::host_local_with_bounds_frame(
            host_position,
            DockViewportWindowBoundsFrame::GlobalScreen(host_window_bounds),
        )
    }

    pub(crate) fn host_local_with_bounds_frame(
        host_position: Point<Pixels>,
        host_window_bounds: DockViewportWindowBoundsFrame,
    ) -> Self {
        Self {
            host_position,
            host_window_bounds,
        }
    }
}

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
            .into_resolution(policy)
    }

    fn normalize_target_context(
        &self,
        target_context: DockViewportTargetContext,
    ) -> DockViewportTargetContext {
        let Some(hovered_window) = target_context.trusted_hovered_window() else {
            return target_context;
        };
        if self.window_input_mask(hovered_window) == Some(DockViewportInputMask::NoInputPassThrough)
        {
            return target_context.without_trusted_hovered_window();
        }
        target_context
    }

    fn resolve_payload_drop_route_plan(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoutePlan {
        if self.trusted_hovered_window_is_known_but_unusable(target_context) {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
            );
        }
        match request.coordinate_space() {
            DockViewportPointerCoordinateSpace::GlobalScreen => {
                if let Some(plan) =
                    self.resolve_global_screen_payload_drop_route_plan(request, target_context)
                {
                    return plan;
                }
            }
            DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal => {
                return DockViewportDropRoutePlan::route(
                    self.resolve_trusted_hovered_window_local_payload_drop_route(
                        request,
                        target_context,
                    ),
                );
            }
            DockViewportPointerCoordinateSpace::EventReceiverLocal => {
                if let Some(route) = self
                    .resolve_event_receiver_local_scene_payload_drop_route(request, target_context)
                {
                    return DockViewportDropRoutePlan::route(route);
                }
                return DockViewportDropRoutePlan::unavailable(unavailable_route_authority_reason(
                    target_context,
                ));
            }
            DockViewportPointerCoordinateSpace::SourceLocalOnly => {
                let local_route =
                    self.resolve_source_local_payload_drop_route(request, target_context);
                if !matches!(local_route, DockViewportDropRoute::Unavailable) {
                    return DockViewportDropRoutePlan::route(local_route);
                }
            }
        }

        DockViewportDropRoutePlan::OutsideRegisteredViewport
    }

    fn trusted_hovered_window_is_known_but_unusable(
        &self,
        target_context: &DockViewportTargetContext,
    ) -> bool {
        target_context
            .trusted_hovered_window()
            .is_some_and(|hovered| self.window_can_authorize_hover_hit(hovered) == Some(false))
    }

    fn resolve_global_screen_payload_drop_route_plan(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoutePlan> {
        let window_hits = self.global_screen_viewport_window_hits(request.release_position());
        let has_any_hits = !window_hits.is_empty();
        let has_blocking_window_hit = window_hits
            .iter()
            .any(DockViewportWindowHit::blocks_host_target);
        let host_hits = window_hits
            .iter()
            .filter_map(DockViewportWindowHit::target_hit)
            .collect::<Vec<_>>();
        let event_receiver_target =
            self.event_receiver_local_scene_target_from_hits(request, target_context, &host_hits);
        let resolution = resolve_authorized_viewport_route_target(window_hits, target_context);
        let Some(resolution) = resolution.or(event_receiver_target) else {
            if let Some(route) =
                self.resolve_event_receiver_global_scene_payload_drop_route(request, target_context)
            {
                return Some(DockViewportDropRoutePlan::route(route));
            }
            if has_blocking_window_hit {
                return Some(DockViewportDropRoutePlan::unavailable(
                    DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
                ));
            }
            return has_any_hits.then(|| {
                DockViewportDropRoutePlan::unavailable(unavailable_route_authority_reason(
                    target_context,
                ))
            });
        };
        Some(self.route_plan_from_authorized_viewport_target(request, resolution))
    }

    fn route_plan_from_authorized_viewport_target(
        &self,
        request: &DockViewportDropRouteRequest,
        resolution: DockAuthorizedViewportRouteTarget,
    ) -> DockViewportDropRoutePlan {
        let route_authority = resolution.authority();
        let Some(target) = resolution.into_target().into_target_hit() else {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
            );
        };
        if request.release_origin() == DockPayloadDropReleaseOrigin::SourceOnly
            && target.space() != request.source_space()
        {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::NoViewportAuthority,
            );
        }
        if target.space() == request.source_space() {
            return DockViewportDropRoutePlan::route(DockViewportDropRoute::Local {
                host_position: target.host_position(),
                window_id: target.window_id(),
                facts_generation: target.facts_generation(),
                authority: route_authority,
            });
        }
        DockViewportDropRoutePlan::route(DockViewportDropRoute::KnownViewport {
            target,
            authority: route_authority,
        })
    }

    fn event_receiver_local_scene_target_from_hits(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
        hits: &[DockViewportTargetHit],
    ) -> Option<DockAuthorizedViewportRouteTarget> {
        let authority = self.event_receiver_local_scene_authority(
            request,
            target_context,
            DockEventReceiverLocalSceneAuthorityMode::HitTestedScene,
        )?;
        let receiver_hit = hits.iter().find(|hit| {
            hit.window_id() == authority.receiver_window && hit.space() == request.source_space()
        })?;
        Some(DockAuthorizedViewportRouteTarget::event_receiver_local_scene(receiver_hit.clone()))
    }

    fn resolve_event_receiver_local_scene_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoute> {
        let authority = self.event_receiver_local_scene_authority(
            request,
            target_context,
            DockEventReceiverLocalSceneAuthorityMode::ReceiverSceneProof,
        )?;
        let host_position =
            authority.host_position_from_window_position(request.release_position())?;
        Some(authority.local_route(host_position))
    }

    fn resolve_event_receiver_global_scene_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoute> {
        let authority = self.event_receiver_local_scene_authority(
            request,
            target_context,
            DockEventReceiverLocalSceneAuthorityMode::ReceiverSceneProof,
        )?;
        let screen_bounds = authority.global_screen_bounds?;
        if !screen_bounds.contains(&request.release_position()) {
            return None;
        }
        let window_position = open_gpui::point(
            request.release_position().x - screen_bounds.origin.x,
            request.release_position().y - screen_bounds.origin.y,
        );
        let host_position = authority.host_position_from_window_position(window_position)?;
        Some(authority.local_route(host_position))
    }

    fn event_receiver_local_scene_authority(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
        mode: DockEventReceiverLocalSceneAuthorityMode,
    ) -> Option<DockEventReceiverLocalSceneAuthority> {
        let proof_required = match (mode, target_context.trusted_hovered_signal()) {
            (
                DockEventReceiverLocalSceneAuthorityMode::HitTestedScene,
                crate::DockViewportTrustedHoveredSignal::Unavailable,
            ) => false,
            (
                DockEventReceiverLocalSceneAuthorityMode::HitTestedScene,
                crate::DockViewportTrustedHoveredSignal::TrustedNone,
            )
            | (
                DockEventReceiverLocalSceneAuthorityMode::ReceiverSceneProof,
                crate::DockViewportTrustedHoveredSignal::Unavailable
                | crate::DockViewportTrustedHoveredSignal::TrustedNone,
            ) => true,
            _ => return None,
        };
        let proof = request.event_receiver_local_scene_proof();
        if proof_required && proof.is_none() {
            return None;
        }
        let receiver_window = request.event_receiver_window()?;
        let receiver_space = self.space_for_window_id(receiver_window)?;
        if receiver_space != request.source_space() {
            return None;
        }
        let snapshot = self.snapshot(request.source_space())?;
        if snapshot.window.window_id() != receiver_window {
            return None;
        }
        let facts_generation =
            self.snapshot_facts_generation(request.source_space(), receiver_window)?;
        if let Some(proof) = proof
            && (!proof.matches_viewport(request.source_space(), receiver_window)
                || proof.generation() != facts_generation)
        {
            return None;
        }
        Some(DockEventReceiverLocalSceneAuthority {
            receiver_window,
            facts_generation,
            host_bounds: snapshot.host_bounds?,
            global_screen_bounds: snapshot.global_screen_bounds(),
        })
    }

    fn resolve_trusted_hovered_window_local_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        if let Some(target) = self.trusted_hovered_window_local_drop_target(request, target_context)
        {
            if &target.target_space == request.source_space() {
                return DockViewportDropRoute::Local {
                    host_position: target.host_position,
                    window_id: target.target_window.window_id(),
                    facts_generation: target.facts_generation,
                    authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
                };
            }

            return DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target.target_space,
                    target.target_window,
                    target.host_position,
                    target.facts_generation,
                ),
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            };
        }

        DockViewportDropRoute::Unavailable
    }

    fn trusted_hovered_window_local_drop_target(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockTrustedHoveredWindowLocalDropTarget> {
        let receiver_window = request.event_receiver_window()?;
        if target_context.trusted_hovered_window() != Some(receiver_window) {
            return None;
        }
        let target_space = self.space_for_window_id(receiver_window).cloned()?;
        let target_window = self.window_for_space(&target_space)?;
        let host_position = self.window_to_host(&target_space, request.release_position())?;
        let facts_generation = self.snapshot_facts_generation(&target_space, receiver_window)?;
        Some(DockTrustedHoveredWindowLocalDropTarget {
            target_space,
            target_window,
            host_position,
            facts_generation,
        })
    }

    fn resolve_source_local_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        if let Some(hovered_window) = target_context.trusted_hovered_window()
            && self
                .window_for_space(request.source_space())
                .is_some_and(|window| window.window_id() == hovered_window)
            && let Some(host_position) =
                self.window_to_host(request.source_space(), request.release_position())
        {
            let Some(facts_generation) =
                self.snapshot_facts_generation(request.source_space(), hovered_window)
            else {
                return DockViewportDropRoute::Unavailable;
            };
            return DockViewportDropRoute::Local {
                host_position,
                window_id: hovered_window,
                facts_generation,
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            };
        }
        DockViewportDropRoute::Unavailable
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
        viewport_drop_scene::{
            DockViewportHostSceneFrame, DockViewportHostSceneRegistry,
            DockViewportHostSceneSnapshot,
        },
        viewport_registry::{DockViewportInputMask, DockViewportWindowBoundsFrame},
        viewport_test_support::{bounds, handle, item, register_viewport, space},
    };
    use open_gpui::{DisplayId, WindowBounds, WindowId, point, px};
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
    fn hovered_host_global_drop_requires_explicit_route_authority() {
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
            "a lone geometry hit is diagnostic-only without backend hovered-window or stack authority"
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
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
            "platform window authority must stop at the front viewport window instead of tunneling to an underlay host hit"
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
            "source-only release should not infer authority from a lone geometry hit"
        );
    }

    #[test]
    fn drop_route_authorizes_window_stack_fallback_when_hovered_backend_is_unavailable() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "front-to-back window stack fallback authorizes a route when the hovered-window backend is unavailable"
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
            "event receiver remains diagnostic-only; a lone geometry hit cannot authorize a viewport route"
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
            "source-only global releases need an accepted preview; backend fallback must not authorize cross-viewport delivery"
        );
    }

    #[test]
    fn source_only_global_drop_rejects_window_stack_authority_for_cross_viewport_route() {
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
            "source-only releases must replay an accepted routed preview; window stack fallback is preview authority only"
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
            "trusted hovered=None is explicit backend authority and must not be replaced by the event receiver"
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
            "trusted hovered=None is explicit backend authority; floating payloads must rely on no-input/fallback or accepted preview routing instead of event-receiver guesses"
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
                authority: DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene,
            },
            "explicit event-receiver scene proof may produce a same-window candidate; workspace delivery still requires the accepted local target snapshot"
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
                authority: DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene,
            },
            "local-coordinate backends may use explicit event-receiver scene proof for same-window drops"
        );

        let request_without_authority = DockViewportDropRouteRequest::from_platform_signals(
            source,
            DockNodeId::null(),
            DockViewportDropPayload::Item(item("a")),
            point(px(30.0), px(50.0)),
            None,
            signals,
        );
        assert_eq!(
            adapter.resolve_payload_drop_route(&request_without_authority, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "event-receiver local coordinates without scene proof must not become a route"
        );
    }

    #[test]
    fn event_receiver_local_scene_proof_rejects_stale_generation() {
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
            0,
        )));

        assert_eq!(
            adapter.resolve_payload_drop_route(&request, &DockPolicy::default()),
            DockViewportDropRoute::Unavailable,
            "event-receiver proof must be tied to the current rendered scene generation"
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
            "event-receiver scene proof belongs to hovered-host render paths; source-only capture replay should continue through tear-off policy"
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
                authority: DockViewportAuthorizedRouteAuthority::EventReceiverLocalScene,
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
            "overlapping route commits must not be chosen by stable fallback ordering alone"
        );
    }

    #[test]
    fn hovered_host_overlap_route_authorizes_window_stack_fallback_when_backend_is_unavailable() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "hovered-host global releases may use front-to-back window stack fallback when the backend lacks hovered-window authority"
        );
    }

    #[test]
    fn no_input_hovered_viewport_uses_window_stack_fallback_authority() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "when backend hover reports a no-input viewport, ImGui-style stack fallback authorizes the underlay commit"
        );
    }

    #[test]
    fn no_input_hovered_viewport_falls_back_to_stack_authority() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "a backend that still reports a no-input viewport as hovered should be treated as a fallback case, not as an authoritative hovered target"
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            "a trusted hovered-window signal for the route-ready underlay keeps hovered authority"
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            "a trusted hovered window stays authoritative even when no-input fallback is disabled"
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "the ImGui-style fallback search skips non-routable viewports and authorizes the first route-ready underlay"
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
    fn no_input_hovered_stack_fallback_uses_registry_facts_to_authorize_underlay() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "no-input hovered authority falls back to the route-ready source viewport when that is the underlay"
        );
        assert_eq!(
            DockDropDelivery::from_route_request_with_resolved_target(
                &DockViewportDropRouteRequest::from_target_context(
                    source,
                    DockNodeId::null(),
                    DockViewportDropPayload::Item(item("a")),
                    point(px(120.0), px(140.0)),
                    None,
                    DockViewportTargetContext::new(),
                ),
                route,
                None,
            ),
            None,
            "workspace delivery still requires the resolved drop target snapshot"
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
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
    fn drop_route_without_global_window_bounds_rejects_hovered_source_without_local_position_authority()
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            }
        );
    }

    #[test]
    fn platform_matrix_global_hovered_backend_authorizes_cross_viewport_commit() {
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            "global-bounds backends with trusted hovered-window authority may commit cross-viewport"
        );
    }

    #[test]
    fn platform_matrix_global_stack_without_hovered_backend_authorizes_window_stack_fallback() {
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
                authority: DockViewportAuthorizedRouteAuthority::FrontToBackWindowStackFallback,
            },
            "global-bounds backends may use window-stack fallback when hovered-window authority is unavailable"
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            "Wayland-style local coordinates may commit only when hovered window also received the event"
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
            "receiver-local coordinates do not authorize a route without hovered-window authority"
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
            "receiver-local coordinates do not authorize a cross-viewport route without hovered-window authority"
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
            "with global bounds, reliable hovered=None means no app viewport is hovered and the event receiver cannot become hovered-window authority"
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
            "hovered=None blocks receiver-local coordinates from becoming route authority"
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
                window_id: handle(1).window_id(),
                facts_generation: 1,
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
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
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn known_viewport_drop_delivery_requires_accepted_routed_preview() {
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

        let target_hit = DockViewportTargetHit::with_facts_generation(
            target,
            target_window,
            known_position,
            target_facts_generation,
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        );
        assert_eq!(
            delivery, None,
            "fresh known viewport route should only create preview state, not delivery"
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit,
                authority: DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview,
            },
            Some(resolved_target.clone()),
        )
        .expect("accepted routed preview should derive a delivery");
        let DockDropDeliveryKind::Workspace(known) = delivery.kind() else {
            panic!("accepted known viewport route should derive a workspace commit");
        };
        assert_eq!(delivery.drag_session_id(), Some(drag_session.id()));
        assert_eq!(delivery.source_space(), &source);
        assert_eq!(delivery.source_node(), source_tabs);
        assert_eq!(delivery.payload(), &DockViewportDropPayload::Item(item));
        assert_eq!(known, &resolved_target);
    }

    #[test]
    fn source_only_cross_viewport_delivery_requires_accepted_routed_preview_authority() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let drag_payload =
            DockDragPayload::new_item(source.clone(), source_tabs, item.clone(), "A".to_string());
        let drag_session = DockRuntimeDragSession::new(13, &drag_payload);
        let request = DockViewportDropRouteRequest::from_platform_signals_with_origin(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item),
            point(px(900.0), px(900.0)),
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
            DockPayloadDropReleaseOrigin::SourceOnly,
        )
        .with_drag_session(Some(drag_session));
        let target = space("target");
        let target_window = handle(9);
        let known_position = point(px(12.0), px(34.0));
        let target_facts_generation = 41;
        let resolved_target = resolved_drop_target_snapshot(
            target.clone(),
            target_window.window_id(),
            target_facts_generation,
        );
        let target_hit = DockViewportTargetHit::with_facts_generation(
            target,
            target_window,
            known_position,
            target_facts_generation,
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                authority: DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        );
        assert_eq!(
            delivery, None,
            "source-only cross-viewport delivery cannot be minted from fresh hover authority"
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit,
                authority: DockViewportAuthorizedRouteAuthority::AcceptedRoutedPreview,
            },
            Some(resolved_target.clone()),
        )
        .expect("accepted routed preview may replay source-only cross-viewport delivery");
        let DockDropDeliveryKind::Workspace(known) = delivery.kind() else {
            panic!("accepted source-only replay should derive a workspace commit");
        };
        assert_eq!(known, &resolved_target);
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
                    Some(release_position),
                    Some(suggested_window_bounds),
                )
                .with_drag_session(Some(drag_session))
                .with_tear_off_geometry(Some(tear_off_geometry))
            )
        );
    }

    #[test]
    fn drop_delivery_preserves_global_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let release_position = point(px(430.0), px(350.0));
        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            source_tabs,
            DockViewportDropPayload::Item(item("a")),
            release_position,
            None,
            DockViewportPlatformSignals::from_target_context(DockViewportTargetContext::new()),
        );

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(tear_off.release_position(), Some(release_position));
    }

    #[test]
    fn drop_delivery_omits_local_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let request = DockViewportDropRouteRequest::from_host_release(
            source.clone(),
            source_tabs,
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

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::EventReceiverLocal
        );
        assert_eq!(tear_off.release_position(), None);
    }

    #[test]
    fn drop_delivery_omits_source_local_release_point_for_tear_off_request() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let request = DockViewportDropRouteRequest::from_host_release(
            source.clone(),
            source_tabs,
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

        let tear_off =
            DockDropDelivery::from_route_request(&request, DockViewportDropRoute::TearOff)
                .expect("tear-off route should derive a delivery")
                .tear_off_request()
                .expect("delivery should contain a tear-off request");

        assert_eq!(request.release_position(), point(px(30.0), px(50.0)));
        assert_eq!(
            request.coordinate_space(),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
        );
        assert_eq!(tear_off.release_position(), None);
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
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(0.0, 0.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(0.0), px(0.0)),
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        DockViewportResolvedDropTargetSnapshot::new(
            target_space.clone(),
            Some(target_window_id),
            frame,
            Some(facts_generation),
            point(px(0.0), px(0.0)),
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::EmptyDockSpace {
                    space: target_space,
                },
                source: DockDropResolveSource::EmptyDockSpace,
                drop_box: None,
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: false,
            },
        )
    }
}
