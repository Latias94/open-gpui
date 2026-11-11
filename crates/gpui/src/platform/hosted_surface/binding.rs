use super::*;
use crate::{
    GpuSpecs, PlatformDisplay, PlatformFrameDeliveryMode, PlatformFrameSubmissionStatus,
    PlatformInputCallback, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PlatformWindowInteractionQuiescence, PromptButton, PromptLevel,
    WindowControlArea, WindowPlatformFacts,
};
use futures::channel::oneshot;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

pub(super) struct HostedSurfaceWindow {
    state: Rc<HostedSurfaceState>,
}

impl HostedSurfaceWindow {
    pub(super) fn new(state: Rc<HostedSurfaceState>) -> Self {
        Self { state }
    }
}

impl Drop for HostedSurfaceWindow {
    fn drop(&mut self) {
        let shutdown = self.state.shutdown.borrow().clone();
        let safe = match shutdown {
            Some(shutdown) => {
                let snapshot = shutdown.snapshot();
                snapshot.quiesced() && snapshot.detached()
            }
            None => !self.state.renderer_used.get(),
        };
        if !safe {
            self.state.begin_close();
            log::error!(
                "hosted surface binding dropped without exact presentation detachment; retaining its owner until process teardown"
            );
            std::mem::forget(self.state.clone());
            return;
        }
        self.state.finish_close();
    }
}

impl HasWindowHandle for HostedSurfaceWindow {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::NotSupported)
    }
}

impl HasDisplayHandle for HostedSurfaceWindow {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::NotSupported)
    }
}

