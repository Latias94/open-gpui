//! Headless windows for the Linux platform client.
//!
//! A headless window has no compositor surface and no GPU. Layout, text
//! shaping, and entity plumbing run normally, `draw` discards the scene, and
//! the sprite atlas hands out tiles without uploading pixels.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use open_gpui::{
    Bounds, Capslock, CursorStyle, GpuSpecs, HeadlessAtlas, Modifiers, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformPresentationShutdownOutcome, PlatformWindow,
    PlatformWindowActiveStatusObservation, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PlatformWindowInteractionQuiescence,
    PlatformWindowPresentOutcome, Point, PreparedPlatformPresentationShutdown, PromptButton,
    PromptLevel, RequestFrameOptions, Scene, Size, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowCreationFacts, WindowParams, WindowPlatformFacts,
    WindowPresentationShutdownTicket, WindowVisibility,
};

struct HeadlessWindowState {
    bounds: Bounds<Pixels>,
    window_bounds: WindowBounds,
    input_callback: PlatformInputCallbackSlot,
    input_handler: PlatformInputHandlerSlot,
    title: Option<String>,
    cursor_style: CursorStyle,
    accepts_pointer_input: bool,
    interaction_quiesced: bool,
    creation_facts: WindowCreationFacts,
    atlas: Arc<HeadlessAtlas>,
    close_callback: Option<Box<dyn FnOnce()>>,
}

pub(crate) struct HeadlessWindow(Rc<RefCell<HeadlessWindowState>>);

impl Drop for HeadlessWindow {
    fn drop(&mut self) {
        let (input_callback, input_handler, close_callback) = {
            let mut state = self.0.borrow_mut();
            (
                state.input_callback.clone(),
                state.input_handler.clone(),
                state.close_callback.take(),
            )
        };
        input_callback.terminate();
        input_handler.terminate();
        if let Some(callback) = close_callback {
            callback();
        }
    }
}

impl raw_window_handle::HasWindowHandle for HeadlessWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl raw_window_handle::HasDisplayHandle for HeadlessWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl HeadlessWindow {
    pub(crate) fn new(params: WindowParams) -> Self {
        let window_bounds = params.window_bounds;
        Self(Rc::new(RefCell::new(HeadlessWindowState {
            bounds: window_bounds.get_bounds(),
            window_bounds,
            input_callback: PlatformInputCallbackSlot::default(),
            input_handler: PlatformInputHandlerSlot::default(),
            title: None,
            cursor_style: CursorStyle::Arrow,
            accepts_pointer_input: params.accepts_pointer_input,
            interaction_quiesced: false,
            creation_facts: WindowCreationFacts {
                show: params.show,
                focus_on_appearing: params.focus_on_appearing,
                transient_for: None,
            },
            atlas: Arc::new(HeadlessAtlas::default()),
            close_callback: None,
        })))
    }
}

