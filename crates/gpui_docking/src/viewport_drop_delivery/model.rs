#[cfg(test)]
use crate::DockWorkspace;
use crate::{
    DockActionApplyError, DockNodeId, DockPolicyError, DockSpaceId, DockViewportAdapter,
    DockViewportDropPayload, DockViewportDropRoute, DockViewportDropRouteRejectionReason,
    DockViewportDropRouteRequest, DockViewportPointerCoordinateSpace, DockViewportRouteProof,
    DockViewportTearOffRequest,
    drop_target::{DockDropTargetKey, DockResolvedDropTarget},
    interaction::DockRuntimeDragSession,
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistry},
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
};
use open_gpui::{Pixels, Point, Size, WindowId};

/// Current workspace target facts for a viewport route.
pub(crate) enum DockViewportWorkspaceRouteTarget {
    Resolved(DockViewportResolvedDropTargetSnapshot),
    PreviewOnly(DockViewportResolvedDropTargetSnapshot),
    /// The viewport route still points at current window facts, but the current host scene has no
    /// workspace drop target at the routed position.
    NoCurrentHostTarget,
    /// The viewport route no longer matches current window or host-scene facts.
    RouteUnavailable,
    Rejected {
        target: DockViewportResolvedDropTargetSnapshot,
        reason: DockPolicyError,
    },
    NotWorkspaceRoute,
}

/// Route and delivery facts resolved from the same release snapshot.
#[derive(Debug, Clone)]
pub(crate) struct DockViewportResolvedDropRoute {
    route: DockViewportDropRoute,
    delivery: Option<DockDropDelivery>,
    preview_target: Option<DockViewportResolvedDropTargetSnapshot>,
    drag_session: Option<DockRuntimeDragSession>,
}

impl DockViewportResolvedDropRoute {
    #[cfg(test)]
    pub(crate) fn new(route: DockViewportDropRoute, delivery: Option<DockDropDelivery>) -> Self {
        let drag_session = delivery
            .as_ref()
            .and_then(DockDropDelivery::drag_session)
            .cloned();
        Self {
            route,
            delivery,
            preview_target: None,
            drag_session,
        }
    }

