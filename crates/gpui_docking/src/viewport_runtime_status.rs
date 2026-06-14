use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockPolicyError, DockSpaceId,
    DockViewportActivationTarget, DockViewportCloseOutcome, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportDropRouteRequest,
    DockViewportShouldCloseOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffOpenOutcome,
    viewport_registry::{
        DockViewportRouteUnavailableReason, DockViewportSnapshot, DockViewportStaleReason,
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

/// Current route-readiness record for one registered viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportLifecycleRecord {
    /// Logical dock space rendered by the viewport window.
    pub space: DockSpaceId,
    /// GPUI window currently bound to the logical dock space.
    pub window_id: WindowId,
    /// Route-readiness status derived from the viewport lifecycle machine.
    pub route_status: DockViewportRouteStatus,
    /// Generation of the latest platform/host route facts.
    pub facts_generation: u64,
}

/// Route-readiness status for a registered viewport.
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
    /// The lifecycle state was ready, but one of the required route fact snapshots is absent.
    MissingRouteFacts,
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
    /// The route cancelled a pending tear-off before graph mutation.
    TearOffCancelled,
    /// The route opened a viewport but failed graph mutation afterward.
    TearOffCommitFailed,
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
    /// Panel item that should receive focus after activation, when known.
    pub focus_item: Option<DockItemId>,
}

/// Live platform-window sync attempted for a reused viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportPlatformSyncRecord {
    /// GPUI window that received the sync attempt.
    pub window_id: WindowId,
    /// Requests that were applied through the current GPUI window interface.
    pub applied: Vec<DockViewportPlatformSyncAction>,
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
    /// Cancel reason when the tear-off was cancelled.
    pub cancel_reason: Option<DockViewportTearOffCancelReason>,
    /// Commit error when the viewport opened but graph mutation failed.
    pub error: Option<DockActionApplyError>,
}

/// High-level tear-off transaction outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportTearOffOutcomeKind {
    /// Viewport registration and graph mutation completed.
    Completed,
    /// A duplicate request reused the existing pending tear-off.
    Duplicate,
    /// The request was cancelled before graph mutation.
    Cancelled,
    /// The viewport opened but graph mutation failed.
    CommitFailed,
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
                .activation_target()
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
}

impl DockViewportLifecycleRecord {
    pub(crate) fn from_snapshot(space: DockSpaceId, snapshot: &DockViewportSnapshot) -> Self {
        Self {
            space,
            window_id: snapshot.window.window_id(),
            route_status: DockViewportRouteStatus::from_snapshot(snapshot),
            facts_generation: snapshot.facts_generation(),
        }
    }
}

impl DockViewportRouteStatus {
    fn from_snapshot(snapshot: &DockViewportSnapshot) -> Self {
        match snapshot.route_unavailable_reason() {
            None => Self::RouteReady,
            Some(DockViewportRouteUnavailableReason::RegisteredNotReady) => {
                Self::RegisteredNotReady
            }
            Some(DockViewportRouteUnavailableReason::Stale(reason)) => Self::Stale {
                reason: DockViewportStaleStatusReason::from(reason),
            },
            Some(DockViewportRouteUnavailableReason::MissingRouteFacts) => Self::MissingRouteFacts,
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
            Self::Local { .. }
            | Self::TearOff { .. }
            | Self::Unavailable
            | Self::Rejected { .. } => None,
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
            DockViewportDropRoute::Local { host_position } => Self::Local {
                space: request.source_space().clone(),
                host_position: *host_position,
            },
            DockViewportDropRoute::KnownViewport { target } => Self::KnownViewport {
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
            DockViewportTearOffOpenOutcome::Cancelled(_cancelled) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCancelled,
                action: Some(DockActionOutcome::Unchanged),
                error: None,
            },
            DockViewportTearOffOpenOutcome::CommitFailed(failure) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCommitFailed,
                action: None,
                error: Some(failure.error().clone()),
            },
        }
    }
}

impl From<&DockViewportActivationTarget> for DockViewportActivationRecord {
    fn from(target: &DockViewportActivationTarget) -> Self {
        Self {
            space: target.space().clone(),
            window_id: target.window().window_id(),
            focus_item: target.focus_item().cloned(),
        }
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
                    cancel_reason: None,
                    error: None,
                }
            }
            DockViewportTearOffOpenOutcome::Duplicate(pending) => {
                let request = pending.request();
                Self {
                    kind: DockViewportTearOffOutcomeKind::Duplicate,
                    source_space: request.source_space().clone(),
                    target_space: pending.target_space().clone(),
                    payload: DockViewportPayloadRecord::from_payload(request.payload()),
                    cancel_reason: None,
                    error: None,
                }
            }
            DockViewportTearOffOpenOutcome::Cancelled(cancelled) => {
                let pending = cancelled.pending();
                let request = pending.request();
                Self {
                    kind: DockViewportTearOffOutcomeKind::Cancelled,
                    source_space: request.source_space().clone(),
                    target_space: pending.target_space().clone(),
                    payload: DockViewportPayloadRecord::from_payload(request.payload()),
                    cancel_reason: Some(cancelled.reason()),
                    error: None,
                }
            }
            DockViewportTearOffOpenOutcome::CommitFailed(failure) => {
                let pending = failure.pending();
                let request = pending.request();
                Self {
                    kind: DockViewportTearOffOutcomeKind::CommitFailed,
                    source_space: request.source_space().clone(),
                    target_space: pending.target_space().clone(),
                    payload: DockViewportPayloadRecord::from_payload(request.payload()),
                    cancel_reason: None,
                    error: Some(failure.error().clone()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockViewportWindowFacts,
        drag::DockDragPayload,
        interaction::DockRuntimeDragSession,
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
        assert_eq!(registered.facts_generation, 0);

        assert!(snapshot.update_route_facts(
            DockViewportWindowFacts::from_window_bounds(WindowBounds::Windowed(bounds(
                100.0, 200.0, 320.0, 240.0
            ))),
            bounds(0.0, 0.0, 320.0, 240.0)
        ));
        let ready = DockViewportLifecycleRecord::from_snapshot(space.clone(), &snapshot);
        assert_eq!(ready.route_status, DockViewportRouteStatus::RouteReady);
        assert_eq!(ready.facts_generation, 1);

        assert!(snapshot.mark_route_facts_stale(DockViewportStaleReason::WindowFactsChanged));
        let stale = DockViewportLifecycleRecord::from_snapshot(space, &snapshot);
        assert_eq!(
            stale.route_status,
            DockViewportRouteStatus::Stale {
                reason: DockViewportStaleStatusReason::WindowFactsChanged
            }
        );
        assert_eq!(stale.facts_generation, 2);
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
            crate::DockViewportPlatformSignals::default(),
        );

        status.record_route(&request, &DockViewportDropRoute::Local { host_position });

        let route = status
            .last_route
            .as_ref()
            .expect("route record should be captured");
        assert_eq!(route.source_space, source);
        assert_eq!(route.drag_session_id, None);
        assert_eq!(route.target.space(), Some(&source));
        assert_eq!(route.target.host_position(), Some(host_position));
        assert_eq!(route.target.window_id(), None);
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

        status.record_route(&request, &DockViewportDropRoute::Local { host_position });

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
                Some(DockViewportActivationTarget::new(
                    target_space.clone(),
                    target_window,
                    Some(focus_item.clone()),
                )),
            ),
        )));
        assert_eq!(
            status.last_activation,
            Some(DockViewportActivationRecord {
                space: target_space,
                window_id: target_window.window_id(),
                focus_item: Some(focus_item),
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
}