impl PlatformWindow for HeadlessWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let state = Rc::downgrade(&self.0);
        PlatformWindowCommandDispatcher::new(move |command| {
            let Some(state) = state.upgrade() else {
                return PlatformWindowCommandOutcome::WindowClosed;
            };
            if state.borrow().interaction_quiesced && command.requires_interaction_authority() {
                return PlatformWindowCommandOutcome::Rejected;
            }
            match command {
                PlatformWindowCommand::CompleteNativeFrame => {
                    PlatformWindowCommandOutcome::Accepted
                }
                PlatformWindowCommand::CompleteInitialPresentation { .. } => {
                    PlatformWindowCommandOutcome::Accepted
                }
                PlatformWindowCommand::Activate { .. } => PlatformWindowCommandOutcome::Unsupported,
                PlatformWindowCommand::RevealDeferredInitialPresentation { .. }
                | PlatformWindowCommand::ShowWindowMenu(_)
                | PlatformWindowCommand::StartWindowMove
                | PlatformWindowCommand::StartWindowResize(_) => {
                    PlatformWindowCommandOutcome::Rejected
                }
            }
        })
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let state = Rc::downgrade(&self.0);
        PlatformWindowInteractionQuiescence::new(move || {
            if let Some(state) = state.upgrade() {
                state.borrow_mut().interaction_quiesced = true;
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        PreparedPlatformPresentationShutdown::new(shutdown, |shutdown| {
            if shutdown.acknowledge_quiesced() {
                PlatformPresentationShutdownOutcome::Quiesced
            } else {
                PlatformPresentationShutdownOutcome::Rejected
            }
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }

    fn is_maximized(&self) -> bool {
        matches!(self.0.borrow().window_bounds, WindowBounds::Maximized(_))
    }

    fn window_bounds(&self) -> WindowBounds {
        self.0.borrow().window_bounds
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn scale_factor(&self) -> f32 {
        1.0
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.0.borrow_mut().cursor_style = style;
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.0.borrow().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.0.borrow().input_handler.clone();
        input_handler_slot.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn is_active(&self) -> bool {
        false
    }

    /// Headless windows present no frames: `draw` discards every scene.
    fn visibility(&self) -> WindowVisibility {
        WindowVisibility::Hidden
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn accepts_pointer_input(&self) -> bool {
        self.0.borrow().accepts_pointer_input
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.0.borrow().creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        false
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        WindowPlatformFacts {
            bounds: self.bounds(),
            coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds: self.window_bounds(),
            inner_window_bounds: self.inner_window_bounds(),
            content_size: self.content_size(),
            scale_factor: self.scale_factor(),
            display_id: self.display().map(|display| display.id()),
            is_minimized: false,
            is_maximized: self.is_maximized(),
            is_fullscreen: self.is_fullscreen(),
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: false,
            focus_on_click: false,
            background_appearance: self.background_appearance(),
            topmost: false,
            taskbar_visible: false,
            is_active: false,
        }
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.0.borrow_mut().title = Some(title.to_owned());
    }

    fn get_title(&self) -> String {
        self.0.borrow().title.clone().unwrap_or_default()
    }

    fn set_background_appearance(&self, _background: WindowBackgroundAppearance) {}

    fn is_fullscreen(&self) -> bool {
        matches!(self.0.borrow().window_bounds, WindowBounds::Fullscreen(_))
    }

    fn on_request_frame(&self, _callback: Box<dyn Fn(RequestFrameOptions)>) {}

    fn on_input(&self, callback: PlatformInputCallback) {
        let input = self.0.borrow().input_callback.clone();
        input.set(callback);
    }

    fn on_active_status_change(
        &self,
        _callback: Box<dyn Fn(PlatformWindowActiveStatusObservation)>,
    ) {
    }

    fn on_hover_status_change(&self, _callback: Box<dyn Fn(bool)>) {}

    fn on_visibility_change(&self, _callback: Box<dyn FnMut(WindowVisibility)>) {}

    fn on_resize(&self, _callback: Box<dyn Fn(Size<Pixels>, f32)>) {}

    fn on_moved(&self, _callback: Box<dyn Fn()>) {}

    fn on_display_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {}

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.borrow_mut().close_callback = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn Fn()>) {}

    fn draw(&self, _scene: &Scene) -> PlatformWindowPresentOutcome {
        PlatformWindowPresentOutcome::Submitted
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.borrow().atlas.clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use open_gpui::{WindowActivationPolicy, WindowKind, point, px, size};

    #[test]
    fn headless_window_keeps_placement_without_a_physical_display() {
        let bounds = Bounds::new(point(px(-120.0), px(30.0)), size(px(960.0), px(720.0)));
        let window = HeadlessWindow::new(WindowParams {
            window_bounds: WindowBounds::Maximized(bounds),
            bounds,
            titlebar: None,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            accepts_pointer_input: true,
            focus_on_appearing: false,
            activation_policy: WindowActivationPolicy::default(),
            transient_for: None,
            show: false,
            provisional_session: None,
            icon: None,
            display_id: None,
            window_min_size: None,
        });

        assert!(window.display().is_none());
        assert_eq!(window.platform_facts().display_id, None);
        assert_eq!(window.platform_facts().physical_geometry, None);
        assert_eq!(window.bounds(), bounds);
        assert_eq!(window.window_bounds(), WindowBounds::Maximized(bounds));
        assert_eq!(window.content_size(), bounds.size);
        assert_eq!(window.scale_factor(), 1.0);
        assert!(window.is_maximized());
        assert_eq!(window.visibility(), WindowVisibility::Hidden);
        assert!(!window.is_visible());
    }
}
