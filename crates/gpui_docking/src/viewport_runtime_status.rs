use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockPolicyError, DockSpaceId,
    DockViewportActivationTransaction, DockViewportCloseOutcome, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportDropRouteRequest,
    DockViewportFocusRequest, DockViewportShouldCloseOutcome, DockViewportTearOffOpenOutcome,
    viewport_registry::{
        DockViewportInputMask, DockViewportPlatformRequests, DockViewportRouteUnavailableReason,
        DockViewportSnapshot, DockViewportStaleReason,
    },
};
use open_gpui::{
    DisplayId, Pixels, Point, Size, WindowBackgroundAppearance, WindowDecorations, WindowId,
};

/// Read-only diagnostic snapshot for the viewport runtime.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockViewportRuntimeStatus {
    /// Current lifecycle/readiness records for registered platform viewports.
    pub viewport_lifecycle: Vec<DockViewportLifecycleRecord>,
    /// Most recent viewport route resolved for a rendered drop.
    pub last_route: Option<DockViewportRouteRecord>,
    /// Most recent routed drop outcome.
    pub last_drop_outcome: Option<DockViewportDropOutcomeRecord>,
    /// Most recent viewport activation requested by a routed drop.
    pub last_activation: Option<DockViewportActivationRecord>,
    /// Most recent platform close cleanup outcome.
    pub last_close: Option<DockViewportCloseOutcome>,
    /// Most recent platform should-close query outcome.
    pub last_should_close: Option<DockViewportShouldCloseOutcome>,
    /// Most recent tear-off transaction outcome.
    pub last_tear_off: Option<DockViewportTearOffRecord>,
    /// Most recent live platform-window sync attempted for a reused viewport.
    pub last_platform_sync: Option<DockViewportPlatformSyncRecord>,
}

/// Current route-facts and platform-request record for one registered viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportLifecycleRecord {
    /// Logical dock space rendered by the viewport window.
    pub space: DockSpaceId,
    /// GPUI window currently bound to the logical dock space.
    pub window_id: WindowId,
    /// Route-facts status derived from the viewport lifecycle machine.
    pub route_status: DockViewportRouteStatus,
    /// Platform input-mask state observed for the viewport window.
    pub input_status: DockViewportInputStatus,
    /// Platform request flags pending for this viewport window.
    pub platform_request_status: DockViewportPlatformRequestStatus,
    /// Generation of the latest platform/host route facts.
    pub facts_generation: u64,
}

/// Route-facts status for a registered viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportRouteStatus {
    /// The space/window binding exists, but no rendered host scene has published route facts yet.
    RegisteredNotReady,
    /// The latest rendered host scene and platform window facts can be used for routing.
    RouteReady,
    /// Previously published route facts were invalidated and need a fresh rendered host scene.
    Stale {
        /// Reason the viewport was demoted from route-ready to stale.
        reason: DockViewportStaleStatusReason,
    },
    /// The latest platform facts say the window is minimized.
    Minimized,
    /// The lifecycle state was ready, but one of the required route fact snapshots is absent.
    MissingRouteFacts,
}

/// Platform request status for a registered viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DockViewportPlatformRequestStatus {
    /// The platform has requested that this viewport should close.
    pub close_requested: bool,
    /// The platform has requested or reported an authoritative resize.
    pub resize_requested: bool,
}

/// Platform input-mask status for a registered viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportInputStatus {
    /// The window receives pointer input.
    ReceivesInput,
    /// The window is minimized.
    Minimized,
    /// The window is native no-input/click-through.
    NoInputPassThrough,
}

/// Public diagnostic reason for stale viewport route facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportStaleStatusReason {
    /// GPUI reported platform window facts changed after the last rendered host scene.
    WindowFactsChanged,
}

/// Payload shape recorded in viewport runtime diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockViewportPayloadRecord {
    /// One tab item was routed.
    Item {
        /// Routed item id.
        item: DockItemId,
    },
    /// The entire source tabs stack was routed.
    Tabs,
    /// An in-window floating subtree was routed.
    Floating {
        /// Routed floating container node.
        floating: DockNodeId,
    },
}

