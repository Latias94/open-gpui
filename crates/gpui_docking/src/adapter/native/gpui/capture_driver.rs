//! Original native drag generation and asynchronous capture release custody.

use super::*;

pub(super) struct CaptureRelease {
    pub(super) binding: NativeWindowBinding,
    barrier: NativeCapturedDragReleaseBarrier,
    cancelled_from_keyboard: bool,
}

impl CaptureRelease {
    fn matches_capture(&self, capture: NativeCapture) -> bool {
        self.barrier.source_window() == capture.source
            && self.barrier.drag_generation() == capture.generation
    }
}

impl NativeCallbacks {
    pub(in crate::adapter) fn can_accept_capture_event(
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

    pub(in crate::adapter::native) fn accept_capture_event(
        &self,
        event: &open_gpui::NativeCapturedDragEvent,
    ) {
        let capture = NativeCapture::from_event(event);
        if self.0.capture.get().map(|previous| previous.generation) != Some(capture.generation) {
            self.0.capture_observation_missing.set(false);
        }
        self.0.capture_ingress.set(event.sequence().ordinal());
        self.0.capture.set(Some(capture));
        if self.0.capture_start.get() == Some(capture) {
            self.0.capture_start.set(None);
        }
    }

    pub(crate) fn active_capture(&self) -> Option<NativeCapture> {
        if self.0.sealed.get()
            || self.0.logical_closed.get()
            || self.0.native_terminal.get()
            || self.0.capture_failed.get()
            || !self.0.capture_releases.borrow().is_empty()
        {
            return None;
        }
        self.0.capture.get().or(self.0.capture_start.get())
    }

    pub(crate) fn retain_drag_tear_off(&self, capture: NativeCapture, drag: NativeDragTearOff) {
        if self.window_id() == Some(capture.source_window())
            && !self.0.sealed.get()
            && !self.0.logical_closed.get()
            && (self.0.capture.get() == Some(capture)
                || self.0.capture_start.get() == Some(capture))
        {
            self.0.drag_tear_off.set(Some((capture, drag)));
        }
    }

    pub(crate) fn drag_tear_off(&self, capture: NativeCapture) -> Option<NativeDragTearOff> {
        self.0
            .drag_tear_off
            .get()
            .filter(|(owner, _)| *owner == capture)
            .map(|(_, drag)| drag)
    }

    pub(crate) fn retain_capture_start(
        &self,
        source: open_gpui::WindowId,
        consumer: &open_gpui::PreparedNativeCapturedDragConsumer,
    ) {
        if self.window_id() == Some(source) && consumer.is_active() && !self.0.sealed.get() {
            let capture = NativeCapture {
                source,
                generation: consumer.generation(),
            };
            // GPUI has already activated this original generation. Closing may
            // race its first native motion, so custody begins at activation.
            self.0.capture_start.set(Some(capture));
            self.0
                .transport
                .retain_mouse_capture(self.binding(), capture);
            self.0.capture_observation_missing.set(false);
        }
    }

    pub(in crate::adapter::native) fn request_capture_cancellation(&self, capture: NativeCapture) {
        if self.0.native_terminal.get()
            || self.0.capture_failed.get()
            || self
                .0
                .capture_releases
                .borrow()
                .iter()
                .any(|release| release.matches_capture(capture))
        {
            return;
        }
        let mut pending = self.0.capture_cancellations.borrow_mut();
        if pending.back() != Some(&capture) {
            pending.push_back(capture);
        }
    }

    pub(in crate::adapter::native) fn needs_capture_observation_report(&self) -> bool {
        !self.0.capture_observation_missing.replace(true)
    }

    /// Retains GPUI's original release barrier for this event's drag generation.
    ///
    /// Call this during the ReleaseLocker phase: the first callback of
    /// [`App::consume_native_captured_drag`], before the later event consumer calls
    /// [`NativeBridge::captured_drag`]. Resolve this callback owner from the event's
    /// full source-window id. The original native binding, drag generation and
    /// release barrier remain owned until their actual terminal enters the same FIFO.
    ///
    /// This only observes a barrier already reserved by GPUI; it does not cancel
    /// the drag, create another release or acknowledge a docking output. Repeated
    /// observations of the same pending release are harmless. Calling only from
    /// the later event consumer cannot recover a retired barrier, and a missing
    /// observation never authorizes release or shutdown.
    pub fn observe_capture_release(&self, event: &open_gpui::NativeCapturedDragEvent, cx: &App) {
        if !self.can_accept_capture_event(event)
            || self.0.capture_failed.get()
            || matches!(event.phase(), open_gpui::NativeCapturedDragPhase::Moved)
            || self
                .0
                .capture_releases
                .borrow()
                .iter()
                .any(|release| release.matches_capture(NativeCapture::from_event(event)))
        {
            return;
        }
        let weak = Rc::downgrade(&self.0);
        let binding = self.binding();
        let completion = self.0.transport.retain_callback();
        if let Some(barrier) = cx.observe_native_captured_drag_release(
            event.source_window(),
            event.generation(),
            move |barrier, terminal, cx| {
                if let Some(state) = weak.upgrade() {
                    state.transport.push(
                        Inbound::CaptureTerminal(binding, barrier, terminal, CallbackCustody(weak)),
                        cx,
                    );
                }
                drop(completion);
            },
        ) {
            self.0.capture_releases.borrow_mut().push(CaptureRelease {
                binding,
                barrier,
                cancelled_from_keyboard: false,
            });
        }
    }

    pub(in crate::adapter::native) fn finish_capture(
        &self,
        binding: NativeWindowBinding,
        barrier: open_gpui::NativeCapturedDragReleaseBarrier,
        terminal: open_gpui::NativeCapturedDragReleaseTerminal,
    ) {
        let mut releases = self.0.capture_releases.borrow_mut();
        let Some(index) = releases
            .iter()
            .position(|pending| pending.binding == binding && pending.barrier == barrier)
        else {
            return;
        };
        releases.swap_remove(index);
        drop(releases);
        self.0.capture_cancellations.borrow_mut().retain(|capture| {
            capture.source != barrier.source_window()
                || capture.generation != barrier.drag_generation()
        });
        if terminal == open_gpui::NativeCapturedDragReleaseTerminal::Failed {
            self.0.capture_failed.set(true);
        } else {
            if self.0.capture.get().is_some_and(|capture| {
                capture.source == barrier.source_window()
                    && capture.generation == barrier.drag_generation()
            }) {
                self.0.capture.set(None);
            }
            if self.0.capture_start.get().is_some_and(|capture| {
                capture.source == barrier.source_window()
                    && capture.generation == barrier.drag_generation()
            }) {
                self.0.capture_start.set(None);
            }
        }
    }

    /// Cancels the original GPUI native capture and retains its exact release barrier.
    /// A failed or unobservable release blocks close until the real native
    /// terminal; service calls do not retry an already retired barrier.
    pub fn cancel_capture(&self, cx: &mut App) {
        if let Some(capture) = self.0.capture.get() {
            self.request_capture_cancellation(capture);
        }
        if let Some(capture) = self.0.capture_start.get() {
            self.request_capture_cancellation(capture);
        }
        self.dispatch_capture_cancellations(cx);
    }

    pub(in crate::adapter) fn cancel_capture_from_keyboard(&self, cx: &mut App) -> bool {
        let Some(capture) = self.0.transport.captured_mouse_press(self.binding()) else {
            return false;
        };
        self.accepts_pointer_controls()
            && !self.0.capture_failed.get()
            && self.cancel_capture_identity(capture, true, cx)
    }

    pub(in crate::adapter::native) fn is_user_capture_cancellation(
        &self,
        event: &open_gpui::NativeCapturedDragEvent,
    ) -> bool {
        matches!(
            event.phase(),
            open_gpui::NativeCapturedDragPhase::Cancelled(
                open_gpui::PointerCancelReason::CaptureRevoked
            )
        ) && self.0.capture_releases.borrow().iter().any(|release| {
            release.cancelled_from_keyboard
                && release.matches_capture(NativeCapture::from_event(event))
        })
    }

    pub(in crate::adapter::native) fn dispatch_capture_cancellations(&self, cx: &mut App) {
        // A terminal failure quarantines this physical callback until its true
        // native terminal. Retrying a retired barrier cannot prove release.
        if self.0.capture_failed.get() {
            return;
        }
        if self.0.native_terminal.get() {
            self.0.capture_cancellations.borrow_mut().clear();
            return;
        }
        let pending = self.0.capture_cancellations.borrow().len();
        for _ in 0..pending {
            // A later generation cannot replace the original in-flight barrier.
            // Its request remains queued until that exact release completes.
            if !self.0.capture_releases.borrow().is_empty() {
                break;
            }
            let Some(capture) = self.0.capture_cancellations.borrow_mut().pop_front() else {
                break;
            };
            self.cancel_capture_identity(capture, false, cx);
        }
    }

    fn cancel_capture_identity(
        &self,
        capture: NativeCapture,
        from_keyboard: bool,
        cx: &mut App,
    ) -> bool {
        let weak = Rc::downgrade(&self.0);
        let binding = self.binding();
        let completion = self.0.transport.retain_callback();
        let on_terminal = move |barrier, terminal, cx: &mut App| {
            if let Some(state) = weak.upgrade() {
                state.transport.push(
                    Inbound::CaptureTerminal(binding, barrier, terminal, CallbackCustody(weak)),
                    cx,
                );
            }
            drop(completion);
        };
        let reason = open_gpui::PointerCancelReason::CaptureRevoked;
        let barrier = if from_keyboard {
            cx.try_cancel_native_captured_drag_with_release_barrier(
                capture.source,
                capture.generation,
                reason,
                on_terminal,
            )
        } else {
            cx.cancel_native_captured_drag_with_release_barrier(
                capture.source,
                capture.generation,
                reason,
                on_terminal,
            )
        };
        if let Some(barrier) = barrier {
            self.0.capture_releases.borrow_mut().push(CaptureRelease {
                binding,
                barrier,
                cancelled_from_keyboard: from_keyboard,
            });
        } else if !from_keyboard {
            self.0.capture_failed.set(true);
            self.0
                .transport
                .push(Inbound::CaptureUnobserved(self.binding()), cx);
        }
        barrier.is_some()
    }
}

impl NativeBridge {
    pub(crate) fn pointer_frame(&self) -> Option<open_gpui::PlatformNativePointerPhysicalFrame> {
        self.captured_pointer.as_ref()?.frame
    }

    pub(crate) fn pointer_capture(&self) -> Option<NativeCapture> {
        self.captured_pointer.as_ref().map(|sample| sample.capture)
    }
}
