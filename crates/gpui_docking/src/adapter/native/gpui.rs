use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::{Rc, Weak},
};

use dockspace::{
    geometry::{PhysicalRect, PhysicalSize, ScaleFactor},
    runtime::{
        DockspaceSession, NativeCleanupObservation, NativeCloseDisposition, NativeCloseState,
        NativeCreateReadiness, NativeDispatchFailure, NativeDragTearOff,
        NativeEffectAcknowledgement, NativeEffectDispatch, NativeEffectOperation,
        NativeEffectRequest, NativeHostCapability, NativeIndeterminateReason, NativeLocalReadiness,
        NativeSurfaceBinding, NativeSurfaceRole, NativeUnsupportedReason, NativeWindowBinding,
        NativeWindowClientGeometry, NativeWindowFacts, NativeWindowFactsSample,
        NativeWindowInputState, NativeWindowObservationOutbox, compile_native_window_facts,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Entity, NativeCapturedDragReleaseBarrier,
    PlatformWindowActivationSupport, Render, Subscription, Window, WindowActivationTerminal,
    WindowActivationTicket, WindowFrameSubmissionTicket, WindowInitialPresentationStatus,
    WindowMutationDispatch, WindowMutationOutcome, WindowMutationTicket, WindowOpenError,
    WindowOpenFailureStage, WindowOptions, WindowPhysicalPlacementRequest, WindowPlatformFacts,
    WindowPresentationShutdownTicket, WindowProvisionalPlacementOutcome,
    WindowProvisionalPlacementSnapshot, WindowProvisionalPlacementTicket,
};

use super::{
    NativeBridge, NativeBridgeError, NativeReport, NativeTransport, NativeWindowFactory,
    input::NativeCapture, transport::Inbound,
};

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod geometry_tests;

#[cfg(test)]
#[path = "external_root_retirement_tests.rs"]
mod external_root_retirement_tests;

mod capture_driver;
mod effect_dispatch;
mod termination;
mod window_lifecycle;

pub(super) use effect_dispatch::PendingDispatch;

type OuterBounds = dyn Fn(&WindowPlatformFacts) -> Option<PhysicalRect>;
type ClientPlacement = dyn Fn(PhysicalRect, &Window) -> Option<WindowPhysicalPlacementRequest>;

/// Native frame geometry supplied by the owning platform adapter.
///
/// GPUI's physical placement describes client bounds; dockspace requests outer
/// bounds. Decoration insets must come from the real native owner at the selected
/// DPI. A missing conversion is unsupported, never an outer/client substitution.
pub struct NativeGeometry {
    outer_bounds: Rc<OuterBounds>,
    client_placement: Rc<ClientPlacement>,
}

impl NativeGeometry {
    /// Uses coherent GPUI outer observations and the owning backend's target-DPI conversion.
    pub fn gpui() -> Self {
        Self::new(
            |facts| super::input::physical_rect(facts.physical_geometry?.outer_bounds()?),
            Self::gpui_client_placement,
        )
    }

    pub(crate) fn gpui_client_placement(
        outer: PhysicalRect,
        window: &Window,
    ) -> Option<WindowPhysicalPlacementRequest> {
        window.physical_placement_for_outer_bounds(super::input::device_bounds(outer)?)
    }
    /// Installs real observed outer geometry and outer-to-client placement conversion.
    pub fn new(
        outer_bounds: impl Fn(&WindowPlatformFacts) -> Option<PhysicalRect> + 'static,
        client_placement: impl Fn(PhysicalRect, &Window) -> Option<WindowPhysicalPlacementRequest>
        + 'static,
    ) -> Self {
        Self {
            outer_bounds: Rc::new(outer_bounds),
            client_placement: Rc::new(client_placement),
        }
    }
}

/// The original core binding, outer rectangle and ownership role sent to a factory.
#[derive(Clone, Copy, Debug)]
pub struct NativeWindowRequest {
    /// Exact pending surface and native lifetime minted by core.
    pub binding: NativeSurfaceBinding,
    /// Requested outer frame in physical desktop pixels, including native decorations.
    pub outer_bounds: PhysicalRect,
    /// Core's ownership role; it does not imply native transient parenting.
    pub role: NativeSurfaceRole,
    /// Exact pointer drag whose provisional resource this request may adopt.
    /// Programmatic creates and replacements carry no drag identity.
    pub drag_tear_off: Option<NativeDragTearOff>,
}

/// A factory failure retains GPUI's actual typed creation stage when available.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct NativeOpenFailure {
    /// Original root-binding failure, when the native transaction reached its root builder.
    pub root_binding: Option<NativeBridgeError>,
    /// Original typed GPUI transaction stage, when available.
    pub stage: Option<WindowOpenFailureStage>,
    /// Diagnostic emitted by the actual window factory.
    pub message: String,
    /// Original GPUI construction-rollback authority, when a native owner was created.
    pub retirement_ticket: Option<WindowPresentationShutdownTicket>,
    /// Original typed GPUI error, preserving its backend error chain.
    #[source]
    pub source: Option<WindowOpenError>,
}

impl From<WindowOpenError> for NativeOpenFailure {
    fn from(error: WindowOpenError) -> Self {
        Self {
            root_binding: None,
            stage: Some(error.stage()),
            message: error.to_string(),
            retirement_ticket: error.retirement_ticket().cloned(),
            source: Some(error),
        }
    }
}

struct CallbackState {
    binding: Cell<NativeWindowBinding>,
    create_request: Cell<Option<NativeWindowRequest>>,
    window: Cell<Option<AnyWindowHandle>>,
    transport: NativeTransport,
    geometry: Option<Rc<NativeGeometry>>,
    bounds_observer: RefCell<Option<Entity<Vec<Subscription>>>>,
    subscriptions: RefCell<Vec<Subscription>>,
    facts: Cell<Option<NativeWindowFacts>>,
    queued_properties: Cell<Option<NativeWindowFacts>>,
    observations: RefCell<NativeWindowObservationOutbox>,
    close_requested: Cell<bool>,
    close_sent: Cell<bool>,
    logical_closed: Cell<bool>,
    native_terminal: Cell<bool>,
    sealed: Cell<bool>,
    anchor: Cell<bool>,
    root_detached: Cell<bool>,
    capture: Cell<Option<NativeCapture>>,
    capture_start: Cell<Option<NativeCapture>>,
    drag_tear_off: Cell<Option<(NativeCapture, NativeDragTearOff)>>,
    capture_releases: RefCell<Vec<capture_driver::CaptureRelease>>,
    capture_failed: Cell<bool>,
    capture_cancellations: RefCell<VecDeque<NativeCapture>>,
    capture_observation_missing: Cell<bool>,
    capture_ingress: Cell<u64>,
    placement_ready: Cell<bool>,
    cleanup: RefCell<Vec<(NativeSurfaceBinding, NativeCleanupObservation)>>,
    open_retirement: RefCell<Option<WindowPresentationShutdownTicket>>,
    create_placement: RefCell<Option<WindowMutationDispatch>>,
    provisional_placement: RefCell<Option<WindowProvisionalPlacementTicket>>,
}

/// Callback custody for one physical lifetime; it never borrows `SharedHost`.
#[derive(Clone)]
pub struct NativeCallbacks(Rc<CallbackState>);

pub(in crate::adapter) struct CallbackCustody(Weak<CallbackState>);
