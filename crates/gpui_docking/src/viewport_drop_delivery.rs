use crate::{
    DockActionApplyError, DockEdgeDockSizing, DockNodeId, DockPolicyError, DockSpaceId,
    DockViewportAdapter, DockViewportDropPayload, DockViewportDropRoute,
    DockViewportDropRouteRequest, DockViewportPointerCoordinateSpace,
    DockViewportRouteSelectionSource, DockViewportTearOffRequest, DockWorkspace, DropZone,
    drop_target::{
        DockDropResolution, DockDropTargetKey, DockResolvedDropTarget,
        validate_resolved_drop_target,
    },
    interaction::DockRuntimeDragSession,
    viewport_drop_scene::{DockViewportHostSceneFrame, DockViewportHostSceneRegistry},
    workspace_drop_target::DockWorkspaceResolvedDropTarget,
    workspace_move_validation::{DockPayloadDockClasses, dock_target_validator},
};
use open_gpui::{Pixels, Point, Size, WindowId};

/// Current workspace target facts for a viewport route.
pub(crate) enum DockViewportWorkspaceRouteTarget {
    Resolved(DockViewportResolvedDropTargetSnapshot),
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

/// Permit that can upgrade a routed drop snapshot into an actual delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockViewportDeliveryPermit {
    /// Mirrors ImGui's AcceptBeforeDelivery path: a target rendered and accepted the preview.
    AcceptedRoutedPreview,
    /// The release is outside registered host viewports and may open a new platform viewport.
    TearOff,
}

impl DockViewportDeliveryPermit {
    fn from_route(route: &DockViewportDropRoute) -> Option<Self> {
        match route {
            DockViewportDropRoute::Local { source, .. }
            | DockViewportDropRoute::KnownViewport { source, .. }
                if *source == DockViewportRouteSelectionSource::AcceptedRoutedPreview =>
            {
                Some(Self::AcceptedRoutedPreview)
            }
            DockViewportDropRoute::TearOff => Some(Self::TearOff),
            DockViewportDropRoute::Local { .. }
            | DockViewportDropRoute::KnownViewport { .. }
            | DockViewportDropRoute::Unavailable
            | DockViewportDropRoute::Rejected(_) => None,
        }
    }
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

