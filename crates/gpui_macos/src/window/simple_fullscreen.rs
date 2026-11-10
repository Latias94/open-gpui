use super::*;
use objc2_app_kit::NSApplicationPresentationOptions;

#[derive(Clone, Copy, Debug)]
pub(super) struct SimpleFullscreenState {
    frame: NSRect,
    pub(super) bounds: Bounds<Pixels>,
    style_mask: NSWindowStyleMask,
}

enum SimpleFullscreenPlan {
    Enter { screen_frame: NSRect },
    Exit(SimpleFullscreenState),
}

struct SimpleFullscreenAppState {
    window_count: usize,
    saved_presentation_options: NSUInteger,
}

impl SimpleFullscreenAppState {
    const fn new() -> Self {
        Self {
            window_count: 0,
            saved_presentation_options: 0,
        }
    }

    fn push(&mut self, presentation_options: NSUInteger) -> bool {
        let is_first = self.window_count == 0;
        if is_first {
            self.saved_presentation_options = presentation_options;
        }
        self.window_count += 1;
        is_first
    }

    fn pop(&mut self) -> Option<NSUInteger> {
        self.window_count = self.window_count.checked_sub(1)?;
        (self.window_count == 0).then_some(self.saved_presentation_options)
    }
}

static SIMPLE_FULLSCREEN_APP_STATE: Mutex<SimpleFullscreenAppState> =
    Mutex::new(SimpleFullscreenAppState::new());

fn set_simple_fullscreen_presentation_options(presentation_options: NSUInteger) {
    // SAFETY: Simple fullscreen and native close run on AppKit's main thread. No window or
    // application-presentation state lock is held while AppKit can invoke delegate callbacks.
    unsafe {
        let app = NSApplication::sharedApplication(nil);
        let _: () = msg_send![app, setPresentationOptions: presentation_options];
    }
}

pub(super) fn release_simple_fullscreen_presentation_options(saved: Option<SimpleFullscreenState>) {
    if saved.is_none() {
        return;
    }
    let presentation_options = SIMPLE_FULLSCREEN_APP_STATE.lock().pop();
    if let Some(presentation_options) = presentation_options {
        set_simple_fullscreen_presentation_options(presentation_options);
    }
}

fn native_fullscreen_blocks_simple_fullscreen(
    is_fullscreen: bool,
    transition: Option<MacFullscreenTransition>,
    pending_restore_bounds: Option<Bounds<Pixels>>,
) -> bool {
    is_fullscreen || transition.is_some() || pending_restore_bounds.is_some()
}

fn simple_fullscreen_style_mask(saved_style_mask: NSWindowStyleMask) -> NSWindowStyleMask {
    NSWindowStyleMask::NSBorderlessWindowMask
        | (saved_style_mask & NSWindowStyleMaskNonactivatingPanel)
}

