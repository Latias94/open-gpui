use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use dockspace::runtime::{
    DockspaceSession, NativeCloseEffectAcknowledgement, NativeCloseState, NativeEffectResult,
    NativeGlobalFocus, NativePointerId, NativePointerInput, NativePointerOwner,
    NativePointerRoster, NativeWindowBinding, NativeWindowFacts, NativeWorkAreaRoster,
    PointerEventMetadata,
};
use open_gpui::{
    App, NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal,
    PlatformDisplaySnapshot, WindowFrameSubmissionSnapshot,
};

use super::{NativeBridge, NativeBridgeError, NativeReport, gpui::CallbackCustody};

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

type Wake = dyn Fn(&mut App);

struct TransportState {
    queue: RefCell<VecDeque<Inbound>>,
    rejection: RefCell<Option<crate::adapter::DockHostFailure>>,
    wake: Rc<Wake>,
    scheduled: Cell<bool>,
    frozen: Cell<bool>,
    pending_callbacks: Cell<usize>,
}

/// UI-thread FIFO shared by native callbacks and the application's one host owner.
///
/// The wake runs in `App::defer`, outside native callbacks and host borrows. Its
/// closure should schedule the host pump through a weak owner. Rejected core
/// ingress retains the original front record and never allows a suffix past it.
#[derive(Clone)]
pub struct NativeTransport(Rc<TransportState>);

/// A copy of the original ingress queue's current diagnostic state.
///
/// Reading this observation does not pop, retry or acknowledge any record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTransportInspection {
    /// Records still owned by the callback FIFO.
    pub queued_record_count: usize,
    /// Diagnostic name of the retained front record, if any.
    pub front_record: Option<&'static str>,
    /// Original failure for the currently rejected front, including core categories.
    pub rejection: Option<crate::adapter::DockHostFailure>,
}

impl NativeTransport {
    /// Creates a callback queue with an application-owned wakeup.
    pub fn new(wake: impl Fn(&mut App) + 'static) -> Self {
        Self(Rc::new(TransportState {
            queue: RefCell::new(VecDeque::new()),
            rejection: RefCell::new(None),
            wake: Rc::new(wake),
            scheduled: Cell::new(false),
            frozen: Cell::new(false),
            pending_callbacks: Cell::new(0),
        }))
    }

    /// Records an already captured native pointer packet without changing its lane.
    pub fn pointer(&self, input: NativePointerInput, cx: &mut App) {
        self.push(Inbound::Pointer(input), cx);
    }

    pub(crate) fn pointer_in_window(&self, input: NativePointerInput) {
        self.0.queue.borrow_mut().push_back(Inbound::Pointer(input));
    }

    pub(in crate::adapter) fn presentation(
        &self,
        terminal: crate::adapter::presentation::PendingPresentation,
        cx: &mut App,
    ) {
        self.push(Inbound::Presentation(terminal), cx);
    }

    pub(in crate::adapter) fn take_presentation(
        &self,
    ) -> Option<crate::adapter::presentation::PendingPresentation> {
        let mut queue = self.0.queue.borrow_mut();
        if matches!(queue.front(), Some(Inbound::Presentation(_)))
            && let Some(Inbound::Presentation(terminal)) = queue.pop_front()
        {
            return Some(terminal);
        }
        None
    }

    pub(crate) fn captured_drag(
        &self,
        binding: NativeWindowBinding,
        capture: NativePointerOwner,
        event: open_gpui::NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
        cx: &mut App,
    ) {
        self.push(
            Inbound::CapturedDrag {
                binding,
                capture,
                event,
                pointer,
                metadata,
            },
            cx,
        );
    }

    pub(crate) fn wake(&self, cx: &mut App) {
        self.push(Inbound::Wake, cx);
    }

    pub(crate) fn freeze_admission(&self) {
        self.0.frozen.set(true);
    }

    /// Records an independently observed complete roster, including explicit Unknown.
    pub fn pointer_roster(&self, roster: NativePointerRoster, cx: &mut App) {
        self.push(Inbound::PointerRoster(roster), cx);
    }

    /// Records a real device-removal boundary without inventing release edges.
    pub fn device_removed(&self, cx: &mut App) {
        self.push(Inbound::DeviceRemoved, cx);
    }

    /// Publishes a complete independently observed work-area roster.
    pub fn work_areas(&self, areas: NativeWorkAreaRoster, cx: &mut App) {
        self.push(Inbound::WorkAreas(areas), cx);
    }