    pub(crate) fn from_workspace_route_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace_target: DockViewportWorkspaceRouteTarget,
    ) -> Self {
        match workspace_target {
            DockViewportWorkspaceRouteTarget::Resolved(target) => {
                Self::with_preview_target(request, route, Some(target.clone()), Some(target))
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
                    DockViewportDropRoute::Rejected(reason),
                    None,
                    Some(target),
                )
            }
        }
    }

    pub(crate) fn from_accepted_workspace_route_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace_target: DockViewportWorkspaceRouteTarget,
        accepted_target_key: &DockDropTargetKey,
    ) -> Option<Self> {
        match workspace_target {
            DockViewportWorkspaceRouteTarget::Resolved(target) => {
                if target.target_key() != accepted_target_key {
                    return None;
                }
                Some(Self::with_preview_target(
                    request,
                    route,
                    Some(target.clone()),
                    Some(target),
                ))
            }
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                if target.target_key() != accepted_target_key {
                    return None;
                }
                Some(Self::with_preview_target(
                    request,
                    DockViewportDropRoute::Rejected(reason),
                    Some(target.clone()),
                    Some(target),
                ))
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget
            | DockViewportWorkspaceRouteTarget::RouteUnavailable
            | DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => None,
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
                .and_then(DockDropDelivery::workspace_target)
        })
    }

    pub(crate) fn into_delivery(self) -> Result<DockDropDelivery, DockActionApplyError> {
        if DockViewportDeliveryPermit::from_route(&self.route).is_none() {
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
    fn from_delivery_permit(
        permit: DockViewportDeliveryPermit,
        request: &DockViewportDropRouteRequest,
        resolved_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Option<Self> {
        match permit {
            DockViewportDeliveryPermit::AcceptedRoutedPreview => {
                Some(Self::Workspace(resolved_target?))
            }
            DockViewportDeliveryPermit::TearOff => Some(Self::TearOff(
                tear_off_request_from_drop_route_request(request),
            )),
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
    fn from_route_request(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
    ) -> Option<Self> {
        Self::from_route_request_with_resolved_target(request, route, None)
    }

    fn from_route_request_with_resolved_target(
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        resolved_target: Option<DockViewportResolvedDropTargetSnapshot>,
    ) -> Option<Self> {
        let delivery_permit = DockViewportDeliveryPermit::from_route(&route)?;
        let source = DockDropDeliverySource::from_request(request);
        let kind =
            DockDropDeliveryKind::from_delivery_permit(delivery_permit, request, resolved_target)?;
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

    #[cfg(test)]
    pub(crate) fn validate_current_workspace_target(
        &self,
        adapter: &DockViewportAdapter,
        host_scenes: &DockViewportHostSceneRegistry,
        workspace: &DockWorkspace,
    ) -> Result<(), DockActionApplyError> {
        match &self.kind {
            DockDropDeliveryKind::Workspace(target) => validate_delivery_workspace_target_inner(
                adapter,
                host_scenes,
                workspace,
                self.source.source_node(),
                self.source.payload(),
                target.clone(),
            )
            .map(|_| ()),
            DockDropDeliveryKind::TearOff(_) => Ok(()),
        }
    }

    pub(crate) fn into_workspace_commit(
        self,
        adapter: &DockViewportAdapter,
        host_scenes: &DockViewportHostSceneRegistry,
        workspace: &DockWorkspace,
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
                let target = resolve_delivery_workspace_target(
                    adapter,
                    host_scenes,
                    workspace,
                    source_node,
                    &payload,
                    target,
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

#[derive(Clone, Copy)]
enum DockMissingHostTargetBehavior {
    PreserveRoute,
    MarkRouteUnavailable,
}

impl DockMissingHostTargetBehavior {
    fn into_route_target(self) -> DockViewportWorkspaceRouteTarget {
        match self {
            Self::PreserveRoute => DockViewportWorkspaceRouteTarget::NoCurrentHostTarget,
            Self::MarkRouteUnavailable => DockViewportWorkspaceRouteTarget::RouteUnavailable,
        }
    }
}

#[derive(Clone, Copy)]
enum DockViewportRouteFactsSource {
    CurrentRouteFacts,
    EventReceiverLocalScene,
}

impl DockViewportRouteFactsSource {
    fn for_local_route_selection_source(source: DockViewportRouteSelectionSource) -> Self {
        match source {
            DockViewportRouteSelectionSource::EventReceiverLocalScene => {
                Self::EventReceiverLocalScene
            }
            DockViewportRouteSelectionSource::TrustedHoveredWindow
            | DockViewportRouteSelectionSource::FrontToBackWindowStackFallback
            | DockViewportRouteSelectionSource::FocusStampWindowStackFallback
            | DockViewportRouteSelectionSource::DragLastHoveredViewportFallback
            | DockViewportRouteSelectionSource::AcceptedRoutedPreview => Self::CurrentRouteFacts,
        }
    }

    fn requires_current_route_facts(self) -> bool {
        matches!(self, Self::CurrentRouteFacts)
    }

    fn facts_generation_for_snapshot(self, facts_generation: u64) -> Option<u64> {
        self.requires_current_route_facts()
            .then_some(facts_generation)
    }
}

#[derive(Clone, Copy)]
struct DockExistingViewportRouteTarget<'a> {
    space: &'a DockSpaceId,
    window_id: WindowId,
    host_position: Point<Pixels>,
    facts_generation: u64,
    missing_host_target: DockMissingHostTargetBehavior,
    route_facts_source: DockViewportRouteFactsSource,
}

/// Resolves the workspace target selected by a viewport route.
pub(crate) fn resolve_workspace_target_for_route(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    route: &DockViewportDropRoute,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
) -> DockViewportWorkspaceRouteTarget {
    match route {
        DockViewportDropRoute::Local {
            host_position,
            window_id,
            facts_generation,
            source,
            ..
        } => resolve_existing_viewport_workspace_target(
            adapter,
            host_scenes,
            request,
            workspace,
            payload_classes,
            DockExistingViewportRouteTarget {
                space: request.source_space(),
                window_id: *window_id,
                host_position: *host_position,
                facts_generation: *facts_generation,
                missing_host_target: DockMissingHostTargetBehavior::PreserveRoute,
                route_facts_source: DockViewportRouteFactsSource::for_local_route_selection_source(
                    *source,
                ),
            },
        ),
        DockViewportDropRoute::KnownViewport { target, .. } => {
            resolve_existing_viewport_workspace_target(
                adapter,
                host_scenes,
                request,
                workspace,
                payload_classes,
                DockExistingViewportRouteTarget {
                    space: target.space(),
                    window_id: target.window_id(),
                    host_position: target.host_position(),
                    facts_generation: target.facts_generation(),
                    missing_host_target: DockMissingHostTargetBehavior::MarkRouteUnavailable,
                    route_facts_source: DockViewportRouteFactsSource::CurrentRouteFacts,
                },
            )
        }
        DockViewportDropRoute::TearOff
        | DockViewportDropRoute::Unavailable
        | DockViewportDropRoute::Rejected(_) => DockViewportWorkspaceRouteTarget::NotWorkspaceRoute,
    }
}

fn resolve_existing_viewport_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    request: &DockViewportDropRouteRequest,
    workspace: &DockWorkspace,
    payload_classes: &DockPayloadDockClasses,
    target: DockExistingViewportRouteTarget<'_>,
) -> DockViewportWorkspaceRouteTarget {
    if target.route_facts_source.requires_current_route_facts()
        && !current_route_window_facts_match(
            adapter,
            target.space,
            target.window_id,
            target.facts_generation,
        )
    {
        return DockViewportWorkspaceRouteTarget::RouteUnavailable;
    }

    let policy = workspace.policy();
    let target_validator = dock_target_validator(target.space, payload_classes, policy);
    let graph = workspace.graph().clone();
    let target_space = target.space.clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
        };
    let payload_size = request_payload_size(request);
    let excluded_nodes = request
        .payload()
        .excluded_nodes_for_drop_scene(workspace.graph(), request.source_node());
    let Some((frame, resolution)) = host_scenes.resolve_frame_for_window(
        target.space,
        Some(target.window_id),
        target.host_position,
        payload_size,
        excluded_nodes,
        policy,
        Some(&target_validator),
        Some(&edge_plan_resolver),
    ) else {
        return target.missing_host_target.into_route_target();
    };

    match resolved_target_snapshot(
        target.space.clone(),
        Some(target.window_id),
        frame,
        target
            .route_facts_source
            .facts_generation_for_snapshot(target.facts_generation),
        target.host_position,
        payload_size,
        resolution,
    ) {
        DockResolvedViewportTarget::Valid(target) => {
            DockViewportWorkspaceRouteTarget::Resolved(target)
        }
        DockResolvedViewportTarget::Rejected { target, reason } => {
            DockViewportWorkspaceRouteTarget::Rejected { target, reason }
        }
    }
}

pub(crate) fn request_payload_size(request: &DockViewportDropRouteRequest) -> Option<Size<Pixels>> {
    let geometry = request.tear_off_geometry()?;
    geometry
        .preferred_size()
        .or_else(|| Some(geometry.source_bounds().size))
}

fn current_route_window_facts(
    adapter: &DockViewportAdapter,
    space: &DockSpaceId,
) -> Option<(WindowId, u64)> {
    let window_id = adapter.window_for_space(space)?.window_id();
    let facts_generation = adapter.snapshot_facts_generation(space, window_id)?;
    Some((window_id, facts_generation))
}

fn current_route_window_facts_match(
    adapter: &DockViewportAdapter,
    space: &DockSpaceId,
    window_id: WindowId,
    facts_generation: u64,
) -> bool {
    current_route_window_facts(adapter, space) == Some((window_id, facts_generation))
}

/// Resolves a delivery target against current viewport and workspace facts.
pub(crate) fn resolve_delivery_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        workspace,
        source_node,
        payload,
        target,
    )
}

/// Verifies that a resolved delivery still points at current route facts and policy.
#[cfg(test)]
pub(crate) fn validate_delivery_workspace_target(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> Result<(), DockActionApplyError> {
    validate_delivery_workspace_target_inner(
        adapter,
        host_scenes,
        workspace,
        source_node,
        payload,
        target.clone(),
    )
    .map(|_| ())
}

fn validate_delivery_workspace_target_inner(
    adapter: &DockViewportAdapter,
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: DockViewportResolvedDropTargetSnapshot,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let facts_current = target_facts_generation_is_current(adapter, &target);
    if !facts_current {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    if !current_resolved_target_key_matches_snapshot(
        host_scenes,
        workspace,
        source_node,
        payload,
        &target,
    ) {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    let target_space = target.target_space().clone();
    validate_resolved_target_snapshot(
        workspace,
        &target_space,
        target.into_target(),
        payload,
        source_node,
    )
}

fn current_resolved_target_key_matches_snapshot(
    host_scenes: &DockViewportHostSceneRegistry,
    workspace: &DockWorkspace,
    source_node: crate::DockNodeId,
    payload: &DockViewportDropPayload,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> bool {
    let policy = workspace.policy();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target.target_space(), &payload_classes, policy);
    let graph = workspace.graph().clone();
    let target_space = target.target_space().clone();
    let edge_plan_resolver =
        move |target_node: crate::DockNodeId, zone: DropZone, sizing: DockEdgeDockSizing| {
            graph.edge_dock_plan_with_sizing(&target_space, target_node, zone, sizing)
        };
    let excluded_nodes = payload.excluded_nodes_for_drop_scene(workspace.graph(), source_node);
    let Some((current_frame, resolution)) = host_scenes.resolve_frame_for_window(
        target.target_space(),
        target.target_window_id(),
        target.host_position(),
        target.payload_size(),
        excluded_nodes,
        policy,
        Some(&target_validator),
        Some(&edge_plan_resolver),
    ) else {
        return false;
    };
    if &current_frame != target.frame() {
        return false;
    }
    match resolution {
        DockDropResolution::Valid(current) => current.target_key() == *target.target_key(),
        DockDropResolution::Rejected(rejection) => {
            rejection.target.target_key() == *target.target_key()
        }
    }
}

fn resolved_target_snapshot(
    target_space: DockSpaceId,
    target_window_id: Option<WindowId>,
    frame: DockViewportHostSceneFrame,
    facts_generation: Option<u64>,
    host_position: open_gpui::Point<Pixels>,
    payload_size: Option<Size<Pixels>>,
    resolution: DockDropResolution,
) -> DockResolvedViewportTarget {
    match resolution {
        DockDropResolution::Valid(target) => {
            DockResolvedViewportTarget::Valid(DockViewportResolvedDropTargetSnapshot::new(
                target_space,
                target_window_id,
                frame,
                facts_generation,
                host_position,
                payload_size,
                target,
            ))
        }
        DockDropResolution::Rejected(rejection) => DockResolvedViewportTarget::Rejected {
            target: DockViewportResolvedDropTargetSnapshot::new(
                target_space,
                target_window_id,
                frame,
                facts_generation,
                host_position,
                payload_size,
                rejection.target,
            ),
            reason: rejection.reason,
        },
    }
}

enum DockResolvedViewportTarget {
    Valid(DockViewportResolvedDropTargetSnapshot),
    Rejected {
        target: DockViewportResolvedDropTargetSnapshot,
        reason: DockPolicyError,
    },
}

fn validate_resolved_target_snapshot(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    target: DockResolvedDropTarget,
    payload: &DockViewportDropPayload,
    source_node: crate::DockNodeId,
) -> Result<DockWorkspaceResolvedDropTarget, DockActionApplyError> {
    let policy = workspace.policy().clone();
    let payload_classes = workspace.payload_dock_classes_for_viewport_payload(payload, source_node);
    let target_validator = dock_target_validator(target_space, &payload_classes, &policy);
    match validate_resolved_drop_target(target, &policy, Some(&target_validator)) {
        DockDropResolution::Valid(target) => Ok(DockWorkspaceResolvedDropTarget::new(
            target_space.clone(),
            target,
        )),
        DockDropResolution::Rejected(rejection) => {
            Err(DockActionApplyError::Policy(rejection.reason))
        }
    }
}

fn target_facts_generation_is_current(
    adapter: &DockViewportAdapter,
    target: &DockViewportResolvedDropTargetSnapshot,
) -> bool {
    let (Some(window_id), Some(facts_generation)) =
        (target.target_window_id(), target.facts_generation())
    else {
        return true;
    };
    adapter.snapshot_facts_generation(target.target_space(), window_id) == Some(facts_generation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockClassId, DockGraph, DockItemId, DockNode, DockNodeId, DockPanel,
        DockViewportDropReleasePoint, DockViewportPlatformSignals, DockViewportTargetContext,
        DockViewportTargetHit, DockViewportWindowFacts,
        drag::{DockDragPayload, DockDragTearOffGeometry},
        drop_runtime::DockHostDropSceneFact,
        drop_target::{
            DockDropResolveSource, DockEmptySpaceDropTarget, DockLeafDropTarget,
            DockResolvedDropTarget, DockResolvedDropTargetKind,
        },
        geometry::{self, DockDropBoxKind, DockDropBoxSet},
        host_test_support::center_drop_position,
        interaction::DockPayloadDropReleaseOrigin,
        viewport_drop_scene::DockViewportHostSceneSnapshot,
        viewport_registry::DockViewportWindowBoundsFrame,
        viewport_test_support::{bounds, handle, item, register_viewport, space},
    };
    use open_gpui::{Bounds, WindowBounds, WindowId, point, px, size};
    use slotmap::Key;

    #[test]
    fn delivery_permit_is_separate_from_route_selection_source() {
        let source_window = handle(1);
        let target = space("target");
        let target_window = handle(2);
        let host_position = point(px(12.0), px(34.0));
        let target_hit = crate::DockViewportTargetHit::with_facts_generation(
            target,
            target_window,
            host_position,
            7,
        );

        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::Local {
                host_position,
                window_id: source_window.window_id(),
                facts_generation: 7,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            }),
            None,
            "fresh backend hover route selects a preview target but must not grant delivery"
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: crate::DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            }),
            None,
            "window-stack fallback is route selection source, not delivery permit"
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: crate::DockViewportRouteSelectionSource::FocusStampWindowStackFallback,
            }),
            None,
            "focus-stamp fallback is a route selection source, not delivery permit"
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::KnownViewport {
                target: target_hit.clone(),
                source: crate::DockViewportRouteSelectionSource::DragLastHoveredViewportFallback,
            }),
            None,
            "last-hovered viewport fallback is route selection source, not delivery permit"
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::KnownViewport {
                target: target_hit,
                source: crate::DockViewportRouteSelectionSource::AcceptedRoutedPreview,
            }),
            Some(DockViewportDeliveryPermit::AcceptedRoutedPreview)
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::TearOff),
            Some(DockViewportDeliveryPermit::TearOff)
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::Rejected(
                DockPolicyError::PlatformViewportsDisabled,
            )),
            None
        );
        assert_eq!(
            DockViewportDeliveryPermit::from_route(&DockViewportDropRoute::Unavailable),
            None
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
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
        );
        assert_eq!(delivery, None);
    }

    #[test]
    fn fallback_local_route_without_resolved_snapshot_is_absent() {
        let source = space("source");
        let source_tabs = DockNodeId::null();
        let item = item("a");
        let request = DockViewportDropRouteRequest::from_target_context(
            source,
            source_tabs,
            DockViewportDropPayload::Item(item),
            point(px(120.0), px(140.0)),
            None,
            DockViewportTargetContext::new(),
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::Local {
                host_position: point(px(20.0), px(40.0)),
                window_id: handle(1).window_id(),
                facts_generation: 1,
                source: DockViewportRouteSelectionSource::FrontToBackWindowStackFallback,
            },
            None,
        );
        assert_eq!(
            delivery, None,
            "workspace delivery still requires the resolved drop target snapshot"
        );
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
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
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
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
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
                source: DockViewportRouteSelectionSource::AcceptedRoutedPreview,
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
    fn source_only_cross_viewport_delivery_requires_accepted_routed_preview_delivery_permit() {
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
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            Some(resolved_target.clone()),
        );
        assert_eq!(
            delivery, None,
            "source-only cross-viewport delivery cannot be minted from fresh hover route selection"
        );

        let delivery = DockDropDelivery::from_route_request_with_resolved_target(
            &request,
            DockViewportDropRoute::KnownViewport {
                target: target_hit,
                source: DockViewportRouteSelectionSource::AcceptedRoutedPreview,
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
    fn local_route_requires_current_window_host_scene_identity() {
        let source_space = space("source");
        let old_window = handle(1);
        let new_window = handle(2);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), new_window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let old_frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                old_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &old_frame,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space: source_space.clone(),
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: false,
                    })
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), DockGraph::new());
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(new_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position: point(px(24.0), px(24.0)),
                window_id: old_window.window_id(),
                facts_generation: 1,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "local route must not replace its frozen source window with the current source mapping"
        );
    }

    #[test]
    fn local_route_without_current_host_target_preserves_route_state() {
        let source_space = space("source");
        let window = handle(1);
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let host_scenes = DockViewportHostSceneRegistry::default();
        let workspace = DockWorkspace::new(source_space.clone(), DockGraph::new());
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position: point(px(24.0), px(24.0)),
                window_id: window.window_id(),
                facts_generation,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(
                target,
                DockViewportWorkspaceRouteTarget::NoCurrentHostTarget
            ),
            "local route should keep its route state even when the host scene has no target"
        );
    }

    #[test]
    fn local_route_excludes_source_floating_from_cached_host_scene() {
        let source_space = space("source");
        let window = handle(4);
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(source_space.clone(), root);
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("floating")],
            selected: Some(DockItemId::from("floating")),
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_tabs,
        });

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 360.0, 240.0,
            ))),
            bounds(0.0, 0.0, 360.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let host_position = center_drop_position(bounds(0.0, 0.0, 360.0, 240.0));
        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 360.0, 240.0)),
                bounds(0.0, 0.0, 360.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root,
                    target_tabs: root,
                    bounds: bounds(0.0, 0.0, 360.0, 240.0),
                    is_central: false,
                }),
            )
            .expect("root target fact should update the current frame");
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: floating,
                    target_tabs: floating_tabs,
                    bounds: bounds(0.0, 0.0, 360.0, 240.0),
                    is_central: false,
                }),
            )
            .expect("source floating child fact should update the current frame");
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    crate::drop_scene_fact::floating_title_bar(
                        floating,
                        floating_tabs,
                        bounds(0.0, 0.0, 360.0, 240.0),
                        bounds(0.0, 0.0, 360.0, 240.0),
                    )
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Floating(floating);
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, floating);
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            floating,
            payload,
            point(px(280.0), px(220.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position,
                window_id: window.window_id(),
                facts_generation,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("local route should resolve the underlying root target");
        };
        assert_eq!(
            target.into_target().kind,
            DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: root,
            }
        );
    }

    #[test]
    fn local_route_preserves_policy_rejected_target() {
        let source_space = space("source");
        let window = handle(3);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        graph.set_root(source_space.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: tabs,
                        target_tabs: tabs,
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: true,
                    })
                )
                .is_some()
        );

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel(
            item("a"),
            DockPanel::lazy("Panel A", |_| unreachable!()).with_dock_class("editor"),
        );
        workspace
            .policy_mut()
            .set_allowed_dock_classes_for_space(source_space.clone(), ["inspector"]);

        let payload = DockViewportDropPayload::Item(DockItemId::from("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        )
        .with_tear_off_geometry(Some(
            DockDragTearOffGeometry::from_source_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
                point(px(12.0), px(12.0)),
            )
            .with_preferred_size(size(px(240.0), px(180.0))),
        ));

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position,
                window_id: window.window_id(),
                facts_generation,
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        match target {
            DockViewportWorkspaceRouteTarget::Rejected { target, reason } => {
                assert_eq!(
                    reason,
                    DockPolicyError::DockClassRejected {
                        space: source_space.clone(),
                        item: DockItemId::from("a"),
                        dock_class: Some(DockClassId::from("editor")),
                    }
                );
                assert_eq!(target.target_space(), &source_space);
                assert_eq!(target.target_window_id(), Some(window.window_id()));
            }
            DockViewportWorkspaceRouteTarget::Resolved(_) => {
                panic!("local route should not resolve when policy rejects the payload")
            }
            DockViewportWorkspaceRouteTarget::NoCurrentHostTarget => {
                panic!("local route should preserve rejected target instead of losing host target")
            }
            DockViewportWorkspaceRouteTarget::RouteUnavailable => {
                panic!("local route should not be route-unavailable when the current facts match")
            }
            DockViewportWorkspaceRouteTarget::NotWorkspaceRoute => {
                panic!("local route should not be classified as non-workspace")
            }
        }
    }

    #[test]
    fn known_viewport_route_requires_current_target_window_facts() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let old_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have route facts");
        assert!(adapter.mark_window_snapshot_stale(target_window.window_id()));

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                point(px(24.0), px(24.0)),
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: true,
                    })
                )
                .is_some()
        );

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel_descriptor(
            item("a"),
            crate::DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
        );
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    point(px(24.0), px(24.0)),
                    old_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "known viewport route must not resolve preview targets from stale window facts"
        );
    }

    #[test]
    fn known_viewport_route_without_current_host_target_is_unavailable() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have route facts");

        let host_scenes = DockViewportHostSceneRegistry::default();
        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space,
            DockNodeId::null(),
            payload,
            point(px(124.0), px(124.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    point(px(24.0), px(24.0)),
                    facts_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        assert!(
            matches!(target, DockViewportWorkspaceRouteTarget::RouteUnavailable),
            "known viewport route should become unavailable when its current host scene disappears"
        );
    }

    #[test]
    fn delivery_validation_requires_current_target_key() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(8);
        let mut graph = DockGraph::new();
        let current_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("current")],
            selected: Some(DockItemId::from("current")),
        });
        let stale_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("stale")],
            selected: Some(DockItemId::from("stale")),
        });
        graph.set_root(target_space.clone(), current_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        let frame = host_scenes
            .push_frame_fact(
                &frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: current_tabs,
                    target_tabs: current_tabs,
                    bounds: bounds(0.0, 0.0, 320.0, 240.0),
                    is_central: true,
                }),
            )
            .expect("current target fact should produce a current frame");

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("current"));
        let stale_snapshot = DockViewportResolvedDropTargetSnapshot::new(
            target_space.clone(),
            Some(target_window.window_id()),
            frame,
            Some(facts_generation),
            host_position,
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::LeafCenter {
                    root: stale_tabs,
                    target_tabs: stale_tabs,
                },
                source: DockDropResolveSource::LeafBody,
                drop_box: None,
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: true,
            },
        );

        let result = validate_delivery_workspace_target(
            &adapter,
            &host_scenes,
            &workspace,
            DockNodeId::null(),
            &payload,
            &stale_snapshot,
        );

        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
    }

    #[test]
    fn delivery_validation_requires_current_host_scene_frame() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(9);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let stale_frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        let current_frame = host_scenes
            .push_frame_fact(
                &stale_frame,
                DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                    root: target_tabs,
                    target_tabs,
                    bounds: bounds(0.0, 0.0, 320.0, 240.0),
                    is_central: true,
                }),
            )
            .expect("current target fact should produce a current frame");
        assert_ne!(stale_frame, current_frame);

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(item("target"));
        let stale_snapshot = DockViewportResolvedDropTargetSnapshot::new(
            target_space.clone(),
            Some(target_window.window_id()),
            stale_frame,
            Some(facts_generation),
            host_position,
            None,
            DockResolvedDropTarget {
                kind: DockResolvedDropTargetKind::LeafCenter {
                    root: target_tabs,
                    target_tabs,
                },
                source: DockDropResolveSource::LeafBody,
                drop_box: None,
                preview_bounds: Some(bounds(0.0, 0.0, 320.0, 240.0)),
                edge_sizing: None,
                edge_plan: None,
                is_central_region: true,
            },
        );

        let result = validate_delivery_workspace_target(
            &adapter,
            &host_scenes,
            &workspace,
            DockNodeId::null(),
            &payload,
            &stale_snapshot,
        );

        assert_eq!(result, Err(DockActionApplyError::DropTargetUnavailable));
    }

    #[test]
    fn known_viewport_route_resolves_edge_sizing_from_request_payload_geometry() {
        let source_space = space("source");
        let target_space = space("target");
        let target_window = handle(7);
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("target")],
            selected: Some(DockItemId::from("target")),
        });
        graph.set_root(target_space.clone(), target_tabs);
        let host_position =
            geometry::drop_boxes(bounds(0.0, 0.0, 1000.0, 600.0), DockDropBoxSet::Inner)
                .into_iter()
                .find(|drop_box| {
                    drop_box.kind == DockDropBoxKind::InnerEdge(crate::DropZone::Right)
                })
                .map(|drop_box| drop_box.hit_bounds.center())
                .expect("right edge drop box should exist");
        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, target_space.clone(), target_window);
        adapter.update_snapshot(
            &target_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 1000.0, 600.0,
            ))),
            bounds(0.0, 0.0, 1000.0, 600.0),
        );
        let facts_generation = adapter
            .snapshot_facts_generation(&target_space, target_window.window_id())
            .expect("target snapshot should have facts");

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                target_space.clone(),
                target_window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 1000.0, 600.0)),
                bounds(0.0, 0.0, 1000.0, 600.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: target_tabs,
                        target_tabs,
                        bounds: bounds(0.0, 0.0, 1000.0, 600.0),
                        is_central: false,
                    })
                )
                .is_some()
        );

        let workspace = DockWorkspace::new(source_space.clone(), graph);
        let payload = DockViewportDropPayload::Item(DockItemId::from("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(970.0), px(400.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(target_window),
        )
        .with_tear_off_geometry(Some(
            DockDragTearOffGeometry::from_source_bounds(
                Bounds::new(point(px(0.0), px(0.0)), size(px(240.0), px(180.0))),
                point(px(12.0), px(12.0)),
            )
            .with_preferred_size(size(px(240.0), px(180.0))),
        ));

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_facts_generation(
                    target_space,
                    target_window,
                    host_position,
                    facts_generation,
                ),
                source: crate::DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("known viewport route should resolve an edge target");
        };
        let target = target.into_target();
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::InnerEdge {
                root: target_tabs,
                target_tabs,
                zone: crate::DropZone::Right,
            }
        );
        assert_eq!(
            target.preview_bounds,
            Some(bounds(760.0, 0.0, 240.0, 600.0))
        );
        assert_eq!(
            target.edge_sizing.map(|sizing| sizing.new_child_share()),
            Some(0.24)
        );
    }

    #[test]
    fn local_event_receiver_scene_route_can_skip_current_route_facts_generation_match() {
        let source_space = space("source");
        let window = handle(7);
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(source_space.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        register_viewport(&mut adapter, source_space.clone(), window);
        adapter.update_snapshot(
            &source_space,
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 100.0, 320.0, 240.0,
            ))),
            bounds(0.0, 0.0, 320.0, 240.0),
        );
        let current_facts_generation = adapter
            .snapshot_facts_generation(&source_space, window.window_id())
            .expect("source snapshot should have route facts");
        let mismatched_facts_generation = current_facts_generation + 1;

        let mut host_scenes = DockViewportHostSceneRegistry::default();
        let host_position = center_drop_position(bounds(0.0, 0.0, 320.0, 240.0));
        let frame = host_scenes
            .register(DockViewportHostSceneSnapshot::new(
                source_space.clone(),
                window.window_id(),
                DockViewportWindowBoundsFrame::GlobalScreen(bounds(100.0, 100.0, 320.0, 240.0)),
                bounds(0.0, 0.0, 320.0, 240.0),
                host_position,
                crate::DockDropGuideStyle::default(),
            ))
            .frame;
        assert!(
            host_scenes
                .push_frame_fact(
                    &frame,
                    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
                        root: tabs,
                        target_tabs: tabs,
                        bounds: bounds(0.0, 0.0, 320.0, 240.0),
                        is_central: true,
                    }),
                )
                .is_some()
        );

        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.register_panel(
            item("a"),
            DockPanel::lazy("Panel A", |_| unreachable!()).with_dock_class("inspector"),
        );
        let payload = DockViewportDropPayload::Item(item("a"));
        let payload_classes =
            workspace.payload_dock_classes_for_viewport_payload(&payload, DockNodeId::null());
        let request = DockViewportDropRouteRequest::from_target_context(
            source_space.clone(),
            DockNodeId::null(),
            payload,
            point(px(220.0), px(200.0)),
            None,
            DockViewportTargetContext::new().with_trusted_hovered_window(window),
        );

        let target = resolve_workspace_target_for_route(
            &adapter,
            &host_scenes,
            &DockViewportDropRoute::Local {
                host_position,
                window_id: window.window_id(),
                facts_generation: mismatched_facts_generation,
                source: crate::DockViewportRouteSelectionSource::EventReceiverLocalScene,
            },
            &request,
            &workspace,
            &payload_classes,
        );

        let DockViewportWorkspaceRouteTarget::Resolved(target) = target else {
            panic!("event-receiver-local-scene route should resolve against current host scene");
        };
        assert_eq!(target.facts_generation(), None);
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