/// Route resolution recorded before a rendered drop mutates the workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportRouteRecord {
    /// Source dock space where the payload drag started.
    pub source_space: DockSpaceId,
    /// Source graph node that owns the routed payload.
    pub source_node: DockNodeId,
    /// Payload being routed.
    pub payload: DockViewportPayloadRecord,
    /// Runtime drag session that produced this route, when known.
    pub drag_session_id: Option<u64>,
    /// Runtime route selected for the release point.
    pub target: DockViewportRouteTarget,
}

/// Runtime route selected for a rendered drop release.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportRouteTarget {
    /// The release stayed in the source viewport.
    Local {
        /// Source dock space that should commit locally.
        space: DockSpaceId,
        /// GPUI window id that rendered the source viewport.
        window_id: WindowId,
        /// Pointer position in source host coordinates.
        host_position: Point<Pixels>,
    },
    /// The release hit another registered viewport.
    KnownViewport {
        /// Destination dock space.
        space: DockSpaceId,
        /// Destination GPUI window id.
        window_id: WindowId,
        /// Pointer position in destination host coordinates.
        host_position: Point<Pixels>,
    },
    /// The release was outside all registered viewports and can become a platform tear-off.
    TearOff {
        /// Screen position where the payload was released.
        release_position: Point<Pixels>,
    },
    /// The release hit a registered viewport that had no current dock target.
    Unavailable,
    /// The release was rejected by policy before mutation.
    Rejected {
        /// Policy reason that rejected the route.
        reason: DockPolicyError,
    },
}

/// Outcome recorded after a routed drop commit attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportDropOutcomeRecord {
    /// High-level outcome kind.
    pub kind: DockViewportDropOutcomeKind,
    /// Workspace action result when one was produced.
    pub action: Option<DockActionOutcome>,
    /// Commit error when the route was rejected or failed.
    pub error: Option<DockActionApplyError>,
}

/// High-level outcome kind for a routed drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportDropOutcomeKind {
    /// The route committed through a normal workspace action.
    Action,
    /// The route completed a tear-off transaction.
    TearOffCompleted,
    /// The route matched an existing pending tear-off.
    TearOffDuplicate,
    /// The route failed before producing a viewport outcome.
    Error,
}

/// Viewport activation requested by a routed drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportActivationRecord {
    /// Logical dock space that should become active.
    pub space: DockSpaceId,
    /// GPUI window id that should become active.
    pub window_id: WindowId,
    /// Focus request that should be applied after activation.
    pub focus_request: DockViewportFocusRequest,
}

/// Live platform-window sync attempted for a reused viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportPlatformSyncRecord {
    /// GPUI window that received the sync attempt.
    pub window_id: WindowId,
    /// Requests that were applied through the current GPUI window interface.
    pub applied: Vec<DockViewportPlatformSyncAction>,
    /// Requests intentionally skipped because the platform backend already reported an
    /// authoritative live-window request for the same property.
    pub skipped_requests: Vec<DockViewportPlatformSyncSkipped>,
    /// Requests that could not be applied because GPUI has no matching live mutation interface yet.
    pub unsupported_requests: Vec<DockViewportPlatformSyncUnsupported>,
}

/// Platform-window request successfully applied while reusing an existing viewport.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportPlatformSyncAction {
    /// Focused and raised the platform window.
    Activate,
    /// Updated the platform window title.
    Title {
        /// Requested title.
        title: String,
    },
    /// Updated the platform application id.
    AppId {
        /// Requested application id.
        app_id: String,
    },
    /// Updated the window content size.
    Resize {
        /// Requested content size.
        size: Size<Pixels>,
    },
    /// Updated the platform window fullscreen state.
    Fullscreen {
        /// Whether fullscreen was enabled.
        enabled: bool,
    },
    /// Updated the platform window background appearance.
    BackgroundAppearance {
        /// Requested background appearance.
        appearance: WindowBackgroundAppearance,
    },
    /// Requested client/server decorations from the platform window.
    WindowDecorations {
        /// Requested decoration mode.
        decorations: WindowDecorations,
    },
    /// Updated native pointer-input routing for a viewport window.
    PointerInput {
        /// Whether pointer input is enabled.
        enabled: bool,
    },
    /// Updated the macOS traffic-light position.
    TrafficLightPosition {
        /// Requested traffic-light position.
        position: Point<Pixels>,
    },
}

