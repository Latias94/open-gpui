use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::{Rc, Weak},
};

use dockspace::{
    geometry::{PhysicalRect, PhysicalSize, ScaleFactor},
    runtime::{
        DockspaceSession, NativeCleanupObservation, NativeCloseDisposition,
        NativeCloseEffectAcknowledgement, NativeCloseState, NativeCreateEffectAcknowledgement,
        NativeCreateReadiness, NativeDispatchFailure, NativeEffectAcknowledgement,
        NativeEffectOperation, NativeEffectRequest, NativeGlobalFocus, NativeHostCapability,
        NativeIndeterminateReason, NativeInputEffectAcknowledgement, NativeSurfaceBinding,
        NativeSurfaceRole, NativeUnsupportedReason, NativeWindowBinding, NativeWindowFacts,
        NativeWindowInputState, NativeWindowPresentationState,
    },
};
use open_gpui::{
    AnyWindowHandle, App, Entity, PlatformWindowActivationSupport, Render, Subscription, Window,
    WindowActivationTerminal, WindowActivationTicket, WindowFrameSubmissionTicket,
    WindowInitialPresentationStatus, WindowMutationDispatch, WindowMutationOutcome,
    WindowMutationTicket, WindowOpenError, WindowOpenFailureStage, WindowOptions,
    WindowPhysicalPlacementRequest, WindowPlatformFacts, WindowPresentationShutdownTicket,
};

use super::{
    NativeBridge, NativeBridgeError, NativeReport, NativeTransport, NativeWindowFactory,
    input::NativeCapture, transport::Inbound,
};

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
            |outer, window| {
                window.physical_placement_for_outer_bounds(super::input::device_bounds(outer)?)
            },
        )
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
    window: Cell<Option<AnyWindowHandle>>,
    transport: NativeTransport,
    geometry: Option<Rc<NativeGeometry>>,
    subscriptions: RefCell<Vec<Subscription>>,
    facts: Cell<Option<NativeWindowFacts>>,
    observed: Cell<Option<NativeWindowFacts>>,
    create: Cell<Option<NativeCreateEffectAcknowledgement>>,
    close: Cell<Option<NativeCloseEffectAcknowledgement>>,
    input: Cell<Option<NativeInputEffectAcknowledgement>>,
    close_requested: Cell<bool>,
    close_sent: Cell<bool>,
    logical_closed: Cell<bool>,
    native_terminal: Cell<bool>,
    sealed: Cell<bool>,
    anchor: Cell<bool>,
    capture: Cell<Option<NativeCapture>>,
    capture_release: Cell<Option<open_gpui::NativeCapturedDragReleaseBarrier>>,
    capture_failed: Cell<bool>,
    capture_cancellations: RefCell<VecDeque<NativeCapture>>,
    capture_observation_missing: Cell<bool>,
    capture_ingress: Cell<u64>,
    placement_ready: Cell<bool>,
    cleanup: RefCell<Vec<(NativeSurfaceBinding, NativeCleanupObservation)>>,
    open_retirement: RefCell<Option<WindowPresentationShutdownTicket>>,
}

/// Callback custody for one physical lifetime; it never borrows `SharedHost`.
#[derive(Clone)]
pub struct NativeCallbacks(Rc<CallbackState>);

pub(super) struct SubmissionCustody(Weak<CallbackState>);

impl NativeCallbacks {
    pub(super) fn new(
        binding: NativeWindowBinding,
        transport: NativeTransport,
        geometry: Option<Rc<NativeGeometry>>,
    ) -> Self {
        Self(Rc::new(CallbackState {
            binding: Cell::new(binding),
            window: Cell::new(None),
            transport,
            geometry,
            subscriptions: RefCell::new(Vec::new()),
            facts: Cell::new(None),
            observed: Cell::new(None),
            create: Cell::new(None),
            close: Cell::new(None),
            input: Cell::new(None),
            close_requested: Cell::new(false),
            close_sent: Cell::new(false),
            logical_closed: Cell::new(false),
            native_terminal: Cell::new(false),
            sealed: Cell::new(false),
            anchor: Cell::new(false),
            capture: Cell::new(None),
            capture_release: Cell::new(None),
            capture_failed: Cell::new(false),
            capture_cancellations: RefCell::new(VecDeque::new()),
            capture_observation_missing: Cell::new(false),
            capture_ingress: Cell::new(0),
            placement_ready: Cell::new(false),
            cleanup: RefCell::new(Vec::new()),
            open_retirement: RefCell::new(None),
        }))
    }