    pub(crate) fn desktop(&self, snapshot: &PlatformDisplaySnapshot, cx: &mut App) {
        self.push(
            Inbound::Desktop(super::desktop::DesktopObservation::from_snapshot(snapshot)),
            cx,
        );
    }

    /// Publishes an independently observed global focus fact.
    pub fn focus(&self, focus: NativeGlobalFocus, cx: &mut App) {
        self.push(Inbound::Focus(focus), cx);
    }

    pub(crate) fn focused_window(&self, focus: open_gpui::PlatformFocusedWindow, cx: &mut App) {
        self.push(Inbound::FocusedWindow(focus), cx);
    }

    /// Returns whether accepted callback work still needs collection.
    pub fn is_empty(&self) -> bool {
        self.0.queue.borrow().is_empty()
    }

    /// Copies the current FIFO head and its last rejection without driving progress.
    pub fn inspection(&self) -> NativeTransportInspection {
        let queue = self.0.queue.borrow();
        NativeTransportInspection {
            queued_record_count: queue.len(),
            front_record: queue.front().map(Inbound::diagnostic_name),
            rejection: self.0.rejection.borrow().clone(),
        }
    }

    /// Returns the synchronous group admission latch.
    pub fn is_frozen(&self) -> bool {
        self.0.frozen.get()
    }

    pub(super) fn freeze(&self, cx: &mut App) {
        if !self.0.frozen.replace(true) {
            self.push(Inbound::Freeze, cx);
        }
    }

    pub(super) fn push(&self, record: Inbound, cx: &mut App) {
        self.0.queue.borrow_mut().push_back(record);
        if !self.0.scheduled.replace(true) {
            let queue = Rc::downgrade(&self.0);
            cx.defer(move |cx| {
                if let Some(queue) = queue.upgrade() {
                    queue.scheduled.set(false);
                    (queue.wake)(cx);
                }
            });
        }
    }

    pub(super) fn wake_after_callback(&self, cx: &App) -> impl FnOnce() + use<> {
        let queue = self.clone();
        let pending = queue.retain_callback();
        let foreground = cx.foreground_executor().clone();
        let async_cx = cx.to_async();
        move || {
            foreground
                .spawn(async move {
                    async_cx.update(|cx| {
                        drop(pending);
                        queue.push(Inbound::Wake, cx);
                    });
                })
                .detach();
        }
    }

    pub(super) fn retain_callback(&self) -> PendingCallback {
        self.0
            .pending_callbacks
            .set(self.0.pending_callbacks.get() + 1);
        PendingCallback(self.clone())
    }

    pub(in crate::adapter) fn callbacks_idle(&self) -> bool {
        self.0.pending_callbacks.get() == 0 && !self.0.scheduled.get()
    }

    fn pop(&self) -> Option<Inbound> {
        self.0.rejection.borrow_mut().take();
        self.0.queue.borrow_mut().pop_front()
    }
    fn retry(&self, record: Inbound) {
        self.0.queue.borrow_mut().push_front(record);
    }

    fn reject(&self, record: Inbound, error: &NativeBridgeError) {
        self.retry(record);
        *self.0.rejection.borrow_mut() = Some(crate::adapter::DockHostFailure::from_error(error));
    }
}

pub(super) struct PendingCallback(NativeTransport);

impl Drop for PendingCallback {
    fn drop(&mut self) {
        self.0
            .0
            .pending_callbacks
            .set(self.0.0.pending_callbacks.get() - 1);
    }
}

pub(super) enum Inbound {
    Presentation(crate::adapter::presentation::PendingPresentation),
    Observed(NativeWindowBinding, NativeWindowFacts),
    Close(
        NativeWindowBinding,
        NativeCloseState,
        Option<NativeCloseEffectAcknowledgement>,
    ),
    LogicalClosed(NativeWindowBinding),
    NativeTerminal(NativeWindowBinding, NativeWindowFacts),
    Submission(
        NativeWindowBinding,
        WindowFrameSubmissionSnapshot,
        CallbackCustody,
    ),
    CaptureTerminal(
        NativeWindowBinding,
        NativeCapturedDragReleaseBarrier,
        NativeCapturedDragReleaseTerminal,
        CallbackCustody,
    ),
    CaptureUnobserved(NativeWindowBinding),
    Pointer(NativePointerInput),
    CapturedDrag {
        binding: NativeWindowBinding,
        capture: NativePointerOwner,
        event: open_gpui::NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
    },
    PointerRoster(NativePointerRoster),
    WorkAreas(NativeWorkAreaRoster),
    Desktop(super::desktop::DesktopObservation),
    Focus(NativeGlobalFocus),
    FocusedWindow(open_gpui::PlatformFocusedWindow),
    DeviceRemoved,
    Effect(NativeEffectResult),
    Freeze,
    Wake,
}