/// Platform-window request that could not be applied to a reused viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportPlatformSyncUnsupported {
    /// Unsupported request.
    pub request: DockViewportPlatformSyncRequest,
    /// Why the request was not applied.
    pub reason: DockViewportPlatformSyncUnsupportedReason,
}

/// Platform-window request intentionally skipped during reused-window sync.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportPlatformSyncSkipped {
    /// Skipped request.
    pub request: DockViewportPlatformSyncRequest,
    /// Why the request was skipped.
    pub reason: DockViewportPlatformSyncSkippedReason,
}

/// Platform-window request shape used by sync diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportPlatformSyncRequest {
    /// Requested platform visibility differs from the already-open window.
    Show {
        /// Requested visibility.
        requested: bool,
    },
    /// Requested platform window kind differs from the already-open window.
    WindowKind,
    /// Requested movability differs from the already-open window.
    Movable {
        /// Requested movability.
        requested: bool,
    },
    /// Requested resizability differs from the already-open window.
    Resizable {
        /// Requested resizability.
        requested: bool,
    },
    /// Requested minimizability differs from the already-open window.
    Minimizable {
        /// Requested minimizability.
        requested: bool,
    },
    /// Requested native pointer-input routing differs from the already-open window.
    PointerInput {
        /// Requested pointer input state.
        requested: bool,
    },
    /// Requested display differs from the already-open window.
    Display {
        /// Requested display id.
        requested: DisplayId,
    },
    /// Requested minimum window size differs from the already-open window.
    WindowMinSize {
        /// Requested minimum size.
        requested: Size<Pixels>,
    },
    /// Requested live content size differs from the already-open window.
    WindowSize {
        /// Requested content size.
        requested: Size<Pixels>,
    },
    /// Requested icon differs from the already-open window.
    Icon,
    /// Requested native tabbing identifier differs from the already-open window.
    TabbingIdentifier {
        /// Requested native tabbing identifier.
        requested: String,
    },
    /// Requested titlebar presence differs from the already-open window.
    TitlebarPresence {
        /// Requested titlebar presence.
        requested: bool,
    },
    /// Requested window origin differs from the already-open window.
    WindowOrigin {
        /// Requested window origin.
        requested: Point<Pixels>,
    },
    /// Requested platform window state differs from the already-open window.
    WindowState {
        /// Requested window state.
        requested: DockViewportPlatformWindowState,
    },
    /// Requested titlebar transparency differs from the already-open window.
    TitlebarTransparency {
        /// Requested titlebar transparency.
        requested: bool,
    },
    /// Requested macOS traffic-light position could not be applied on this platform.
    TrafficLightPosition {
        /// Requested traffic-light position.
        requested: Point<Pixels>,
    },
}

/// Platform window state requested through `WindowOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportPlatformWindowState {
    /// Normal windowed state.
    Windowed,
    /// Maximized state.
    Maximized,
    /// Fullscreen state.
    Fullscreen,
}

/// Why a platform-window sync request could not be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportPlatformSyncUnsupportedReason {
    /// GPUI's public `Window` interface does not expose a live mutation for this request.
    UnsupportedByWindowApi,
}

/// Why a platform-window sync request was intentionally skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportPlatformSyncSkippedReason {
    /// The platform backend has already reported a live move/resize request for this window.
    PlatformRequestInProgress,
}

/// Tear-off transaction outcome recorded by the viewport runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRecord {
    /// High-level tear-off outcome kind.
    pub kind: DockViewportTearOffOutcomeKind,
    /// Source dock space where the tear-off started.
    pub source_space: DockSpaceId,
    /// Target dock space opened for the tear-off.
    pub target_space: DockSpaceId,
    /// Payload that was torn off.
    pub payload: DockViewportPayloadRecord,
}