    /// Opens an ordinary GPUI-owned native window with early lifecycle interception.
    ///
    /// `build_root` mounts the original retained Views through the caller's surface.
    /// Do not derive `transient_for` from the anchor: it is a lifetime peer window.
    pub fn open<V: Render + 'static>(
        &self,
        options: WindowOptions,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
        cx: &mut App,
    ) -> Result<AnyWindowHandle, NativeOpenFailure> {
        let callbacks = self.clone();
        let window = cx.open_window_detailed(options, move |window, cx| {
            if callbacks.install(window, cx).is_err() {
                window.remove_window(cx);
            }
            build_root(window, cx)
        })?;
        Ok(window.into())
    }

    /// Installs before initial drawing for a window whose OS lifetime GPUI owns.
    ///
    /// Application-owned external attachments require their owning adapter to
    /// distinguish binding detachment from OS destruction; this entry point is
    /// deliberately for ordinary GPUI-created native windows only.
    pub fn install(&self, window: &mut Window, cx: &mut App) -> Result<(), NativeBridgeError> {
        let handle = window.window_handle();
        if let Some(existing) = self.0.window.get() {
            return if existing.window_id() == handle.window_id() {
                Ok(())
            } else {
                Err(NativeBridgeError::WindowAlreadyBound)
            };
        }
        if !window.presentation_facts().native_created {
            return Err(NativeBridgeError::UnknownBinding);
        }
        self.0.window.set(Some(handle));
        let weak = Rc::downgrade(&self.0);
        window.on_window_should_close(cx, move |_, cx| {
            let Some(state) = weak.upgrade() else {
                return false;
            };
            if state.sealed.get() {
                return false;
            }
            if state.anchor.get() {
                state.transport.freeze(cx);
            } else if !state.close_requested.replace(true) {
                state.transport.push(
                    Inbound::Close(state.binding.get(), NativeCloseState::Requested, None),
                    cx,
                );
            }
            false
        });
        let weak = Rc::downgrade(&self.0);
        let logical = cx.on_window_closed(move |cx, window_id| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if state.window.get().map(|handle| handle.window_id()) != Some(window_id)
                || state.sealed.get()
            {
                return;
            }
            state.logical_closed.set(true);
            if state.anchor.get() {
                state.transport.freeze(cx);
            }
            state
                .transport
                .push(Inbound::LogicalClosed(state.binding.get()), cx);
        });
        let weak = Rc::downgrade(&self.0);
        let terminal = cx.on_window_native_terminal(move |cx, window_id| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if state.window.get().map(|handle| handle.window_id()) != Some(window_id)
                || state.sealed.get()
            {
                return;
            }
            state.native_terminal.set(true);
            state.capture.set(None);
            state.capture_release.set(None);
            state.capture_failed.set(false);
            state.capture_cancellations.borrow_mut().clear();
            if state.anchor.get() {
                state.transport.freeze(cx);
            }
            let facts = state.close.take().map_or_else(
                NativeWindowFacts::destroyed,
                NativeWindowFacts::destroyed_after,
            );
            state
                .transport
                .push(Inbound::NativeTerminal(state.binding.get(), facts), cx);
        });
        let weak = Rc::downgrade(&self.0);
        let initial = window.observe_window_initial_presentation(move |window, cx| {
            if let Some(state) = weak.upgrade() {
                NativeCallbacks(state).observe(window, cx);
            }
        });
        self.0
            .subscriptions
            .borrow_mut()
            .extend([logical, terminal, initial]);
        self.observe(window, cx);
        Ok(())
    }

    /// Captures current platform facts. Call from bounds/DPI/visibility observations.
    ///
    /// Renderer preparation uses actual native visibility, positive client extent
    /// and GPUI's completed initial presentation. First-live still needs U5's exact
    /// semantic output and original renderer submission ticket.
    pub fn observe(&self, window: &Window, cx: &mut App) {
        if self.0.sealed.get()
            || self.0.logical_closed.get()
            || self.window_id() != Some(window.window_handle().window_id())
        {
            return;
        }
        if let Some(facts) = self.window_facts(window) {
            if self.0.observed.replace(Some(facts)) == Some(facts) {
                return;
            }
            self.0
                .transport
                .push(Inbound::Observed(self.binding(), facts), cx);
        }
    }

    /// Observes U5's existing ticket, preserving its real window and frame authority.
    pub fn watch_submission(
        &self,
        ticket: &WindowFrameSubmissionTicket,
        window: &mut Window,
    ) -> Result<(), NativeBridgeError> {
        if self.window_id() != Some(ticket.snapshot().window_id()) || self.0.sealed.get() {
            return Err(NativeBridgeError::UnknownBinding);
        }
        let binding = self.binding();
        let weak = Rc::downgrade(&self.0);
        window.on_exact_frame_submission_presented(ticket, move |snapshot, _, cx| {
            if let Some(state) = weak.upgrade()
                && !state.sealed.get()
            {
                state.transport.push(
                    Inbound::Submission(binding, snapshot, SubmissionCustody(weak)),
                    cx,
                );
            }
        });
        Ok(())
    }

    /// Stops future delivery only after the actual native terminal and capture boundary.
    /// U5 must separately retire its original submission callbacks and output owners.
    pub fn seal(&self) -> Result<(), NativeBridgeError> {
        if !self.0.native_terminal.get()
            || self.0.capture_release.get().is_some()
            || self.0.capture_failed.get()
            || self
                .0
                .open_retirement
                .borrow()
                .as_ref()
                .is_some_and(|ticket| !ticket.snapshot().quiesced())
        {
            return Err(NativeBridgeError::NotQuiescent);
        }
        self.0.sealed.set(true);
        self.0.subscriptions.borrow_mut().clear();
        Ok(())
    }

    /// Returns the exact current callback route, never a surface-id lookup.
    pub fn binding(&self) -> NativeWindowBinding {
        self.0.binding.get()
    }
    pub(super) fn has_submission_custody(&self, original: &SubmissionCustody) -> bool {
        Rc::downgrade(&self.0).ptr_eq(&original.0)
    }
    /// Returns the full GPUI window id reserved during root construction.
    pub fn window_id(&self) -> Option<open_gpui::WindowId> {
        self.0
            .window
            .get()
            .map(|window| window.window_id())
            .or_else(|| {
                self.0
                    .open_retirement
                    .borrow()
                    .as_ref()
                    .map(|ticket| ticket.snapshot().window_id())
            })
    }

    pub(crate) fn retain_open_retirement(
        &self,
        ticket: WindowPresentationShutdownTicket,
        cx: &mut App,
    ) {
        // Even a failure before root construction can own a real native window.
        // Keep GPUI's original shutdown authority; never mint a receipt for it.
        let owner = ticket.snapshot().window_id();
        *self.0.open_retirement.borrow_mut() = Some(ticket);
        if self.0.window.get().is_none() {
            let weak = Rc::downgrade(&self.0);
            let subscription = cx.on_window_native_terminal(move |cx, terminal| {
                if terminal == owner
                    && let Some(state) = weak.upgrade()
                {
                    NativeCallbacks(state).observe_open_retirement(cx);
                }
            });
            self.0.subscriptions.borrow_mut().push(subscription);
        }
        self.observe_open_retirement(cx);
    }

    fn observe_open_retirement(&self, cx: &mut App) {
        let snapshot = self
            .0
            .open_retirement
            .borrow()
            .as_ref()
            .map(WindowPresentationShutdownTicket::snapshot);
        if snapshot.is_some_and(|snapshot| snapshot.native_terminal())
            && !self.0.native_terminal.replace(true)
        {
            self.0.transport.push(
                Inbound::NativeTerminal(self.binding(), NativeWindowFacts::destroyed()),
                cx,
            );
        }
    }

    pub(super) fn correlate_cleanup(
        &self,
        mut result: dockspace::runtime::NativeEffectResult,
    ) -> dockspace::runtime::NativeEffectResult {
        let mut observations = std::mem::take(&mut *self.0.cleanup.borrow_mut()).into_iter();
        let mut unmatched = Vec::new();
        while let Some((binding, observation)) = observations.next() {
            match observation.correlate(result) {
                Ok(correlated) => {
                    result = correlated;
                    unmatched.extend(observations);
                    break;
                }
                Err(error) => {
                    let (observation, predecessor) = error.into_parts();
                    unmatched.push((binding, observation));
                    result = predecessor;
                }
            }
        }
        self.0.cleanup.borrow_mut().extend(unmatched);
        result
    }

    pub(super) fn handle(&self) -> Option<AnyWindowHandle> {
        self.0.window.get()
    }
    pub(super) fn facts(&self) -> Option<NativeWindowFacts> {
        self.0.facts.get()
    }
    pub(super) fn set_facts(&self, facts: NativeWindowFacts) {
        self.0.facts.set(Some(facts));
    }
    pub(super) fn set_logical_closed(&self) {
        self.0.logical_closed.set(true);
    }
    pub(super) fn set_native_terminal(&self) {
        self.0.native_terminal.set(true);
    }
    pub(super) fn set_anchor(&self) {
        self.0.anchor.set(true);
    }
    pub(super) fn allow_preparation(&self) {
        self.0.placement_ready.set(true);
    }
    pub(super) fn continue_root(&self, binding: NativeWindowBinding) {
        self.0.binding.set(binding);
        // Continuation retains this exact physical window. Its independent
        // geometry and effect proofs must refresh, but inventory remains known.
        self.0.facts.set(Some(if self.0.native_terminal.get() {
            NativeWindowFacts::destroyed()
        } else {
            NativeWindowFacts::live()
        }));
        self.0.observed.set(None);
        self.0.create.set(None);
        self.0.close.set(None);
        self.0.input.set(None);
    }

    pub(super) fn can_accept_capture_event(
        &self,
        event: &open_gpui::NativeCapturedDragEvent,
    ) -> bool {
        if self.0.sealed.get()
            || self.0.logical_closed.get()
            || self.window_id() != Some(event.source_window())
            || event.sequence().ordinal() <= self.0.capture_ingress.get()
        {
            return false;
        }
        let capture = NativeCapture::from_event(event);
        if self
            .0
            .capture
            .get()
            .is_some_and(|active| active.generation.ordinal() > capture.generation.ordinal())
        {
            return false;
        }
        true
    }

    pub(super) fn accept_capture_event(&self, event: &open_gpui::NativeCapturedDragEvent) {
        let capture = NativeCapture::from_event(event);
        if self.0.capture.get().map(|previous| previous.generation) != Some(capture.generation) {
            self.0.capture_observation_missing.set(false);
        }
        self.0.capture_ingress.set(event.sequence().ordinal());
        self.0.capture.set(Some(capture));
    }

    pub(super) fn request_capture_cancellation(&self, capture: NativeCapture) {
        if self.0.native_terminal.get()
            || self.0.capture_release.get().is_some_and(|barrier| {
                barrier.source_window() == capture.source
                    && barrier.drag_generation() == capture.generation
            })
        {
            return;
        }
        let mut pending = self.0.capture_cancellations.borrow_mut();
        if pending.back() != Some(&capture) {
            pending.push_back(capture);
        }
    }

    pub(super) fn needs_capture_observation_report(&self) -> bool {
        !self.0.capture_observation_missing.replace(true)
    }

    pub(super) fn finish_capture(
        &self,
        barrier: open_gpui::NativeCapturedDragReleaseBarrier,
        terminal: open_gpui::NativeCapturedDragReleaseTerminal,
    ) {
        if self.0.capture_release.get() != Some(barrier) {
            return;
        }
        self.0.capture_release.set(None);
        if terminal == open_gpui::NativeCapturedDragReleaseTerminal::Failed {
            self.0.capture_failed.set(true);
        } else {
            if self.0.capture.get().is_some_and(|capture| {
                capture.source == barrier.source_window()
                    && capture.generation == barrier.drag_generation()
            }) {
                self.0.capture.set(None);
            }
            self.0.capture_failed.set(false);
        }
    }

    /// Cancels the original GPUI native capture and retains its exact release barrier.
    pub fn cancel_capture(&self, cx: &mut App) {
        if let Some(capture) = self.0.capture.get() {
            self.request_capture_cancellation(capture);
        }
        self.dispatch_capture_cancellations(cx);
    }

    fn dispatch_capture_cancellations(&self, cx: &mut App) {
        if self.0.native_terminal.get() {
            self.0.capture_cancellations.borrow_mut().clear();
            return;
        }
        let pending = self.0.capture_cancellations.borrow().len();
        for _ in 0..pending {
            // A later generation cannot replace the original in-flight barrier.
            // Its request remains queued until that exact release completes.
            if self.0.capture_release.get().is_some() {
                break;
            }
            let Some(capture) = self.0.capture_cancellations.borrow_mut().pop_front() else {
                break;
            };
            self.cancel_capture_identity(capture, cx);
        }
    }

    fn cancel_capture_identity(&self, capture: NativeCapture, cx: &mut App) {
        let weak = Rc::downgrade(&self.0);
        let binding = self.binding();
        let barrier = cx.cancel_native_captured_drag_with_release_barrier(
            capture.source,
            capture.generation,
            open_gpui::PointerCancelReason::CaptureRevoked,
            move |barrier, terminal, cx| {
                if let Some(state) = weak.upgrade() {
                    state
                        .transport
                        .push(Inbound::CaptureTerminal(binding, barrier, terminal), cx);
                }
            },
        );
        if let Some(barrier) = barrier {
            self.0.capture_release.set(Some(barrier));
        } else {
            self.0.capture_failed.set(true);
            self.0
                .transport
                .push(Inbound::CaptureUnobserved(self.binding()), cx);
        }
    }

    fn window_facts(&self, window: &Window) -> Option<NativeWindowFacts> {
        let platform = window.platform_facts();
        let presentation = window.presentation_facts();
        if !presentation.native_created {
            return None;
        }
        let scale = ScaleFactor::new(f64::from(platform.scale_factor)).ok()?;
        let presentation_scale = ScaleFactor::new(f64::from(window.scale_factor())).ok()?;
        let size = if let Some(geometry) = platform.physical_geometry {
            PhysicalSize::new(
                f64::from(geometry.client_bounds().size.width.0),
                f64::from(geometry.client_bounds().size.height.0),
            )
            .ok()?
        } else {
            PhysicalSize::new(
                f64::from(f32::from(platform.content_size.width))
                    * f64::from(platform.scale_factor),
                f64::from(f32::from(platform.content_size.height))
                    * f64::from(platform.scale_factor),
            )
            .ok()?
        };
        let visible = presentation.native_visible && !platform.is_minimized;
        let prepared = self.0.placement_ready.get()
            && visible
            && size.width() > 0.0
            && size.height() > 0.0
            && presentation.initial_presentation == WindowInitialPresentationStatus::Completed;
        let mut facts = NativeWindowFacts::live()
            .with_content_size(size)
            .with_native_scale_factor(scale)
            .with_presentation_scale_factor(presentation_scale)
            .with_input(
                if platform.accepts_pointer_input {
                    NativeWindowInputState::ReceivesInput
                } else {
                    NativeWindowInputState::PassThrough
                },
                self.0.input.take(),
            )
            .with_presentation(if platform.is_minimized {
                NativeWindowPresentationState::Minimized
            } else if presentation.native_visible {
                NativeWindowPresentationState::Visible
            } else {
                NativeWindowPresentationState::Hidden
            })
            .with_create_readiness(
                if prepared {
                    NativeCreateReadiness::Prepared
                } else {
                    NativeCreateReadiness::Created
                },
                self.0.create.take(),
            )
            .with_close(
                if self.0.close_requested.get() {
                    NativeCloseState::Requested
                } else {
                    NativeCloseState::Clear
                },
                None,
            );
        if let Some(geometry) = platform.physical_geometry
            && let Some(bounds) = super::input::physical_rect(geometry.client_bounds())
        {
            facts = facts.with_content_bounds(bounds);
        }
        if let Some(geometry) = self.0.geometry.as_ref()
            && let Some(bounds) = (geometry.outer_bounds)(platform)
        {
            facts = facts.with_outer_bounds(bounds);
        }
        Some(facts)
    }
}