impl Inbound {
    fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::Presentation(_) => "presentation",
            Self::Observed(..) => "window-observation",
            Self::Close(..) => "close",
            Self::LogicalClosed(_) => "logical-close",
            Self::NativeTerminal(..) => "native-terminal",
            Self::Submission(..) => "submission",
            Self::CaptureTerminal(..) => "capture-terminal",
            Self::CaptureUnobserved(_) => "capture-unobserved",
            Self::Pointer(_) => "pointer",
            Self::CapturedDrag { .. } => "captured-drag",
            Self::PointerRoster(_) => "pointer-roster",
            Self::WorkAreas(_) | Self::Desktop(_) => "work-areas",
            Self::Focus(_) | Self::FocusedWindow(_) => "focus",
            Self::DeviceRemoved => "device-removed",
            Self::Effect(_) => "effect-result",
            Self::Freeze => "freeze",
            Self::Wake => "wake",
        }
    }

    fn binding(&self) -> Option<NativeWindowBinding> {
        match self {
            Self::Observed(binding, _)
            | Self::Close(binding, _, _)
            | Self::LogicalClosed(binding)
            | Self::NativeTerminal(binding, _)
            | Self::Submission(binding, _, _)
            | Self::CaptureTerminal(binding, _, _, _) => Some(*binding),
            Self::CaptureUnobserved(binding) => Some(*binding),
            Self::Effect(result) => Some(result.binding().window()),
            Self::CapturedDrag { binding, .. } => Some(*binding),
            _ => None,
        }
    }
}

impl NativeBridge {
    pub(in crate::adapter) fn service_failed_capture_terminals(&self) {
        // Read actual backend release proofs without removing or reordering any
        // semantic records. The failed generation can never resume collection.
        for record in self.transport.0.queue.borrow().iter() {
            if let Inbound::CaptureTerminal(binding, barrier, terminal, custody) = record
                && let Some(callbacks) = self
                    .windows
                    .binding(barrier.source_window())
                    .and_then(|current| self.windows.get(current))
                && callbacks.has_callback_custody(custody)
            {
                callbacks.finish_capture(*binding, *barrier, *terminal);
            }
        }
    }

    /// Records the successful callback prefix, retaining the exact rejected front.
    ///
    /// This does not begin or commit a host frame. InputCapacity and pending-roster
    /// failures need the host to commit its accepted native prefix before retrying.
    /// Platform focus mapping also waits for earlier recorded input to commit so
    /// visibility and lifecycle changes determine whether known focus is valid.
    pub fn collect(&mut self, session: &mut DockspaceSession) -> Result<usize, NativeBridgeError> {
        if self.desktop_pending_commit {
            if session.has_pending_native_input() {
                return Ok(0);
            }
            self.desktop_pending_commit = false;
        }
        let mut collected = 0;
        while let Some(record) = self.transport.pop() {
            let desktop = matches!(record, Inbound::Desktop(_));
            let platform_focus = matches!(record, Inbound::FocusedWindow(_))
                && self
                    .capabilities
                    .supports(dockspace::runtime::NativeHostCapability::GlobalFocusObservation);
            if matches!(record, Inbound::Presentation(_))
                || ((desktop || platform_focus) && session.has_pending_native_input())
            {
                // Platform focus is mapped against the committed core roster.
                // Earlier visibility/lifecycle facts must reduce before that
                // mapping can decide whether a retained binding is reportable.
                self.transport.retry(record);
                break;
            }
            if let Some(binding) = record.binding()
                && (!session.recognizes_native_window_binding(binding)
                    || match &record {
                        Inbound::Submission(_, snapshot, original) => !self
                            .windows
                            .binding(snapshot.window_id())
                            .and_then(|current| self.windows.get(current))
                            .is_some_and(|callbacks| callbacks.has_callback_custody(original)),
                        Inbound::CaptureTerminal(_, barrier, _, original) => !self
                            .windows
                            .binding(barrier.source_window())
                            .and_then(|current| self.windows.get(current))
                            .is_some_and(|callbacks| callbacks.has_callback_custody(original)),
                        _ => self.windows.get(binding).is_none(),
                    })
                && !matches!(record, Inbound::Effect(_))
            {
                self.reports.push_back(NativeReport::StaleCallback(binding));
                collected += 1;
                continue;
            }
            match record {
                Inbound::Effect(result) => {
                    let result =
                        if let Some(callbacks) = self.windows.get(result.binding().window()) {
                            callbacks.correlate_cleanup(result)
                        } else {
                            result
                        };
                    if let Err(error) = session.report_native_effect_result(result) {
                        let (kind, result) = error.into_parts();
                        let error = NativeBridgeError::EffectReport(kind);
                        self.transport.reject(Inbound::Effect(result), &error);
                        return Err(error);
                    }
                }
                record => {
                    if let Err(error) = self.collect_record(session, &record) {
                        self.transport.reject(record, &error);
                        return Err(error);
                    }
                }
            }
            collected += 1;
            // A following pointer can use only the work-area binding minted by
            // this publication's commit, never the preceding roster's binding.
            if desktop && session.has_pending_native_input() {
                break;
            }
        }
        Ok(collected)
    }