/// High-level tear-off transaction outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportTearOffOutcomeKind {
    /// Viewport registration and graph mutation completed.
    Completed,
    /// A duplicate request reused the existing pending tear-off.
    Duplicate,
}

impl DockViewportRuntimeStatus {
    pub(crate) fn with_viewport_lifecycle(
        mut self,
        viewport_lifecycle: Vec<DockViewportLifecycleRecord>,
    ) -> Self {
        self.viewport_lifecycle = viewport_lifecycle;
        self
    }

    pub(crate) fn record_route(
        &mut self,
        request: &DockViewportDropRouteRequest,
        route: &DockViewportDropRoute,
    ) {
        self.last_route = Some(DockViewportRouteRecord {
            source_space: request.source_space().clone(),
            source_node: request.source_node(),
            payload: DockViewportPayloadRecord::from_payload(request.payload()),
            drag_session_id: request.drag_session().map(|session| session.id()),
            target: DockViewportRouteTarget::from_route(request, route),
        });
    }

    pub(crate) fn record_drop_result(
        &mut self,
        result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) {
        self.last_drop_outcome = Some(DockViewportDropOutcomeRecord::from_result(result));
        self.last_activation = None;
        if let Ok(outcome) = result {
            self.last_activation = outcome
                .activation_transaction()
                .as_ref()
                .map(DockViewportActivationRecord::from);
            if let DockViewportDropRouteOutcome::TearOff(tear_off) = outcome {
                self.record_tear_off(tear_off.as_ref());
            }
        }
    }

    pub(crate) fn record_tear_off(&mut self, outcome: &DockViewportTearOffOpenOutcome) {
        self.last_tear_off = Some(DockViewportTearOffRecord::from_outcome(outcome));
    }

    pub(crate) fn record_close(&mut self, outcome: &DockViewportCloseOutcome) {
        self.last_close = Some(outcome.clone());
    }

    pub(crate) fn record_should_close(&mut self, outcome: &DockViewportShouldCloseOutcome) {
        self.last_should_close = Some(outcome.clone());
    }

    pub(crate) fn record_platform_sync(&mut self, record: DockViewportPlatformSyncRecord) {
        self.last_platform_sync = Some(record);
    }

    pub(crate) fn clear_window_references(&mut self, space: &DockSpaceId, window_id: WindowId) {
        if self
            .last_route
            .as_ref()
            .is_some_and(|route| route.references_window(space, window_id))
        {
            self.last_route = None;
        }
        if self
            .last_activation
            .as_ref()
            .is_some_and(|activation| activation.references_window(space, window_id))
        {
            self.last_activation = None;
        }
        if self
            .last_platform_sync
            .as_ref()
            .is_some_and(|sync| sync.window_id == window_id)
        {
            self.last_platform_sync = None;
        }
    }
}

impl DockViewportLifecycleRecord {
    pub(crate) fn from_snapshot(space: DockSpaceId, snapshot: &DockViewportSnapshot) -> Self {
        Self {
            space,
            window_id: snapshot.window.window_id(),
            route_status: DockViewportRouteStatus::from_snapshot(snapshot),
            input_status: DockViewportInputStatus::from(snapshot.input_mask),
            platform_request_status: DockViewportPlatformRequestStatus::from(
                snapshot.platform_requests(),
            ),
            facts_generation: snapshot.facts_generation(),
        }
    }
}

impl DockViewportRouteStatus {
    fn from_snapshot(snapshot: &DockViewportSnapshot) -> Self {
        match snapshot.route_facts_unavailable_reason() {
            None => Self::RouteReady,
            Some(DockViewportRouteUnavailableReason::PlatformCloseRequested) => {
                unreachable!("platform close requests are not route-facts lifecycle state")
            }
            Some(DockViewportRouteUnavailableReason::RegisteredNotReady) => {
                Self::RegisteredNotReady
            }
            Some(DockViewportRouteUnavailableReason::Stale(reason)) => Self::Stale {
                reason: DockViewportStaleStatusReason::from(reason),
            },
            Some(DockViewportRouteUnavailableReason::Minimized) => Self::Minimized,
            Some(DockViewportRouteUnavailableReason::MissingRouteFacts) => Self::MissingRouteFacts,
        }
    }
}

