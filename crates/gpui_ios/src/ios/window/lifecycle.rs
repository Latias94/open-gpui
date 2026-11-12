use super::*;

impl IosWindowState {
    pub(super) fn quiesce_interaction(&self) {
        self.interaction_quiesced.set(true);
        self.back_gesture_start.set(None);
        self.keyboard_dismiss_touch.set(None);
        self.input_callback.terminate();
        self.text_input_view.quiesce_interaction();
        self.view.setUserInteractionEnabled(false);
        self.window.setUserInteractionEnabled(false);
        if self.window.isKeyWindow() {
            // UIKit's resignKeyWindow is a notification hook, not a key-window setter.
            // A synchronous hide/show releases its key ownership while retaining the view.
            let hidden = self.window.isHidden();
            self.window.setHidden(true);
            self.window.setHidden(hidden);
        }
        self.notify_active_status_change(false);
    }

    pub(super) fn dispatch_command(
        self: &Rc<Self>,
        command: PlatformWindowCommand,
    ) -> PlatformWindowCommandOutcome {
        if self.closed.get() {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        if self.interaction_quiesced.get() && command.requires_interaction_authority() {
            return PlatformWindowCommandOutcome::Rejected;
        }
        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                if !self.initial_presentation_completed.replace(true) && self.creation_facts.show {
                    if activate && self.creation_facts.focus_on_appearing {
                        self.window.makeKeyAndVisible();
                    } else {
                        self.window.setHidden(false);
                    }
                    self.handle_layout_change();
                    self.notify_active_status_change(true);
                    if self
                        .platform
                        .upgrade()
                        .is_some_and(|platform| platform.application.is_foreground())
                    {
                        self.notify_visibility_change(WindowVisibility::Visible);
                    }
                    self.queue_frame(RequestFrameOptions {
                        force_render: true,
                        require_presentation: true,
                        ..Default::default()
                    });
                }
            }
            PlatformWindowCommand::Activate { .. } => {
                self.window.makeKeyAndVisible();
                self.refresh_native_visibility();
                self.notify_active_status_change(true);
            }
            PlatformWindowCommand::ShowSoftKeyboard => self.show_keyboard(),
            PlatformWindowCommand::HideSoftKeyboard => self.hide_keyboard(),
            PlatformWindowCommand::TextInputStateChanged(change) => {
                self.text_input_view.state_changed(change)
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
            | PlatformWindowCommand::ShowWindowMenu(_)
            | PlatformWindowCommand::StartWindowMove
            | PlatformWindowCommand::StartWindowResize(_)
            | PlatformWindowCommand::RestoreNativeWindowState(_)
            | PlatformWindowCommand::ToggleSimpleFullscreen => {
                return PlatformWindowCommandOutcome::Unsupported;
            }
        }
        if self.closed.get() {
            PlatformWindowCommandOutcome::WindowClosed
        } else {
            PlatformWindowCommandOutcome::Accepted
        }
    }

    pub(super) fn quiesce_presentation(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformPresentationShutdownOutcome {
        if shutdown.snapshot().window_id() != self.handle.window_id() {
            return PlatformPresentationShutdownOutcome::Rejected;
        }
        {
            let mut current = self.presentation_shutdown.borrow_mut();
            if let Some(current) = current.as_ref() {
                if !current.same_authority(shutdown) {
                    return PlatformPresentationShutdownOutcome::Rejected;
                }
            } else {
                *current = Some(shutdown.clone());
            }
        }
        {
            let Some(mut renderer) = self.renderer.try_lock() else {
                return PlatformPresentationShutdownOutcome::Rejected;
            };
            if renderer.is_quiesced_for(shutdown) {
                return PlatformPresentationShutdownOutcome::Quiesced;
            }
            if !renderer.reserve_surface_quiescence(shutdown) {
                return PlatformPresentationShutdownOutcome::Rejected;
            }
        }
        self.quiesce_interaction();
        self.input_handler.terminate();
        self.frame_demand.clear();
        self.request_frame_callback.take();
        self.text_input_view.set_window(Weak::new());
        self.text_input_view.set_keyboard_visible(false);
        self.text_input_view.removeFromSuperview();
        self.view.removeFromSuperview();
        self.view_controller.setView(None);
        if let Some(layer) = self.view.layer().downcast_ref::<CAMetalLayer>() {
            layer.setDevice(None);
        }
        let Some(mut renderer) = self.renderer.try_lock() else {
            return PlatformPresentationShutdownOutcome::Rejected;
        };
        if renderer.finish_surface_quiescence(shutdown) {
            PlatformPresentationShutdownOutcome::Quiesced
        } else {
            PlatformPresentationShutdownOutcome::Rejected
        }
    }

    pub(super) fn retire_native(
        self: &Rc<Self>,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        if !self
            .presentation_shutdown
            .borrow()
            .as_ref()
            .is_some_and(|current| current.same_authority(shutdown))
        {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        if self.closed.get() {
            return if shutdown.snapshot().native_terminal() {
                PlatformNativeWindowRetirementOutcome::Accepted
            } else {
                PlatformNativeWindowRetirementOutcome::Rejected
            };
        }
        if !self.renderer.lock().is_quiesced_for(shutdown) {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        self.closed.set(true);
        self.visibility.set(WindowVisibility::Hidden);
        self.window.setHidden(true);
        self.window.setRootViewController(None);
        self.window.setWindowScene(None);
        *self.view_controller.ivars().0.borrow_mut() = Weak::new();
        *self.view.ivars().0.borrow_mut() = Weak::new();
        if let Some(platform) = self.platform.upgrade() {
            platform.application.unregister_window(self);
        }
        let observers = std::mem::take(&mut *self.native_observers.borrow_mut());
        for observer in observers {
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
        if !shutdown.acknowledge_native_terminal() {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        if let Some(callback) = self.close_callback.take() {
            callback();
        }
        PlatformNativeWindowRetirementOutcome::Accepted
    }
}
