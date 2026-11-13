use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use dockspace::runtime::{
    DockspaceSession, NativeCloseEffectAcknowledgement, NativeCloseState, NativeEffectResult,
    NativeGlobalFocus, NativePointerId, NativePointerInput, NativePointerRoster,
    NativeWindowBinding, NativeWindowFacts, NativeWorkAreaRoster, PointerEventMetadata,
};
use open_gpui::{
    App, NativeCapturedDragReleaseBarrier, NativeCapturedDragReleaseTerminal,
    WindowFrameSubmissionSnapshot,
};

use super::{NativeBridge, NativeBridgeError, NativeReport, gpui::CallbackCustody};

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

type Wake = dyn Fn(&mut App);

struct TransportState {
    queue: RefCell<VecDeque<Inbound>>,
    wake: Rc<Wake>,
    scheduled: Cell<bool>,
    frozen: Cell<bool>,
}

/// UI-thread FIFO shared by native callbacks and the application's one host owner.
///
/// The wake runs in `App::defer`, outside native callbacks and host borrows. Its
/// closure should schedule the host pump through a weak owner. Rejected core
/// ingress retains the original front record and never allows a suffix past it.
#[derive(Clone)]
pub struct NativeTransport(Rc<TransportState>);

impl NativeTransport {
    /// Creates a callback queue with an application-owned wakeup.
    pub fn new(wake: impl Fn(&mut App) + 'static) -> Self {
        Self(Rc::new(TransportState {
            queue: RefCell::new(VecDeque::new()),
            wake: Rc::new(wake),
            scheduled: Cell::new(false),
            frozen: Cell::new(false),
        }))
    }

    /// Records an already captured native pointer packet without changing its lane.
    pub fn pointer(&self, input: NativePointerInput, cx: &mut App) {
        self.push(Inbound::Pointer(input), cx);
    }

    pub(crate) fn pointer_in_window(&self, input: NativePointerInput) {
        self.0.queue.borrow_mut().push_back(Inbound::Pointer(input));
    }

    pub(crate) fn captured_drag(
        &self,
        binding: NativeWindowBinding,
        event: open_gpui::NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
        cx: &mut App,
    ) {
        self.push(
            Inbound::CapturedDrag {
                binding,
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

    /// Publishes an independently observed global focus fact.
    pub fn focus(&self, focus: NativeGlobalFocus, cx: &mut App) {
        self.push(Inbound::Focus(focus), cx);
    }

    /// Returns whether accepted callback work still needs collection.
    pub fn is_empty(&self) -> bool {
        self.0.queue.borrow().is_empty()
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

    pub(super) fn wake_after_callback(&self, cx: &App) -> impl FnOnce() + 'static {
        let queue = self.clone();
        let foreground = cx.foreground_executor().clone();
        let async_cx = cx.to_async();
        move || {
            foreground
                .spawn(async move {
                    async_cx.update(|cx| queue.push(Inbound::Wake, cx));
                })
                .detach();
        }
    }

    fn pop(&self) -> Option<Inbound> {
        self.0.queue.borrow_mut().pop_front()
    }
    fn retry(&self, record: Inbound) {
        self.0.queue.borrow_mut().push_front(record);
    }
}

pub(super) enum Inbound {
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
        event: open_gpui::NativeCapturedDragEvent,
        pointer: NativePointerId,
        metadata: PointerEventMetadata,
    },
    PointerRoster(NativePointerRoster),
    WorkAreas(NativeWorkAreaRoster),
    Focus(NativeGlobalFocus),
    DeviceRemoved,
    Effect(NativeEffectResult),
    Freeze,
    Wake,
}

impl Inbound {
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
    /// Records the successful callback prefix, retaining the exact rejected front.
    ///
    /// This does not begin or commit a host frame. InputCapacity and pending-roster
    /// failures need the host to commit its accepted native prefix before retrying.
    pub fn collect(&mut self, session: &mut DockspaceSession) -> Result<usize, NativeBridgeError> {
        let mut collected = 0;
        while let Some(record) = self.transport.pop() {
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
                        self.transport.retry(Inbound::Effect(result));
                        return Err(NativeBridgeError::EffectReport(kind));
                    }
                }
                record => {
                    if let Err(error) = self.collect_record(session, &record) {
                        self.transport.retry(record);
                        return Err(error);
                    }
                }
            }
            collected += 1;
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
            Inbound::Pointer(input) => session.record_native_pointer(*input)?,
            Inbound::CapturedDrag {
                binding,
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
                let input =
                    self.captured_pointer_input(session, *binding, event, *pointer, *metadata);
                session.record_native_pointer(input)?;
                // Advance native correlation only after ingress accepts the
                // original front record. A refusal retains this event unchanged.
                callbacks.accept_capture_event(event);
                if callbacks.needs_capture_observation_report() {
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
            Inbound::WorkAreas(areas) => {
                self.record_snapshot(session, None, Some(areas.clone()))?;
                self.work_areas = areas.clone();
            }
            Inbound::Freeze => {
                self.draining = true;
                self.reports.push_back(NativeReport::GroupShutdownRequested);
            }
            Inbound::Effect(_) => return Err(NativeBridgeError::UnknownBinding),
            Inbound::Wake => {}
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