impl From<DockViewportInputMask> for DockViewportInputStatus {
    fn from(input_mask: DockViewportInputMask) -> Self {
        match input_mask {
            DockViewportInputMask::ReceivesInput => Self::ReceivesInput,
            DockViewportInputMask::Minimized => Self::Minimized,
            DockViewportInputMask::NoInputPassThrough => Self::NoInputPassThrough,
        }
    }
}

impl From<DockViewportPlatformRequests> for DockViewportPlatformRequestStatus {
    fn from(requests: DockViewportPlatformRequests) -> Self {
        Self {
            close_requested: requests.close_requested,
            resize_requested: requests.resize_requested,
        }
    }
}

impl From<DockViewportStaleReason> for DockViewportStaleStatusReason {
    fn from(reason: DockViewportStaleReason) -> Self {
        match reason {
            DockViewportStaleReason::WindowFactsChanged => Self::WindowFactsChanged,
        }
    }
}

impl DockViewportRouteTarget {
    /// Returns the dock space for routes that target an existing workspace.
    pub fn space(&self) -> Option<&DockSpaceId> {
        match self {
            Self::Local { space, .. } | Self::KnownViewport { space, .. } => Some(space),
            Self::TearOff { .. } | Self::Unavailable | Self::Rejected { .. } => None,
        }
    }

    /// Returns the destination window id for routes that target a registered viewport.
    pub fn window_id(&self) -> Option<WindowId> {
        match self {
            Self::KnownViewport { window_id, .. } => Some(*window_id),
            Self::Local { window_id, .. } => Some(*window_id),
            Self::TearOff { .. } | Self::Unavailable | Self::Rejected { .. } => None,
        }
    }

    /// Returns the host-relative pointer position for routes into an existing workspace.
    pub fn host_position(&self) -> Option<Point<Pixels>> {
        match self {
            Self::Local { host_position, .. } | Self::KnownViewport { host_position, .. } => {
                Some(*host_position)
            }
            Self::TearOff { .. } | Self::Unavailable | Self::Rejected { .. } => None,
        }
    }

    /// Returns the screen release position for tear-off routes.
    pub fn release_position(&self) -> Option<Point<Pixels>> {
        match self {
            Self::TearOff { release_position } => Some(*release_position),
            Self::Local { .. }
            | Self::KnownViewport { .. }
            | Self::Unavailable
            | Self::Rejected { .. } => None,
        }
    }

    /// Returns the policy rejection reason for rejected routes.
    pub fn rejection_reason(&self) -> Option<DockPolicyError> {
        match self {
            Self::Rejected { reason } => Some(reason.clone()),
            Self::Local { .. }
            | Self::KnownViewport { .. }
            | Self::TearOff { .. }
            | Self::Unavailable => None,
        }
    }

    fn from_route(request: &DockViewportDropRouteRequest, route: &DockViewportDropRoute) -> Self {
        match route {
            DockViewportDropRoute::Local {
                host_position,
                window_id,
                ..
            } => Self::Local {
                space: request.source_space().clone(),
                window_id: *window_id,
                host_position: *host_position,
            },
            DockViewportDropRoute::KnownViewport { target, .. } => Self::KnownViewport {
                space: target.space().clone(),
                window_id: target.window_id(),
                host_position: target.host_position(),
            },
            DockViewportDropRoute::TearOff => Self::TearOff {
                release_position: request.release_position(),
            },
            DockViewportDropRoute::Unavailable => Self::Unavailable,
            DockViewportDropRoute::Rejected(reason) => Self::Rejected {
                reason: reason.clone(),
            },
        }
    }

    fn references_window(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        match self {
            Self::Local {
                space: route_space,
                window_id: route_window,
                ..
            }
            | Self::KnownViewport {
                space: route_space,
                window_id: route_window,
                ..
            } => route_space == space && *route_window == window_id,
            Self::TearOff { .. } | Self::Unavailable | Self::Rejected { .. } => false,
        }
    }
}