    fn collect_record(
        &mut self,
        session: &mut DockspaceSession,
        record: &Inbound,
    ) -> Result<(), NativeBridgeError> {
        match record {
            Inbound::Observed(binding, facts) => {
                self.record_snapshot(session, Some((*binding, *facts)), None)?;
                if let Some(callbacks) = self.windows.get(*binding) {
                    callbacks.set_facts(*facts);
                }
            }
            Inbound::Close(binding, state, acknowledgement) => {
                session.publish_native_close(*binding, *state, *acknowledgement)?;
                if let Some(callbacks) = self.windows.get(*binding)
                    && let Some(facts) = callbacks.facts()
                {
                    callbacks.set_facts(facts.with_close(*state, *acknowledgement));
                }
            }
            Inbound::LogicalClosed(binding) => {
                session.report_native_input_binding_retired(*binding)?;
                if let Some(callbacks) = self.windows.get(*binding) {
                    callbacks.set_logical_closed();
                }
                self.reports
                    .push_back(NativeReport::LogicalClosed(*binding));
                if self.anchor == Some(*binding) {
                    self.draining = true;
                }
            }
            Inbound::NativeTerminal(binding, facts) => {
                self.record_snapshot(session, Some((*binding, *facts)), None)?;
                if let Some(callbacks) = self.windows.get(*binding) {
                    callbacks.set_facts(*facts);
                    callbacks.set_native_terminal();
                }
                self.reports
                    .push_back(NativeReport::NativeTerminal(*binding));
                if self.anchor == Some(*binding) {
                    self.draining = true;
                }
            }
            Inbound::Submission(binding, snapshot, _) => {
                self.reports.push_back(NativeReport::Submission {
                    binding: *binding,
                    snapshot: *snapshot,
                });
            }
            Inbound::CaptureTerminal(binding, barrier, terminal, _) => {
                if let Some(callbacks) = self
                    .windows
                    .binding(barrier.source_window())
                    .and_then(|current| self.windows.get(current))
                {
                    callbacks.finish_capture(*binding, *barrier, *terminal);
                }
                self.reports.push_back(NativeReport::CaptureTerminal {
                    binding: *binding,
                    barrier: *barrier,
                    terminal: *terminal,
                });
            }
            Inbound::CaptureUnobserved(_) => self.reports.push_back(NativeReport::Unsupported {
                operation: "observing the original native capture release after drag retirement",
            }),
            Inbound::Pointer(input) => {
                session.record_native_pointer(*input)?;
                // A local packet does not extend an earlier captured generation's
                // desktop authority. The next real captured callback supplies its own.
                self.pointer_frame = None;
                self.pointer_capture = None;
            }
            Inbound::CapturedDrag {
                binding,
                capture,
                event,
                pointer,
                metadata,
            } => {
                let callbacks = self
                    .windows
                    .get(*binding)
                    .ok_or(NativeBridgeError::UnknownBinding)?;
                if !callbacks.can_accept_capture_event(event) {
                    self.reports
                        .push_back(NativeReport::StaleCallback(*binding));
                    return Ok(());
                }
                let input = self.captured_pointer_input(
                    session, *binding, *capture, event, *pointer, *metadata,
                );
                session.record_native_pointer(input)?;
                self.pointer_frame = event.physical_frame();
                self.pointer_capture = Some(super::NativeCapture::from_event(event));
                if matches!(event.phase(), open_gpui::NativeCapturedDragPhase::Released) {
                    self.reports.push_back(NativeReport::CapturedRelease {
                        capture: super::NativeCapture::from_event(event),
                        frame: event.physical_frame(),
                    });
                }
                // Advance native correlation only after ingress accepts the
                // original front record. A refusal retains this event unchanged.
                callbacks.accept_capture_event(event);
                if *capture == NativePointerOwner::Unknown
                    && !matches!(
                        event.phase(),
                        open_gpui::NativeCapturedDragPhase::Cancelled(_)
                    )
                    && callbacks.needs_capture_observation_report()
                {
                    self.reports.push_back(NativeReport::Unsupported {
                        operation: "independent native OS capture ownership observation",
                    });
                }
                if !matches!(event.phase(), open_gpui::NativeCapturedDragPhase::Moved) {
                    callbacks.request_capture_cancellation(super::NativeCapture::from_event(event));
                }
            }
            Inbound::PointerRoster(roster) => {
                session.report_native_pointer_roster(roster.clone())?
            }
            Inbound::DeviceRemoved => session.report_native_input_device_removal()?,
            Inbound::Focus(focus) => session.report_native_global_focus(*focus)?,
            Inbound::FocusedWindow(focus) => {
                if !self
                    .capabilities
                    .supports(dockspace::runtime::NativeHostCapability::GlobalFocusObservation)
                {
                    return Ok(());
                }
                let focus = match focus {
                    open_gpui::PlatformFocusedWindow::Unavailable => NativeGlobalFocus::Unknown,
                    // No focused application window does not distinguish a
                    // foreign foreground owner from an entirely unfocused desktop.
                    open_gpui::PlatformFocusedWindow::NoWindow => NativeGlobalFocus::Unknown,
                    open_gpui::PlatformFocusedWindow::Window(window) => {
                        match self.binding_for_window(window.window_id()) {
                            Some(binding) if session.native_binding_can_report_focus(binding) => {
                                NativeGlobalFocus::Dock(binding)
                            }
                            // Existence alone does not authorize known focus:
                            // hidden, unobserved and cleanup-only owners stay Unknown.
                            Some(_) => NativeGlobalFocus::Unknown,
                            None => NativeGlobalFocus::Foreign,
                        }
                    }
                };
                session.report_native_global_focus(focus)?;
            }
            Inbound::WorkAreas(areas) => {
                self.record_snapshot(session, None, Some(areas.clone()))?;
                self.work_areas = areas.clone();
                self.desktop = super::desktop::DesktopObservation::default();
            }
            Inbound::Desktop(desktop) => {
                if *desktop != self.desktop {
                    let areas = desktop.roster();
                    self.record_snapshot(session, None, Some(areas.clone()))?;
                    self.work_areas = areas;
                    self.desktop = desktop.clone();
                    self.desktop_pending_commit = true;
                }
            }
            Inbound::Freeze => {
                self.draining = true;
                self.reports.push_back(NativeReport::GroupShutdownRequested);
            }
            Inbound::Effect(_) => return Err(NativeBridgeError::UnknownBinding),
            // The managed host consumes this barrier only after committing the
            // prefix collected above; the bridge never consumes its outputs.
            Inbound::Presentation(_) | Inbound::Wake => {}
        }
        Ok(())
    }

    fn record_snapshot(
        &self,
        session: &mut DockspaceSession,
        replacement: Option<(NativeWindowBinding, NativeWindowFacts)>,
        areas: Option<NativeWorkAreaRoster>,
    ) -> Result<(), NativeBridgeError> {
        if let Some(observations) = self.snapshot(session, replacement) {
            session.report_managed_native_snapshot(
                observations,
                areas.unwrap_or_else(|| self.work_areas.clone()),
            )?;
        } else {
            session.report_native_inventory_unknown()?;
        }
        Ok(())
    }

    /// Retries a complete current inventory after registrations or create failures commit.
    pub fn publish_inventory(
        &self,
        session: &mut DockspaceSession,
    ) -> Result<(), NativeBridgeError> {
        if !self.transport.is_empty() {
            return Err(NativeBridgeError::NotQuiescent);
        }
        self.record_snapshot(session, None, None)
    }
}