impl PlatformWindow for HostedSurfaceWindow {
    fn has_native_window(&self) -> bool {
        false
    }

    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let state = Rc::downgrade(&self.state);
        PlatformWindowCommandDispatcher::new(move |command| {
            let Some(state) = state.upgrade() else {
                return PlatformWindowCommandOutcome::WindowClosed;
            };
            state.execute_command(command)
        })
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let state = Rc::downgrade(&self.state);
        PlatformWindowInteractionQuiescence::new(move || {
            if let Some(state) = state.upgrade() {
                state.invalidate_submission();
                state.interaction_quiesced.set(true);
                state.active.set(false);
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let current = self.state.shutdown.borrow().clone();
        if let Some(current) = current {
            return current;
        }
        if Some(shutdown.snapshot().window_id()) != self.state.bound_id.get() {
            return PreparedPlatformPresentationShutdown::new(shutdown, |_| {
                PlatformPresentationShutdownOutcome::Rejected
            });
        }
        self.state.begin_close();
        let prepared = self.state.renderer.prepare_shutdown(shutdown.clone());
        let prepared = if prepared.ticket().same_authority(&shutdown) {
            prepared
        } else {
            log::error!("hosted renderer returned a different presentation-shutdown authority");
            PreparedPlatformPresentationShutdown::new(shutdown, |_| {
                PlatformPresentationShutdownOutcome::Rejected
            })
        };
        self.state.shutdown.replace(Some(prepared.clone()));
        prepared
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        let current = self.state.shutdown.borrow().clone();
        if current
            .as_ref()
            .is_some_and(|current| current.ticket().same_authority(shutdown))
            && Some(shutdown.snapshot().window_id()) == self.state.bound_id.get()
            && shutdown.snapshot().quiesced()
        {
            PlatformNativeWindowRetirementOutcome::BindingDetached
        } else {
            PlatformNativeWindowRetirementOutcome::Rejected
        }
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(Point::default(), self.content_size())
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.state.geometry.get().size()
    }

    fn scale_factor(&self) -> f32 {
        self.state.geometry.get().scale_factor()
    }

    fn appearance(&self) -> WindowAppearance {
        self.state.appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.state.mouse_position.get()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        if !self.state.terminal.get() && self.state.cursor_style.replace(style) != style {
            (self.state.wake)();
        }
    }

    fn modifiers(&self) -> Modifiers {
        self.state.modifiers.get()
    }

    fn capslock(&self) -> Capslock {
        self.state.capslock.get()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        if !self.state.closing.get() {
            self.state.input_handler.set(input_handler);
        }
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.state.input_handler.take()
    }

    fn take_input_handler_for_frame(&mut self) -> Option<PlatformInputHandler> {
        self.state.input_handler.take_for_frame()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn input_handler_slot_for_test(&self) -> Option<PlatformInputHandlerSlot> {
        Some(self.state.input_handler.clone())
    }

    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        if !self.state.terminal.get()
            && self.state.text_input.borrow().configuration != configuration
        {
            self.state
                .update_text_input(|state| state.configuration = configuration);
        }
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }

    fn is_active(&self) -> bool {
        !self.state.closing.get() && self.state.active.get()
    }

    fn visibility(&self) -> WindowVisibility {
        self.state.visibility()
    }

    fn is_hovered(&self) -> bool {
        !self.state.closing.get() && self.state.hovered.get()
    }

    fn accepts_pointer_input(&self) -> bool {
        !self.state.closing.get() && !self.state.interaction_quiesced.get()
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.state
            .creation_facts
            .borrow()
            .as_ref()
            .expect("hosted platform binding has creation facts")
            .clone()
    }

    fn is_visible(&self) -> bool {
        self.state.visibility().is_visible()
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        let geometry = self.state.geometry.get();
        let bounds = Bounds::new(Point::default(), geometry.size());
        WindowPlatformFacts {
            bounds,
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds: WindowBounds::Windowed(bounds),
            inner_window_bounds: WindowBounds::Windowed(bounds),
            content_size: geometry.size(),
            scale_factor: geometry.scale_factor(),
            display_id: None,
            is_minimized: false,
            is_maximized: false,
            is_fullscreen: false,
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: false,
            focus_on_click: false,
            background_appearance: self.state.background.get(),
            topmost: false,
            taskbar_visible: false,
            is_active: self.is_active(),
        }
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.state.background.get()
    }

    fn set_title(&mut self, _title: &str) {}

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {}

    fn is_fullscreen(&self) -> bool {
        false
    }

    fn request_frame(&self, options: RequestFrameOptions) {
        self.state.request_frame(options);
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        let state = Rc::downgrade(&self.state);
        Some(Rc::new(move || {
            if let Some(state) = state.upgrade() {
                state.request_frame(RequestFrameOptions {
                    signal_source: FrameRequestSource::LocalSchedule,
                    ..RequestFrameOptions::default()
                });
            }
        }))
    }

    fn schedule_frame(&self) {
        self.state.request_frame(RequestFrameOptions {
            signal_source: FrameRequestSource::LocalSchedule,
            ..RequestFrameOptions::default()
        });
    }

    fn on_request_frame(&self, callback: Box<dyn Fn(RequestFrameOptions)>) {
        if !self.state.closing.get() {
            self.state
                .request_frame_callback
                .replace(Some(Rc::from(callback)));
        }
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        if !self.state.closing.get() {
            self.state.input_callback.set(callback);
            self.state.input_installed.set(true);
        }
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn Fn(PlatformWindowActiveStatusObservation)>,
    ) {
        if !self.state.closing.get() {
            self.state.active_callback.replace(Some(Rc::from(callback)));
        }
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.state.visibility_callback.set(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn Fn(bool)>) {
        if !self.state.closing.get() {
            self.state.hover_callback.replace(Some(Rc::from(callback)));
        }
    }

    fn on_resize(&self, callback: Box<dyn Fn(Size<Pixels>, f32)>) {
        if !self.state.closing.get() {
            self.state.resize_callback.replace(Some(Rc::from(callback)));
        }
    }

    fn on_moved(&self, _callback: Box<dyn Fn()>) {}

    fn on_display_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {}

    fn on_close_required(&self, callback: Box<dyn FnOnce()>) {
        if !self.state.closing.get() {
            self.state.close_required_callback.replace(Some(callback));
        }
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        if !self.state.terminal.get() {
            self.state.close_callback.replace(Some(callback));
        }
    }

    fn on_appearance_changed(&self, _callback: Box<dyn Fn()>) {}

    fn draw(&self, _scene: &Scene) -> PlatformWindowPresentOutcome {
        PlatformWindowPresentOutcome::Rejected
    }

    fn frame_delivery_mode(&self) -> PlatformFrameDeliveryMode {
        PlatformFrameDeliveryMode::Deferred
    }

    fn draw_with_submission(
        &self,
        scene: &Scene,
        submission: &PlatformFrameSubmission,
    ) -> PlatformWindowPresentOutcome {
        if self.state.ensure_bound().is_err()
            || Some(submission.window_id()) != self.state.bound_id.get()
        {
            return PlatformWindowPresentOutcome::Rejected;
        }
        if !submission.is_pending() {
            return PlatformWindowPresentOutcome::RepaintRequired;
        }
        let pending = self.state.pending_submission.borrow().clone();
        if let Some(pending) = pending
            && pending.status() == PlatformFrameSubmissionStatus::Pending
        {
            return if pending.same_authority(submission) {
                PlatformWindowPresentOutcome::Deferred
            } else {
                PlatformWindowPresentOutcome::Rejected
            };
        }
        self.state
            .pending_submission
            .replace(Some(submission.clone()));
        let geometry = self.state.geometry.get();
        if geometry.is_suspended() || !self.state.host_visible.get() {
            return PlatformWindowPresentOutcome::Deferred;
        }
        self.state.renderer_used.set(true);
        let outcome = self.state.renderer.draw(scene, geometry, submission);
        if self.state.closing.get() || self.state.terminal.get() {
            return PlatformWindowPresentOutcome::Rejected;
        }
        let _ = submission.is_pending();
        match submission.status() {
            PlatformFrameSubmissionStatus::Pending => outcome,
            PlatformFrameSubmissionStatus::Submitted => PlatformWindowPresentOutcome::Deferred,
            PlatformFrameSubmissionStatus::RepaintRequired
            | PlatformFrameSubmissionStatus::Cancelled => {
                PlatformWindowPresentOutcome::RepaintRequired
            }
            PlatformFrameSubmissionStatus::Rejected => PlatformWindowPresentOutcome::Rejected,
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.state.renderer.sprite_atlas()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        if !self.state.terminal.get() && self.state.text_input.borrow().caret_bounds != Some(bounds)
        {
            self.state
                .update_text_input(|state| state.caret_bounds = Some(bounds));
        }
    }

    fn show_soft_keyboard(&self) {
        if !self.state.terminal.get() && !self.state.text_input.borrow().soft_keyboard_visible {
            self.state
                .update_text_input(|state| state.soft_keyboard_visible = true);
        }
    }

    fn hide_soft_keyboard(&self) {
        if !self.state.terminal.get() && self.state.text_input.borrow().soft_keyboard_visible {
            self.state
                .update_text_input(|state| state.soft_keyboard_visible = false);
        }
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        if !self.state.terminal.get() {
            self.state.text_input_changed(change);
        }
    }
}

impl HostedSurfaceState {
    fn execute_command(&self, command: PlatformWindowCommand) -> PlatformWindowCommandOutcome {
        if self.closing.get() || self.terminal.get() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate: false } => {
                if !self.initially_presented.get() {
                    self.invalidate_submission();
                    let previous = self.visibility();
                    self.initially_presented.set(true);
                    let current = self.visibility();
                    if current != previous {
                        self.visibility_callback.call(current);
                    }
                    (self.wake)();
                }
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::ShowSoftKeyboard if !self.interaction_quiesced.get() => {
                self.update_text_input(|state| state.soft_keyboard_visible = true);
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::HideSoftKeyboard => {
                self.update_text_input(|state| state.soft_keyboard_visible = false);
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::TextInputStateChanged(change) => {
                self.text_input_changed(change);
                PlatformWindowCommandOutcome::Accepted
            }
            _ => PlatformWindowCommandOutcome::Unsupported,
        }
    }
}