impl DockViewportRouteRecord {
    fn references_window(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.target.references_window(space, window_id)
    }
}

impl DockViewportPayloadRecord {
    fn from_payload(payload: &DockViewportDropPayload) -> Self {
        match payload {
            DockViewportDropPayload::Item(item) => Self::Item { item: item.clone() },
            DockViewportDropPayload::Tabs => Self::Tabs,
            DockViewportDropPayload::Floating(floating) => Self::Floating {
                floating: *floating,
            },
        }
    }
}

impl DockViewportDropOutcomeRecord {
    fn from_result(result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>) -> Self {
        match result {
            Ok(DockViewportDropRouteOutcome::Action(outcome)) => Self {
                kind: DockViewportDropOutcomeKind::Action,
                action: Some(outcome.action()),
                error: None,
            },
            Ok(DockViewportDropRouteOutcome::TearOff(outcome)) => {
                Self::from_tear_off_outcome(outcome.as_ref())
            }
            Err(error) => Self {
                kind: DockViewportDropOutcomeKind::Error,
                action: None,
                error: Some(error.clone()),
            },
        }
    }

    fn from_tear_off_outcome(outcome: &DockViewportTearOffOpenOutcome) -> Self {
        match outcome {
            DockViewportTearOffOpenOutcome::Completed(completed) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCompleted,
                action: Some(completed.action()),
                error: None,
            },
            DockViewportTearOffOpenOutcome::Duplicate(_pending) => Self {
                kind: DockViewportDropOutcomeKind::TearOffDuplicate,
                action: Some(DockActionOutcome::Unchanged),
                error: None,
            },
        }
    }
}

impl From<&DockViewportActivationTransaction> for DockViewportActivationRecord {
    fn from(target: &DockViewportActivationTransaction) -> Self {
        Self {
            space: target.space().clone(),
            window_id: target.window().window_id(),
            focus_request: target.focus_request().clone(),
        }
    }
}

impl DockViewportActivationRecord {
    fn references_window(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.space == *space && self.window_id == window_id
    }
}