pub(super) struct PendingDispatch {
    request: NativeEffectRequest,
    callbacks: NativeCallbacks,
    ticket: PendingTicket,
    create: bool,
    _subscription: Subscription,
}

enum PendingTicket {
    Mutation(WindowMutationTicket),
    Focus(WindowActivationTicket),
}

impl NativeBridge {
    pub(crate) fn preflight_root_binding(
        &self,
        window: &Window,
        anchor: bool,
        cx: &App,
    ) -> Result<(), NativeBridgeError> {
        if self.draining || self.transport.is_frozen() {
            return Err(NativeBridgeError::Draining);
        }
        if self.opening.is_some() {
            return Err(NativeBridgeError::NotQuiescent);
        }
        let owner = window.window_handle().window_id();
        if (anchor && self.anchor.is_some()) || self.binding_for_window(owner).is_some() {
            return Err(NativeBridgeError::WindowAlreadyBound);
        }
        if !window.presentation_facts().native_created {
            return Err(NativeBridgeError::UnknownBinding);
        }
        let anchor = if anchor {
            Some(owner)
        } else {
            self.anchor
                .and_then(|binding| self.windows.get(binding))
                .and_then(NativeCallbacks::window_id)
        };
        if let Some(anchor) = anchor {
            let dependencies = self
                .windows
                .iter()
                .map(|(_, window, _)| window)
                .chain(std::iter::once(owner))
                .filter(|dependency| *dependency != anchor);
            cx.register_native_window_retirement_dependencies(anchor, dependencies)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    pub(crate) fn retain_root_rollback(
        &mut self,
        binding: NativeWindowBinding,
        ticket: WindowPresentationShutdownTicket,
        anchor: bool,
        cx: &mut App,
    ) -> Result<NativeCallbacks, NativeBridgeError> {
        if self.opening.is_some() {
            return Err(NativeBridgeError::NotQuiescent);
        }
        let owner = ticket.snapshot().window_id();
        let callbacks = if let Some(callbacks) = self.windows.get(binding) {
            if callbacks.window_id() != Some(owner) {
                return Err(NativeBridgeError::WindowAlreadyBound);
            }
            callbacks.clone()
        } else {
            let callbacks =
                NativeCallbacks::new(binding, self.transport.clone(), self.geometry.clone());
            self.windows
                .insert(binding, owner, callbacks.clone())
                .map_err(|_| NativeBridgeError::WindowAlreadyBound)?;
            callbacks
        };
        if anchor && self.anchor.is_none() {
            callbacks.set_anchor();
            self.anchor = Some(binding);
        }
        callbacks.retain_open_retirement(ticket, cx);
        self.refresh_dependencies(cx)?;
        Ok(callbacks)
    }

    pub(crate) fn retain_root_retirement_dependencies(
        &self,
        retirements: impl IntoIterator<Item = open_gpui::WindowId>,
        cx: &App,
    ) -> Result<(), NativeBridgeError> {
        if let Some(anchor) = self
            .anchor
            .and_then(|binding| self.windows.get(binding))
            .and_then(NativeCallbacks::window_id)
        {
            cx.register_native_window_retirement_dependencies(anchor, retirements)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    /// Dispatches the exact core FIFO outside `SharedHost` and entity borrows.
    ///
    /// A queued placement or pointer-input mutation retains the original affine
    /// request and blocks subsequent effects until its real observation arrives.
    /// Invoke again on the owning mutation wake; no timer implies success.
    pub fn dispatch_effects(
        &mut self,
        factory: &mut NativeWindowFactory<'_>,
        cx: &mut App,
    ) -> Result<usize, NativeBridgeError> {
        if self.opening.is_some() {
            self.register_opening()?;
            self.refresh_dependencies(cx)?;
        }
        for (_, _, callbacks) in self.windows.iter() {
            callbacks.observe_open_retirement(cx);
            callbacks.dispatch_capture_cancellations(cx);
        }
        for (_, _, callbacks) in self
            .windows
            .iter()
            .filter(|(_, _, callbacks)| callbacks.0.close.get().is_some())
        {
            self.remove_window(callbacks, cx);
        }
        if !self.finish_pending(cx)? {
            return Ok(0);
        }
        let mut dispatched = 0;
        while let Some(request) = self.effects.pop_front() {
            let operation = request.operation().clone();
            if (self.draining || self.transport.is_frozen())
                && !matches!(
                    operation,
                    NativeEffectOperation::CompensatingClose { .. }
                        | NativeEffectOperation::ReleaseChild { .. }
                        | NativeEffectOperation::AwaitCleanup { .. }
                        | NativeEffectOperation::ResolveNativeClose {
                            resolution: NativeCloseDisposition::Accept,
                            ..
                        }
                )
            {
                self.transport.push(
                    Inbound::Effect(
                        request.dispatch_failed(NativeDispatchFailure::ProviderStopped),
                    ),
                    cx,
                );
                dispatched += 1;
                continue;
            }
            match operation {
                NativeEffectOperation::CreateWindow {
                    binding,
                    placement,
                    role,
                }
                | NativeEffectOperation::RequestReplacement {
                    binding,
                    placement,
                    role,
                } => {
                    if self.geometry.is_none()
                        || !self
                            .capabilities
                            .supports(NativeHostCapability::GlobalWindowPlacement)
                    {
                        self.transport.push(
                            Inbound::Effect(
                                request.unsupported(NativeUnsupportedReason::BackendUnsupported),
                            ),
                            cx,
                        );
                    } else {
                        self.create_window(
                            request,
                            NativeWindowRequest {
                                binding,
                                outer_bounds: placement,
                                role,
                            },
                            factory,
                            cx,
                        )?;
                    }
                }
                NativeEffectOperation::CompensatingClose { binding }
                | NativeEffectOperation::ReleaseChild { binding } => {
                    self.close_effect(request, binding, cx)
                }
                NativeEffectOperation::ResolveNativeClose {
                    close,
                    resolution: NativeCloseDisposition::Accept,
                } => self.close_effect(request, close.binding(), cx),
                NativeEffectOperation::CancelRootClose { binding } => {
                    self.cancel_close_effect(request, binding, cx)
                }
                NativeEffectOperation::ResolveNativeClose {
                    close,
                    resolution: NativeCloseDisposition::Cancel,
                } => self.cancel_close_effect(request, close.binding(), cx),
                NativeEffectOperation::RetainChild { binding } => {
                    if self.windows.get(binding.window()).is_some() {
                        let _ = request.begin_dispatch().accepted();
                    } else {
                        self.transport.push(
                            Inbound::Effect(
                                request.dispatch_failed(NativeDispatchFailure::WindowUnavailable),
                            ),
                            cx,
                        );
                    }
                }
                NativeEffectOperation::AwaitCleanup { binding } => {
                    if let Some(callbacks) = self.windows.get(binding.window()) {
                        if let Some(NativeEffectAcknowledgement::Cleanup(observation)) =
                            request.begin_dispatch().accepted()
                        {
                            callbacks
                                .0
                                .cleanup
                                .borrow_mut()
                                .push((binding, observation));
                        }
                    } else {
                        self.transport.push(
                            Inbound::Effect(
                                request.dispatch_failed(NativeDispatchFailure::WindowUnavailable),
                            ),
                            cx,
                        );
                    }
                }
                NativeEffectOperation::SetPointerPassthrough { binding, enabled } => {
                    self.pointer_input_effect(request, binding, enabled, cx)
                }
                NativeEffectOperation::RequestFocus { binding } => {
                    self.focus_effect(request, binding, cx)
                }
                _ => self.transport.push(
                    Inbound::Effect(
                        request.unsupported(NativeUnsupportedReason::BackendUnsupported),
                    ),
                    cx,
                ),
            }
            dispatched += 1;
            if self.pending.is_some() {
                break;
            }
        }
        Ok(dispatched)
    }

    fn create_window(
        &mut self,
        request: NativeEffectRequest,
        open: NativeWindowRequest,
        factory: &mut NativeWindowFactory<'_>,
        cx: &mut App,
    ) -> Result<(), NativeBridgeError> {
        if self.windows.get(open.binding.window()).is_some() {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::AdapterRejected)),
                cx,
            );
            return Ok(());
        }
        let callbacks = NativeCallbacks::new(
            open.binding.window(),
            self.transport.clone(),
            self.geometry.clone(),
        );
        self.opening = Some(callbacks.clone());
        let dispatch = request.begin_dispatch();
        match factory(open, &callbacks, cx) {
            Err(error) => {
                let retirement_ticket = error.retirement_ticket.clone();
                if let Some(ticket) = retirement_ticket.as_ref() {
                    callbacks.retain_open_retirement(ticket.clone(), cx);
                }
                let owns_native = retirement_ticket.is_some() || callbacks.window_id().is_some();
                let result = if owns_native {
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost)
                } else {
                    dispatch.not_applied(NativeDispatchFailure::AdapterRejected)
                };
                self.transport.push(Inbound::Effect(result), cx);
                self.reports.push_back(NativeReport::OpenFailed {
                    binding: open.binding,
                    stage: error.stage,
                    message: error.message,
                    retirement_ticket,
                });
                if !owns_native {
                    self.opening = None;
                } else {
                    self.register_opening()?;
                    self.refresh_dependencies(cx)?;
                }
            }
            Ok(handle) => {
                let registered = self
                    .register_opening()
                    .and_then(|()| self.refresh_dependencies(cx));
                if let Err(error) = registered {
                    self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    );
                    return Err(error);
                }
                if callbacks.handle() != Some(handle) {
                    self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    );
                    return Err(NativeBridgeError::UnknownBinding);
                }
                let placement = handle.update(cx, |_, window, _| {
                    self.geometry
                        .as_ref()
                        .and_then(|geometry| (geometry.client_placement)(open.outer_bounds, window))
                        .map(|request| window.request_physical_window_placement(request))
                });
                match placement {
                    Ok(Some(WindowMutationDispatch::Queued(ticket))) => {
                        let wake = self.transport.wake_after_callback(cx);
                        let subscription = ticket.subscribe(move |_| wake());
                        self.pending = Some(PendingDispatch {
                            request: dispatch.await_observation(),
                            callbacks,
                            ticket: PendingTicket::Mutation(ticket),
                            create: true,
                            _subscription: subscription,
                        });
                    }
                    Ok(Some(WindowMutationDispatch::Unchanged)) => {
                        if let Some(NativeEffectAcknowledgement::Create(acknowledgement)) =
                            dispatch.accepted()
                        {
                            callbacks.0.create.set(Some(acknowledgement));
                        }
                        callbacks.allow_preparation();
                        let _ = handle.update(cx, |_, window, cx| callbacks.observe(window, cx));
                    }
                    Ok(None) | Ok(Some(WindowMutationDispatch::Unsupported)) => {
                        self.reports.push_back(NativeReport::Unsupported {
                            operation: "native outer-frame to client placement conversion",
                        });
                        // The factory already created a native owner. Failure
                        // to place it cannot be reported as not-applied.
                        self.transport.push(
                            Inbound::Effect(
                                dispatch
                                    .indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                            ),
                            cx,
                        );
                    }
                    _ => self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    ),
                }
            }
        }
        Ok(())
    }

    fn finish_pending(&mut self, cx: &mut App) -> Result<bool, NativeBridgeError> {
        let Some(pending) = self.pending.take() else {
            return Ok(true);
        };
        if pending.request.operation().binding().window() != pending.callbacks.binding()
            || pending.callbacks.0.logical_closed.get()
            || pending.callbacks.0.native_terminal.get()
        {
            self.transport.push(
                Inbound::Effect(
                    pending
                        .request
                        .indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
            return Ok(true);
        }
        let observation = match &pending.ticket {
            PendingTicket::Mutation(ticket) => ticket.observation(),
            PendingTicket::Focus(ticket) => {
                let Some(terminal) = ticket.snapshot().status().terminal() else {
                    self.pending = Some(pending);
                    return Ok(false);
                };
                let binding = pending.callbacks.binding();
                match terminal {
                    WindowActivationTerminal::Activated => {
                        let _ = pending.request.accepted();
                        self.transport
                            .push(Inbound::Focus(NativeGlobalFocus::Dock(binding)), cx);
                    }
                    WindowActivationTerminal::Unsupported => self.transport.push(
                        Inbound::Effect(
                            pending
                                .request
                                .unsupported(NativeUnsupportedReason::BackendUnsupported),
                        ),
                        cx,
                    ),
                    WindowActivationTerminal::Rejected => self.transport.push(
                        Inbound::Effect(
                            pending
                                .request
                                .not_applied(NativeDispatchFailure::AdapterRejected),
                        ),
                        cx,
                    ),
                    _ => self.transport.push(
                        Inbound::Effect(
                            pending
                                .request
                                .indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    ),
                }
                return Ok(true);
            }
        };
        let Some(observation) = observation else {
            self.pending = Some(pending);
            return Ok(false);
        };
        match observation.outcome {
            WindowMutationOutcome::Exact | WindowMutationOutcome::Adjusted => {
                match pending.request.accepted() {
                    Some(NativeEffectAcknowledgement::Create(ack)) => {
                        pending.callbacks.0.create.set(Some(ack))
                    }
                    Some(NativeEffectAcknowledgement::Input(ack)) => {
                        pending.callbacks.0.input.set(Some(ack))
                    }
                    _ => {}
                }
                if pending.create {
                    pending.callbacks.allow_preparation();
                }
                if let Some(handle) = pending.callbacks.handle() {
                    handle
                        .update(cx, |_, window, cx| pending.callbacks.observe(window, cx))
                        .map_err(|error| NativeBridgeError::WindowUpdate(error.to_string()))?;
                }
            }
            _ => {
                let result = if pending.create {
                    pending
                        .request
                        .indeterminate(NativeIndeterminateReason::AcknowledgementLost)
                } else if observation.outcome == WindowMutationOutcome::Unsupported {
                    pending
                        .request
                        .unsupported(NativeUnsupportedReason::BackendUnsupported)
                } else {
                    pending
                        .request
                        .not_applied(NativeDispatchFailure::AdapterRejected)
                };
                self.transport.push(Inbound::Effect(result), cx);
            }
        }
        Ok(true)
    }

    fn pointer_input_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        enabled: bool,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let Some(handle) = callbacks.handle() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let dispatch = request.begin_dispatch();
        match handle.update(cx, |_, window, _| window.request_pointer_input(!enabled)) {
            Ok(WindowMutationDispatch::Queued(ticket)) => {
                let wake = self.transport.wake_after_callback(cx);
                let subscription = ticket.subscribe(move |_| wake());
                self.pending = Some(PendingDispatch {
                    request: dispatch.await_observation(),
                    callbacks,
                    ticket: PendingTicket::Mutation(ticket),
                    create: false,
                    _subscription: subscription,
                });
            }
            Ok(WindowMutationDispatch::Unchanged) => {
                if let Some(NativeEffectAcknowledgement::Input(ack)) = dispatch.accepted() {
                    callbacks.0.input.set(Some(ack));
                }
                let _ = handle.update(cx, |_, window, cx| callbacks.observe(window, cx));
            }
            Ok(WindowMutationDispatch::Unsupported) => self.transport.push(
                Inbound::Effect(dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            ),
            Ok(WindowMutationDispatch::Rejected) => self.transport.push(
                Inbound::Effect(dispatch.not_applied(NativeDispatchFailure::AdapterRejected)),
                cx,
            ),
            _ => self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            ),
        }
    }

    fn focus_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let Some(handle) = callbacks.handle() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if !self
            .capabilities
            .supports(NativeHostCapability::WindowActivationControl)
        {
            self.transport.push(
                Inbound::Effect(request.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            );
            return;
        }
        let dispatch = request.begin_dispatch();
        match handle.update(cx, |_, window, _| {
            (window.window_capabilities().activation == PlatformWindowActivationSupport::Observed)
                .then(|| window.activate_window())
        }) {
            Ok(Some(ticket)) => {
                let wake = self.transport.wake_after_callback(cx);
                let subscription = ticket.subscribe(move |_| wake());
                self.pending = Some(PendingDispatch {
                    request: dispatch.await_observation(),
                    callbacks,
                    ticket: PendingTicket::Focus(ticket),
                    create: false,
                    _subscription: subscription,
                });
            }
            Ok(None) => self.transport.push(
                Inbound::Effect(dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            ),
            Err(_) => self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            ),
        }
    }

    fn cancel_close_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()) else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if callbacks.0.close_sent.get() || callbacks.0.logical_closed.get() {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        }
        if let Some(NativeEffectAcknowledgement::Close(ack)) = request.begin_dispatch().accepted() {
            callbacks.0.close_requested.set(false);
            self.transport.push(
                Inbound::Close(binding.window(), NativeCloseState::Clear, Some(ack)),
                cx,
            );
        }
    }

    fn close_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if callbacks.0.close_sent.get() {
            self.transport.push(
                Inbound::Effect(
                    request.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
            return;
        }
        if let Some(NativeEffectAcknowledgement::Close(ack)) = request.begin_dispatch().accepted() {
            callbacks.0.close.set(Some(ack));
        }
        self.remove_window(&callbacks, cx);
    }

    fn remove_window(&self, callbacks: &NativeCallbacks, cx: &mut App) {
        if callbacks.0.logical_closed.get() || callbacks.0.native_terminal.get() {
            return;
        }
        callbacks.cancel_capture(cx);
        if let Some(handle) = callbacks.handle() {
            if handle
                .update(cx, |_, window, cx| window.remove_window(cx))
                .is_ok()
            {
                callbacks.0.close_sent.set(true);
            }
        }
    }

    pub(super) fn refresh_dependencies(&self, cx: &App) -> Result<(), NativeBridgeError> {
        if let Some(anchor) = self
            .anchor
            .and_then(|binding| self.windows.get(binding))
            .and_then(NativeCallbacks::window_id)
        {
            let dependents = self
                .windows
                .iter()
                .filter(|(binding, _, _)| Some(*binding) != self.anchor)
                .map(|(_, window, _)| window);
            cx.register_native_window_retirement_dependencies(anchor, dependents)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    /// Freezes this provider and starts forced dependent-first retirement.
    pub fn begin_shutdown(&mut self, cx: &mut App) -> Result<(), NativeBridgeError> {
        self.transport.freeze(cx);
        self.draining = true;
        self.drive_shutdown(cx)
    }

    /// Retries close intents; a failed handle update never becomes a native terminal.
    pub fn drive_shutdown(&mut self, cx: &mut App) -> Result<(), NativeBridgeError> {
        if !self.draining && !self.transport.is_frozen() {
            return Ok(());
        }
        self.draining = true;
        for (binding, _, callbacks) in self.windows.iter() {
            if Some(binding) != self.anchor {
                self.remove_window(callbacks, cx);
            }
        }
        let dependents_terminal = self
            .windows
            .iter()
            .filter(|(binding, _, _)| Some(*binding) != self.anchor)
            .all(|(_, _, callbacks)| callbacks.0.native_terminal.get());
        if dependents_terminal
            && self.opening.is_none()
            && let Some(anchor) = self.anchor.and_then(|binding| self.windows.get(binding))
        {
            self.remove_window(anchor, cx);
        }
        Ok(())
    }

    /// Declares the caller's exact retired placement permanently free of U5 owners.
    ///
    /// The caller first retires output tickets, artifacts, routes and renderer
    /// sidecars. For a destroyed window, seal its callbacks after true terminal.
    /// Core ingress must commit before this permanent boundary can be recorded.
    pub fn quiesce_binding(
        &mut self,
        session: &mut DockspaceSession,
        binding: NativeSurfaceBinding,
    ) -> Result<(), NativeBridgeError> {
        if !self.transport.is_empty()
            || self.opening.is_some()
            || !self.reports.is_empty()
            || session.has_pending_native_input()
            || self
                .effects
                .iter()
                .any(|request| request.operation().binding().window() == binding.window())
            || self.pending.as_ref().is_some_and(|pending| {
                pending.request.operation().binding().window() == binding.window()
            })
        {
            return Err(NativeBridgeError::NotQuiescent);
        }
        if let Some(callbacks) = self.windows.get(binding.window())
            && (!callbacks.0.sealed.get() || !callbacks.0.native_terminal.get())
            && !self.retired_placements.contains(&binding)
        {
            return Err(NativeBridgeError::NotQuiescent);
        }
        session.report_native_binding_quiescence(binding)?;
        if let Some(callbacks) = self.windows.get(binding.window()) {
            callbacks
                .0
                .cleanup
                .borrow_mut()
                .retain(|(owner, _)| *owner != binding);
        }
        self.retired_placements.remove(&binding);
        if session
            .retained_native_surface_bindings_for_window(binding.window())
            .next()
            .is_none()
        {
            let _ = self.windows.remove(binding.window());
        }
        Ok(())
    }
}