pub(super) fn window_bounds_from_client_bounds(
    bounds: Bounds<Pixels>,
    is_fullscreen: bool,
    is_maximized: bool,
    fullscreen_restore_bounds: Bounds<Pixels>,
    windowed_restore_bounds: Bounds<Pixels>,
    simple_fullscreen_restore_bounds: Option<Bounds<Pixels>>,
) -> WindowBounds {
    if is_fullscreen {
        WindowBounds::Fullscreen(fullscreen_restore_bounds)
    } else if let Some(saved_bounds) = simple_fullscreen_restore_bounds {
        WindowBounds::Windowed(saved_bounds)
    } else if is_maximized {
        WindowBounds::Maximized(windowed_restore_bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

struct SimpleFullscreenTarget {
    window_state: Weak<Mutex<MacWindowState>>,
    native_window: Retained<NativeWindow>,
    native_view: Retained<Objc2Object>,
    handle: AnyWindowHandle,
}

impl SimpleFullscreenTarget {
    fn capture(window_state: &Arc<Mutex<MacWindowState>>) -> Option<Self> {
        let state = window_state.lock();
        if !MacNativeWindowHandleRequirement::InteractionAdmitted
            .accepts_lifecycle(state.is_closed(), state.interaction_is_quiesced())
            || native_fullscreen_blocks_simple_fullscreen(
                state.is_fullscreen(),
                state.fullscreen_transition,
                state.pending_fullscreen_restore_bounds,
            )
        {
            return None;
        }
        // SAFETY: Both objects belong to this live window. Retaining them under the state lock
        // keeps them alive across reentrant AppKit calls without extending command-queue lifetime.
        unsafe {
            Some(Self {
                window_state: Arc::downgrade(window_state),
                native_window: Retained::retain(state.native_window.cast())?,
                native_view: Retained::retain(state.native_view.as_ptr().cast())?,
                handle: state.handle,
            })
        }
    }

    fn native_id(&self) -> id {
        Retained::as_ptr(&self.native_window).cast_mut().cast()
    }

    fn satisfies(&self, requirement: MacNativeWindowHandleRequirement) -> bool {
        let Some(window_state) = self.window_state.upgrade() else {
            return false;
        };
        let state = window_state.lock();
        state.native_window == self.native_id()
            && state.handle == self.handle
            && requirement.accepts_lifecycle(state.is_closed(), state.interaction_is_quiesced())
    }

    fn apply_frame(
        &self,
        style_mask: NSWindowStyleMask,
        frame: NSRect,
        requirement: MacNativeWindowHandleRequirement,
    ) -> bool {
        if !self.satisfies(requirement) {
            return false;
        }
        // SAFETY: The window is retained and was checked against its original live handle.
        // Each setter may invoke AppKit delegates, so no state lock spans either call.
        unsafe {
            let _: () = msg_send![self.native_id(), setStyleMask: style_mask.bits()];
        }
        if !self.satisfies(requirement) {
            return false;
        }
        // SAFETY: Revalidate after the style setter, which may synchronously close the window.
        unsafe {
            let _: () = msg_send![self.native_id(), setFrame: frame display: YES];
        }
        self.satisfies(requirement)
    }

    fn restore_keyboard_input(&self, window_state: &Arc<Mutex<MacWindowState>>) {
        if !self.satisfies(MacNativeWindowHandleRequirement::InteractionAdmitted) {
            return;
        }
        if !activate_mac_window(window_state) {
            return;
        }
        if !self.satisfies(MacNativeWindowHandleRequirement::InteractionAdmitted) {
            return;
        }
        // Changing the style mask makes AppKit resign the window's key status and
        // first responder, so keyboard input stops reaching the editor. Re-make the
        // window key and restore the GPUI view as first responder.
        // SAFETY: Both original objects are retained, the full handle is still admitted, and
        // activation has passed through the existing transient-owner and activation-policy gate.
        unsafe {
            let native_view = Retained::as_ptr(&self.native_view).cast_mut();
            let _: BOOL = msg_send![self.native_id(), makeFirstResponder: native_view];
        }
    }
}

impl MacWindow {
    pub(super) fn toggle_simple_fullscreen_native(&self) -> bool {
        let Some(target) = SimpleFullscreenTarget::capture(&self.0) else {
            return false;
        };
        request_window_observation_commit_for_retained_topology(&self.0, None, true);
        if !target.satisfies(MacNativeWindowHandleRequirement::InteractionAdmitted) {
            return false;
        }

        // SAFETY: The original native window is retained and admitted. These AppKit getters run
        // outside the window and application-presentation locks.
        let (frame, style_mask, screen_frame, presentation_options) = unsafe {
            let native_window = target.native_id();
            let style_mask = NSWindow::styleMask(native_window);
            if style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask) {
                return false;
            }
            let screen = NSWindow::screen(native_window);
            let screen_frame = (!screen.is_null()).then(|| NSScreen::frame(screen));
            let app = NSApplication::sharedApplication(nil);
            let presentation_options: NSUInteger = msg_send![app, presentationOptions];
            (
                NSWindow::frame(native_window),
                style_mask,
                screen_frame,
                presentation_options,
            )
        };
        let (plan, is_first_window) = {
            let mut state = self.0.lock();
            if state.native_window != target.native_id()
                || state.handle != target.handle
                || state.is_closed()
                || state.interaction_is_quiesced()
                || native_fullscreen_blocks_simple_fullscreen(
                    state.is_fullscreen(),
                    state.fullscreen_transition,
                    state.pending_fullscreen_restore_bounds,
                )
            {
                return false;
            }
            if let Some(saved) = state.simple_fullscreen_state {
                (SimpleFullscreenPlan::Exit(saved), false)
            } else {
                let Some(screen_frame) = screen_frame.filter(|frame| native_rect_is_valid(*frame))
                else {
                    return false;
                };
                if !native_rect_is_valid(frame) {
                    return false;
                }
                let Ok(observation) =
                    state.native_observation_for_generation(state.observation_target_generation())
                else {
                    return false;
                };
                if observation.style_mask != style_mask.bits() {
                    return false;
                }
                state.simple_fullscreen_state = Some(SimpleFullscreenState {
                    frame,
                    bounds: observation.bounds,
                    style_mask,
                });
                let is_first_window = SIMPLE_FULLSCREEN_APP_STATE
                    .lock()
                    .push(presentation_options);
                (
                    SimpleFullscreenPlan::Enter { screen_frame },
                    is_first_window,
                )
            }
        };

        match plan {
            SimpleFullscreenPlan::Enter { screen_frame } => {
                if is_first_window {
                    set_simple_fullscreen_presentation_options(
                        (NSApplicationPresentationOptions::AutoHideDock
                            | NSApplicationPresentationOptions::AutoHideMenuBar)
                            .bits() as NSUInteger,
                    );
                }
                let applied = target.apply_frame(
                    simple_fullscreen_style_mask(style_mask),
                    screen_frame,
                    MacNativeWindowHandleRequirement::InteractionAdmitted,
                );
                if applied {
                    target.restore_keyboard_input(&self.0);
                }
                if !applied
                    || !target.satisfies(MacNativeWindowHandleRequirement::InteractionAdmitted)
                {
                    let saved = self.0.lock().simple_fullscreen_state;
                    if let Some(saved) = saved {
                        target.apply_frame(
                            saved.style_mask,
                            saved.frame,
                            MacNativeWindowHandleRequirement::ExactLive,
                        );
                    }
                    self.finish_simple_fullscreen(&target);
                    request_window_observation_commit_for_retained_topology(&self.0, None, true);
                    notify_window_state_changed(&self.0);
                    return false;
                }
            }
            SimpleFullscreenPlan::Exit(saved) => {
                target.apply_frame(
                    saved.style_mask,
                    saved.frame,
                    MacNativeWindowHandleRequirement::ExactLive,
                );
                self.finish_simple_fullscreen(&target);
                target.restore_keyboard_input(&self.0);
            }
        }

        request_window_observation_commit_for_retained_topology(&self.0, None, true);
        notify_window_state_changed(&self.0);
        target.satisfies(MacNativeWindowHandleRequirement::InteractionAdmitted)
    }

    fn finish_simple_fullscreen(&self, target: &SimpleFullscreenTarget) {
        let saved = {
            let mut state = self.0.lock();
            if state.handle != target.handle || state.native_window != target.native_id() {
                return;
            }
            state.simple_fullscreen_state.take()
        };
        release_simple_fullscreen_presentation_options(saved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_state() -> SimpleFullscreenState {
        SimpleFullscreenState {
            frame: NSRect::new(NSPoint::new(-1200.0, 80.0), NSSize::new(900.0, 640.0)),
            bounds: Bounds::new(point(px(-1200.0), px(180.0)), size(px(900.0), px(640.0))),
            style_mask: NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSResizableWindowMask,
        }
    }

    #[test]
    fn simple_fullscreen_command_rejects_an_expired_native_owner() {
        assert_eq!(
            dispatch_mac_window_command(
                &Weak::new(),
                PlatformWindowCommand::ToggleSimpleFullscreen,
            ),
            PlatformWindowCommandOutcome::WindowClosed,
        );
    }

    #[test]
    fn simple_fullscreen_persists_saved_windowed_bounds_despite_native_zoom() {
        let saved = saved_state();
        let current = Bounds::new(point(px(-1440.0), px(0.0)), size(px(1440.0), px(900.0)));
        let restore = Bounds::new(point(px(20.0), px(40.0)), size(px(800.0), px(600.0)));

        for is_maximized in [false, true] {
            assert_eq!(
                window_bounds_from_client_bounds(
                    current,
                    false,
                    is_maximized,
                    restore,
                    restore,
                    Some(saved.bounds),
                ),
                WindowBounds::Windowed(saved.bounds),
            );
        }
        assert_eq!(
            window_bounds_from_client_bounds(current, false, true, restore, restore, None),
            WindowBounds::Maximized(restore),
        );
        assert_eq!(
            window_bounds_from_client_bounds(
                current,
                true,
                false,
                restore,
                restore,
                Some(saved.bounds),
            ),
            WindowBounds::Fullscreen(restore),
        );
    }

    #[test]
    fn native_fullscreen_transitions_block_simple_fullscreen_without_a_style_bit() {
        for transition in [
            MacFullscreenTransition::Entering,
            MacFullscreenTransition::Exiting,
        ] {
            assert!(native_fullscreen_blocks_simple_fullscreen(
                false,
                Some(transition),
                None,
            ));
        }
        assert!(native_fullscreen_blocks_simple_fullscreen(
            false,
            None,
            Some(saved_state().bounds),
        ));
        assert!(native_fullscreen_blocks_simple_fullscreen(true, None, None));
        assert!(!native_fullscreen_blocks_simple_fullscreen(
            false, None, None
        ));
    }

    #[test]
    fn borderless_presentation_preserves_the_native_nonactivating_panel_policy() {
        let saved = saved_state();
        assert_eq!(
            simple_fullscreen_style_mask(saved.style_mask),
            NSWindowStyleMask::NSBorderlessWindowMask,
        );
        assert_eq!(
            simple_fullscreen_style_mask(saved.style_mask | NSWindowStyleMaskNonactivatingPanel),
            NSWindowStyleMaskNonactivatingPanel,
        );
    }

    #[test]
    fn presentation_options_restore_only_after_the_last_exact_owner_releases() {
        let mut state = SimpleFullscreenAppState::new();
        assert!(state.push(17));
        assert!(!state.push(23));
        assert_eq!(state.pop(), None);
        assert!(!state.push(29));
        assert_eq!(state.pop(), None);
        assert_eq!(state.pop(), Some(17));
        assert_eq!(state.pop(), None);
        assert!(state.push(31));
        assert_eq!(state.pop(), Some(31));
    }

    #[test]
    fn quiesced_targets_keep_restore_authority_without_keyboard_or_drag_authority() {
        assert!(MacNativeWindowHandleRequirement::ExactLive.accepts_lifecycle(false, true));
        assert!(
            !MacNativeWindowHandleRequirement::InteractionAdmitted.accepts_lifecycle(false, true)
        );
        assert!(!MacNativeWindowHandleRequirement::ExactLive.accepts_lifecycle(true, true));
    }
}