impl DockViewportTearOffRecord {
    fn from_outcome(outcome: &DockViewportTearOffOpenOutcome) -> Self {
        match outcome {
            DockViewportTearOffOpenOutcome::Completed(completed) => {
                let pending = completed.pending();
                let request = pending.request();
                Self {
                    kind: DockViewportTearOffOutcomeKind::Completed,
                    source_space: request.source_space().clone(),
                    target_space: pending.target_space().clone(),
                    payload: DockViewportPayloadRecord::from_payload(request.payload()),
                }
            }
            DockViewportTearOffOpenOutcome::Duplicate(pending) => {
                let request = pending.request();
                Self {
                    kind: DockViewportTearOffOutcomeKind::Duplicate,
                    source_space: request.source_space().clone(),
                    target_space: pending.target_space().clone(),
                    payload: DockViewportPayloadRecord::from_payload(request.payload()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DockViewportFocusRequest;
    use crate::{
        DockViewportInputStatus, DockViewportWindowFacts,
        drag::DockDragPayload,
        interaction::DockRuntimeDragSession,
        viewport_registry::DockViewportInputMask,
        viewport_test_support::{bounds, handle, space},
    };
    use open_gpui::{WindowBounds, point, px};
    use slotmap::Key;

    #[test]
    fn viewport_lifecycle_record_reports_route_status_from_snapshot() {
        let space = space("main");
        let window = handle(7);
        let mut snapshot = DockViewportSnapshot::new(window);

        let registered = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(registered.space, space);
        assert_eq!(registered.window_id, window.window_id());
        assert_eq!(
            registered.route_status,
            DockViewportRouteStatus::RegisteredNotReady
        );
        assert_eq!(registered.input_status, DockViewportInputStatus::Minimized);
        assert_eq!(
            registered.platform_request_status,
            DockViewportPlatformRequestStatus::default()
        );
        assert_eq!(registered.facts_generation, 0);

        assert!(snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 320.0, 240.0
            ))),
            bounds(0.0, 0.0, 320.0, 240.0)
        ));
        let ready = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(ready.route_status, DockViewportRouteStatus::RouteReady);
        assert_eq!(ready.input_status, DockViewportInputStatus::ReceivesInput);
        assert_eq!(
            ready.platform_request_status,
            DockViewportPlatformRequestStatus::default()
        );
        assert_eq!(ready.facts_generation, 1);

        assert!(
            snapshot.update_route_facts(
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 200.0, 320.0, 240.0
                )))
                .with_input_mask(DockViewportInputMask::Minimized),
                bounds(0.0, 0.0, 320.0, 240.0)
            )
        );
        let minimized = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(minimized.route_status, DockViewportRouteStatus::Minimized);
        assert_eq!(minimized.input_status, DockViewportInputStatus::Minimized);
        assert_eq!(
            minimized.facts_generation, 1,
            "input-mask-only changes do not advance route facts generation"
        );

        assert!(
            snapshot.update_route_facts(
                DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                    100.0, 200.0, 320.0, 240.0
                )))
                .with_input_mask(DockViewportInputMask::NoInputPassThrough),
                bounds(0.0, 0.0, 320.0, 240.0)
            )
        );
        let no_input = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(no_input.route_status, DockViewportRouteStatus::RouteReady);
        assert_eq!(
            no_input.input_status,
            DockViewportInputStatus::NoInputPassThrough
        );
        assert_eq!(
            no_input.facts_generation, 1,
            "input-mask-only changes do not advance route facts generation"
        );

        assert!(snapshot.mark_route_facts_stale(DockViewportStaleReason::WindowFactsChanged));
        let stale = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(
            stale.route_status,
            DockViewportRouteStatus::Stale {
                reason: DockViewportStaleStatusReason::WindowFactsChanged
            }
        );
        assert_eq!(stale.facts_generation, 2);

        assert!(snapshot.mark_platform_close_requested());
        let closing = DockViewportLifecycleRecord::from_snapshot(space, &snapshot);
        assert_eq!(
            closing.route_status,
            DockViewportRouteStatus::Stale {
                reason: DockViewportStaleStatusReason::WindowFactsChanged
            },
            "close requests do not replace the current route-facts lifecycle"
        );
        assert_eq!(
            closing.platform_request_status,
            DockViewportPlatformRequestStatus {
                close_requested: true,
                resize_requested: false,
            }
        );
        assert_eq!(closing.facts_generation, 2);
    }

    #[test]
    fn route_record_derives_local_target_identity_from_source() {
        let source = DockSpaceId::from("source");
        let host_position = point(px(12.0), px(34.0));
        let mut status = DockViewportRuntimeStatus::default();

        let request = DockViewportDropRouteRequest::from_platform_signals(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Tabs,
            host_position,
            None,
            crate::DockViewportPlatformSignals::default().with_trusted_hovered_window(handle(7)),
        );

        status.record_route(
            &request,
            &DockViewportDropRoute::Local {
                host_position,
                window_id: handle(7).window_id(),
                facts_generation: 1,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
        );

        let route = status
            .last_route
            .as_ref()
            .expect("route record should be captured");
        assert_eq!(route.source_space, source);
        assert_eq!(route.drag_session_id, None);
        assert_eq!(route.target.space(), Some(&source));
        assert_eq!(route.target.host_position(), Some(host_position));
        assert_eq!(route.target.window_id(), Some(WindowId::from(7)));
    }

    #[test]
    fn route_record_preserves_runtime_drag_session_id() {
        let source = DockSpaceId::from("source");
        let source_tabs = DockNodeId::null();
        let host_position = point(px(12.0), px(34.0));
        let payload = DockDragPayload::new_tabs(source.clone(), source_tabs, "Stack".to_string());
        let drag_session = DockRuntimeDragSession::new(19, &payload);
        let mut status = DockViewportRuntimeStatus::default();

        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            source_tabs,
            DockViewportDropPayload::Tabs,
            host_position,
            None,
            crate::DockViewportPlatformSignals::default(),
        )
        .with_drag_session(Some(drag_session));

        status.record_route(
            &request,
            &DockViewportDropRoute::Local {
                host_position,
                window_id: handle(7).window_id(),
                facts_generation: 1,
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
        );

        assert_eq!(
            status
                .last_route
                .as_ref()
                .map(|route| route.drag_session_id),
            Some(Some(19))
        );
    }

    #[test]
    fn drop_error_clears_previous_activation_record() {
        let mut status = DockViewportRuntimeStatus::default();
        let target_space = DockSpaceId::from("target");
        let target_window = handle(44);
        let focus_item = DockItemId::from("a");

        status.record_drop_result(&Ok(DockViewportDropRouteOutcome::Action(
            crate::DockViewportDropActionOutcome::new(
                DockActionOutcome::Changed,
                Some(DockViewportActivationTransaction::new(
                    target_space.clone(),
                    target_window,
                    DockViewportFocusRequest::panel(focus_item.clone()),
                )),
            ),
        )));
        assert_eq!(
            status.last_activation,
            Some(DockViewportActivationRecord {
                space: target_space,
                window_id: target_window.window_id(),
                focus_request: DockViewportFocusRequest::panel(focus_item),
            })
        );

        status.record_drop_result(&Err(DockActionApplyError::DropTargetUnavailable));

        assert_eq!(
            status
                .last_drop_outcome
                .as_ref()
                .map(|outcome| outcome.kind),
            Some(DockViewportDropOutcomeKind::Error)
        );
        assert_eq!(status.last_activation, None);
    }

    #[test]
    fn activation_record_preserves_no_panel_focus_request() {
        let mut status = DockViewportRuntimeStatus::default();
        let target_space = DockSpaceId::from("target");
        let target_window = handle(44);

        status.record_drop_result(&Ok(DockViewportDropRouteOutcome::Action(
            crate::DockViewportDropActionOutcome::new(
                DockActionOutcome::Changed,
                Some(DockViewportActivationTransaction::new(
                    target_space.clone(),
                    target_window,
                    DockViewportFocusRequest::no_panel_focus(),
                )),
            ),
        )));

        assert_eq!(
            status.last_activation,
            Some(DockViewportActivationRecord {
                space: target_space,
                window_id: target_window.window_id(),
                focus_request: DockViewportFocusRequest::no_panel_focus(),
            })
        );
    }

    #[test]
    fn clearing_window_references_removes_current_window_diagnostics() {
        let source = DockSpaceId::from("source");
        let target = DockSpaceId::from("target");
        let source_tabs = DockNodeId::null();
        let target_window = handle(44);
        let host_position = point(px(12.0), px(34.0));
        let focus_item = DockItemId::from("a");
        let mut status = DockViewportRuntimeStatus::default();

        let request = DockViewportDropRouteRequest::from_platform_signals(
            source,
            source_tabs,
            DockViewportDropPayload::Tabs,
            host_position,
            None,
            crate::DockViewportPlatformSignals::default()
                .with_trusted_hovered_window(target_window),
        );
        status.record_route(
            &request,
            &DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::new(
                    target.clone(),
                    target_window,
                    host_position,
                ),
                authority: crate::DockViewportAuthorizedRouteAuthority::TrustedHoveredWindow,
            },
        );
        status.record_drop_result(&Ok(DockViewportDropRouteOutcome::Action(
            crate::DockViewportDropActionOutcome::new(
                DockActionOutcome::Changed,
                Some(DockViewportActivationTransaction::new(
                    target.clone(),
                    target_window,
                    DockViewportFocusRequest::panel(focus_item),
                )),
            ),
        )));
        status.record_platform_sync(DockViewportPlatformSyncRecord {
            window_id: target_window.window_id(),
            applied: Vec::new(),
            skipped_requests: Vec::new(),
            unsupported_requests: Vec::new(),
        });

        status.clear_window_references(&target, target_window.window_id());

        assert_eq!(status.last_route, None);
        assert_eq!(status.last_activation, None);
        assert_eq!(status.last_platform_sync, None);
        assert_eq!(
            status
                .last_drop_outcome
                .as_ref()
                .map(|outcome| outcome.kind),
            Some(DockViewportDropOutcomeKind::Action),
            "drop outcomes remain historical commit results rather than live window references"
        );
    }
}