    pub(crate) fn from_workspace_route_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace_target: DockViewportWorkspaceRouteTarget,
    ) -> Self {
        match workspace_target {
            DockViewportWorkspaceRouteTarget::Resolved(target) => {
                Self::with_preview_target(request, route, Some(target.clone()), Some(target))
            }
            DockViewportWorkspaceRouteTarget::PreviewOnly(target) => {
                Self::with_preview_target(request, route, None, Some(target))
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget
            | DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => {
                Self::with_preview_target(request, route, None, None)
            }
            DockViewportWorkspaceRouteTarget::RouteUnavailable => {
                Self::with_preview_target(request, DockViewportDropRoute::Unavailable, None, None)
            }
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                Self::with_preview_target(
                    request,
                    DockViewportDropRoute::Rejected(reason.into()),
                    None,
                    Some(target),
                )
            }
        }
    }

    pub(crate) fn foreign_surface_rejection(request: &DockViewportDropRouteRequest) -> Self {
        Self {
            route: DockViewportDropRoute::Rejected(
                DockViewportDropRouteRejectionReason::ForeignSurface,
            ),
            delivery: None,
            preview_target: None,
            drag_session: request.drag_session().cloned(),
        }
    }

    fn with_preview_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        delivery_target: Option<DockViewportResolvedDropTargetSnapshot>,
        preview_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Self {
        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            request,
            route.clone(),
            delivery_target,
        );
        Self {
            route,
            delivery,
            preview_target,
            drag_session: request.drag_session().cloned(),
        }
    }

    pub(crate) fn route(&self) -> &DockViewportDropRoute {
        &self.route
    }

    pub(crate) fn delivery(&self) -> Option<&DockDropDelivery> {
        self.delivery.as_ref()
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.drag_session.as_ref()
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
                .and_then(DockDropDelivery::workspace_target)
        })
    }

    pub(crate) fn into_delivery(self) -> Result<DockDropDelivery, DockActionApplyError> {
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
            && self.drag_session == other.drag_session
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

impl DockDropDeliveryKind {
    fn from_route_request(
        request: &DockViewportDropRouteRequest,
        route: &DockViewportDropRoute,
        resolved_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Option<Self> {
        match route {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                Some(Self::Workspace(resolved_target?))
            }
            DockViewportDropRoute::TearOff => Some(Self::TearOff(
                tear_off_request_from_drop_route_request(request),
            )),
            DockViewportDropRoute::Unavailable | DockViewportDropRoute::Rejected(_) => None,
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

/// Workspace commit facts prepared from a resolved drop delivery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropWorkspaceCommit {
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_node: DockNodeId,
    pub(crate) payload: DockViewportDropPayload,
    pub(crate) target: DockWorkspaceResolvedDropTarget,
    pub(crate) drag_session: Option<DockRuntimeDragSession>,
}

/// Resolved target snapshot captured from a concrete host-scene frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportResolvedDropTargetSnapshot {
    route_proof: DockViewportRouteProof,
    requires_current_route_facts: bool,
    frame: DockViewportHostSceneFrame,
    drop_guide_metrics: crate::DockDropGuideMetrics,
    host_position: Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    target_key: DockDropTargetKey,
    target: DockResolvedDropTarget,
    preview_only: bool,
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
}

impl DockViewportResolvedDropTargetSnapshot {
    pub(crate) fn new(
        frame: DockViewportHostSceneFrame,
        drop_guide_metrics: crate::DockDropGuideMetrics,
        facts_generation: u64,
        requires_current_route_facts: bool,
        host_position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        target: DockResolvedDropTarget,
    ) -> Self {
        let target_key = target.target_key();
        let route_proof =
            DockViewportRouteProof::new(frame.registration_key().clone(), facts_generation);
        Self {
            route_proof,
            requires_current_route_facts,
            frame,
            drop_guide_metrics,
            host_position,
            payload_size,
            target_key,
            target,
            preview_only: false,
        }
    }

    pub(crate) fn new_preview_only(
        frame: DockViewportHostSceneFrame,
        drop_guide_metrics: crate::DockDropGuideMetrics,
        facts_generation: u64,
        requires_current_route_facts: bool,
        host_position: Point<Pixels>,
        payload_size: Option<Size<Pixels>>,
        target: DockResolvedDropTarget,
    ) -> Self {
        let mut snapshot = Self::new(
            frame,
            drop_guide_metrics,
            facts_generation,
            requires_current_route_facts,
            host_position,
            payload_size,
            target,
        );
        snapshot.preview_only = true;
        snapshot
    }

    pub(crate) fn facts_generation(&self) -> Option<u64> {
        self.requires_current_route_facts
            .then_some(self.route_proof.facts_generation())
    }

    pub(crate) fn route_proof(&self) -> &DockViewportRouteProof {
        &self.route_proof
    }

    pub(crate) fn frame(&self) -> &DockViewportHostSceneFrame {
        &self.frame
    }

    pub(crate) fn drop_guide_metrics(&self) -> crate::DockDropGuideMetrics {
        self.drop_guide_metrics
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
        self.route_proof.space()
    }

    pub(crate) fn target_window_id(&self) -> Option<WindowId> {
        Some(self.route_proof.window_id())
    }

    pub(crate) fn into_target(self) -> DockResolvedDropTarget {
        self.target
    }

    pub(crate) fn target(&self) -> &DockResolvedDropTarget {
        &self.target
    }

    pub(crate) fn is_preview_only(&self) -> bool {
        self.preview_only
    }
}

impl DockDropDelivery {
    #[cfg(test)]
    pub(super) fn from_route_request(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
    ) -> Option<Self> {
        Self::from_route_request_with_resolved_target(request, route, None)
    }

    pub(super) fn from_route_request_with_resolved_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        resolved_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Option<Self> {
        let source = DockDropDeliverySource::from_request(request);
        let kind = DockDropDeliveryKind::from_route_request(request, &route, resolved_target)?;
        Some(Self { source, kind })
    }

    pub(crate) fn drag_session_id(&self) -> Option<u64> {
        self.source.drag_session().map(DockRuntimeDragSession::id)
    }

    pub(crate) fn drag_session(&self) -> Option<&DockRuntimeDragSession> {
        self.source.drag_session()
    }

    pub(crate) fn workspace_target(&self) -> Option<&DockViewportResolvedDropTargetSnapshot> {
        match &self.kind {
            DockDropDeliveryKind::Workspace(target) => Some(target),
            DockDropDeliveryKind::TearOff(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn source_space(&self) -> &DockSpaceId {
        self.source.source_space()
    }

    pub(crate) fn source_node(&self) -> DockNodeId {
        self.source.source_node
    }

    pub(crate) fn payload(&self) -> &DockViewportDropPayload {
        &self.source.payload
    }

    #[cfg(test)]
    pub(crate) fn kind(&self) -> &DockDropDeliveryKind {
        &self.kind
    }

    #[cfg(test)]
    pub(crate) fn validate_current_workspace_target(
        &self,
        adapter: &DockViewportAdapter,
        host_scenes: &DockViewportHostSceneRegistry,
        workspace: &DockWorkspace,
    ) -> Result<(), DockActionApplyError> {
        match &self.kind {
            DockDropDeliveryKind::Workspace(target) => {
                let facts = crate::DockViewportWorkspaceRouteFacts::capture_for_payload(
                    workspace,
                    self.source.payload(),
                    self.source.source_node(),
                );
                super::validate_delivery_workspace_target_inner(
                    adapter,
                    host_scenes,
                    self.source.source_node(),
                    self.source.payload(),
                    target.clone(),
                    &facts,
                )
                .map(|_| ())
            }
            DockDropDeliveryKind::TearOff(_) => Ok(()),
        }
    }

    pub(crate) fn into_workspace_commit(
        self,
        adapter: &DockViewportAdapter,
        host_scenes: &DockViewportHostSceneRegistry,
        facts: &crate::DockViewportWorkspaceRouteFacts,
    ) -> Result<DockDropWorkspaceCommit, DockActionApplyError> {
        let Self { source, kind } = self;
        match kind {
            DockDropDeliveryKind::Workspace(target) => {
                let DockDropDeliverySource {
                    source_space,
                    source_node,
                    payload,
                    drag_session,
                } = source;
                let target = super::resolve_delivery_workspace_target_with_facts(
                    adapter,
                    host_scenes,
                    source_node,
                    &payload,
                    target,
                    facts,
                )?;
                Ok(DockDropWorkspaceCommit {
                    source_space,
                    source_node,
                    payload,
                    target,
                    drag_session,
                })
            }
            DockDropDeliveryKind::TearOff(_) => {
                Err(DockActionApplyError::TearOffViewportOpenFailed {
                    message:
                        "tear-off viewport commits must be opened through DockViewportRuntimeHandle"
                            .to_string(),
                })
            }
        }
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
        resolution.into_delivery()
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
