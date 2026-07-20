use std::{cell::Cell, mem, rc::Rc};

use thiserror::Error;

use crate::{
    App, HitboxId, MouseButton, MouseMoveEvent, MouseUpEvent, PlatformInput, PointerCancelEvent,
    PointerCancelReason, Window, WindowId,
};

use super::{Frame, PendingPointerCancellation};

/// A stable identifier for a pointer capture owner within one window.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PointerCaptureId(pub(super) u64);

impl PointerCaptureId {
    /// Converts this pointer capture ID to a `u64`.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// A stable, window-owned handle used to keep pointer capture across redraws.
///
/// The handle must be bound to exactly one hitbox in every frame where its owner is rendered.
/// Pointer capture is automatically released when a frame no longer contains that binding.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PointerCaptureHandle {
    window_id: WindowId,
    id: PointerCaptureId,
}

/// The owner and initiating button of an active pointer-capture session.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PointerCapture {
    handle: PointerCaptureHandle,
    button: MouseButton,
}

impl PointerCapture {
    /// Returns the stable handle that owns this capture session.
    pub fn handle(self) -> PointerCaptureHandle {
        self.handle
    }

    /// Returns the mouse button whose release terminates this capture session.
    pub fn button(self) -> MouseButton {
        self.button
    }
}

impl PointerCaptureHandle {
    /// Returns the stable ID of this pointer capture owner.
    pub fn id(self) -> PointerCaptureId {
        self.id
    }

    /// Returns the window that created this handle.
    pub fn window_id(self) -> WindowId {
        self.window_id
    }

    /// Returns whether this handle currently owns pointer capture in the given window.
    pub fn is_captured(self, window: &Window) -> bool {
        window
            .captured_pointer
            .is_some_and(|captured| captured.handle == self)
            && self.window_id == window.handle.window_id()
    }
}

/// An error produced while binding or changing pointer capture.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub enum PointerCaptureError {
    /// The handle was used with a window other than the one that created it.
    #[error(
        "pointer capture handle belongs to window {handle_window:?}, not window {target_window:?}"
    )]
    WrongWindow {
        /// The window that created the handle.
        handle_window: WindowId,
        /// The window on which the operation was attempted.
        target_window: WindowId,
    },
    /// The stable handle was already bound in the current frame.
    #[error("pointer capture handle {handle:?} is already bound in the current frame")]
    HandleAlreadyBound {
        /// The duplicate handle.
        handle: PointerCaptureHandle,
    },
    /// The hitbox was already bound to another stable handle in the current frame.
    #[error("hitbox {hitbox:?} is already bound to a pointer capture handle")]
    HitboxAlreadyBound {
        /// The duplicate hitbox.
        hitbox: HitboxId,
    },
    /// The hitbox does not belong to the frame currently being built.
    #[error("hitbox {hitbox:?} does not belong to the frame currently being built")]
    HitboxNotInCurrentFrame {
        /// The stale or unknown hitbox.
        hitbox: HitboxId,
    },
    /// The handle has no binding in the current interactive frame.
    #[error("pointer capture handle {handle:?} is not bound in the current interactive frame")]
    HandleNotBound {
        /// The unbound handle.
        handle: PointerCaptureHandle,
    },
    /// Pointer capture cannot begin while the target window is inactive.
    #[error("pointer capture cannot begin while window {window:?} is inactive")]
    WindowInactive {
        /// The inactive window.
        window: WindowId,
    },
    /// The requested button is not part of the window's current pressed-button set.
    #[error("pointer capture cannot begin for released button {button:?}")]
    ButtonNotPressed {
        /// The button that was expected to be pressed.
        button: MouseButton,
    },
    /// Another stable handle already owns the active pointer-capture session.
    #[error("pointer capture is already owned by {captured:?}, not {requested:?}")]
    PointerAlreadyCaptured {
        /// The existing capture owner.
        captured: PointerCaptureHandle,
        /// The handle that attempted to replace it.
        requested: PointerCaptureHandle,
    },
}

#[derive(Copy, Clone, Default)]
pub(super) struct PressedMouseButtons(u8);

impl PressedMouseButtons {
    pub(super) fn insert(&mut self, button: MouseButton) {
        self.0 |= Self::mask(button);
    }

    pub(super) fn remove(&mut self, button: MouseButton) {
        self.0 &= !Self::mask(button);
    }

    pub(super) fn contains(self, button: MouseButton) -> bool {
        self.0 & Self::mask(button) != 0
    }

    pub(super) fn clear(&mut self) {
        self.0 = 0;
    }

    pub(super) fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn mask(button: MouseButton) -> u8 {
        match button {
            MouseButton::Left => 1 << 0,
            MouseButton::Right => 1 << 1,
            MouseButton::Middle => 1 << 2,
            MouseButton::Navigate(crate::NavigationDirection::Back) => 1 << 3,
            MouseButton::Navigate(crate::NavigationDirection::Forward) => 1 << 4,
        }
    }
}

pub(super) struct InputDispatchGuard {
    dispatch_active: Rc<Cell<bool>>,
}

impl InputDispatchGuard {
    pub(super) fn try_enter(dispatch_active: Rc<Cell<bool>>) -> Option<Self> {
        if dispatch_active.replace(true) {
            return None;
        }

        Some(Self { dispatch_active })
    }
}

impl Drop for InputDispatchGuard {
    fn drop(&mut self) {
        self.dispatch_active.set(false);
    }
}

pub(super) struct MouseEventTargetGuard {
    target: Rc<Cell<Option<HitboxId>>>,
}

impl MouseEventTargetGuard {
    pub(super) fn enter(target: Rc<Cell<Option<HitboxId>>>, hitbox: HitboxId) -> Self {
        debug_assert!(target.get().is_none());
        target.set(Some(hitbox));
        Self { target }
    }
}

impl Drop for MouseEventTargetGuard {
    fn drop(&mut self) {
        self.target.set(None);
    }
}

impl Window {
    /// Creates a stable pointer capture handle owned by this window.
    ///
    /// Bind the handle to its owner's hitbox in every rendered frame with
    /// [`crate::InteractiveElement::track_pointer_capture`] or [`Window::bind_pointer_capture`].
    pub fn new_pointer_capture_handle(&mut self) -> PointerCaptureHandle {
        let id = self.next_pointer_capture_id;
        self.next_pointer_capture_id = id.next();
        PointerCaptureHandle {
            window_id: self.handle.window_id(),
            id,
        }
    }

    /// Captures the pointer for a stable handle bound in the current interactive frame.
    ///
    /// While captured, the handle's current-frame hitbox is the exclusive event target for mouse
    /// move, up, pressure, and cancellation listeners, regardless of pointer position. Physical
    /// hover remains attached to the pointer. Capture survives redraws as long as each new frame
    /// binds the same handle, and is released when the initiating button is released.
    pub fn capture_pointer(
        &mut self,
        handle: &PointerCaptureHandle,
        button: MouseButton,
    ) -> Result<(), PointerCaptureError> {
        self.ensure_pointer_capture_window(handle)?;
        if !self.subtree_presentation().is_interactive() {
            return Err(PointerCaptureError::HandleNotBound { handle: *handle });
        }
        if !self.is_window_active() {
            return Err(PointerCaptureError::WindowInactive {
                window: self.handle.window_id(),
            });
        }
        if !self.pressed_mouse_buttons.contains(button) {
            return Err(PointerCaptureError::ButtonNotPressed { button });
        }
        let frame = self.current_interaction_frame();
        let bound_hitbox = frame
            .pointer_capture_bindings
            .iter()
            .find_map(|(id, hitbox)| (*id == handle.id).then_some(*hitbox))
            .filter(|hitbox| {
                frame
                    .hitboxes
                    .iter()
                    .any(|candidate| candidate.id == *hitbox && candidate.is_active())
            });
        if bound_hitbox.is_none() {
            return Err(PointerCaptureError::HandleNotBound { handle: *handle });
        }
        if let Some(captured) = self.captured_pointer {
            if captured.handle == *handle && captured.button == button {
                return Ok(());
            }
            return Err(PointerCaptureError::PointerAlreadyCaptured {
                captured: captured.handle,
                requested: *handle,
            });
        }
        self.captured_pointer = Some(PointerCapture {
            handle: *handle,
            button,
        });
        Ok(())
    }

    /// Releases pointer capture when it is currently owned by the given handle.
    ///
    /// Returns `true` when capture was released and `false` when no capture was owned by this
    /// handle.
    pub fn release_pointer(
        &mut self,
        handle: &PointerCaptureHandle,
    ) -> Result<bool, PointerCaptureError> {
        self.ensure_pointer_capture_window(handle)?;
        if self
            .captured_pointer
            .is_some_and(|captured| captured.handle == *handle)
        {
            self.captured_pointer = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn finish_drag_source(
        &mut self,
        handle: &PointerCaptureHandle,
        button: MouseButton,
    ) -> Result<bool, PointerCaptureError> {
        self.ensure_pointer_capture_window(handle)?;
        let capture_released = if self
            .captured_pointer
            .is_some_and(|captured| captured.handle == *handle)
        {
            self.captured_pointer = None;
            true
        } else {
            false
        };
        let button_released = self.pressed_mouse_buttons.contains(button);
        self.pressed_mouse_buttons.remove(button);
        Ok(button_released || capture_released)
    }

    /// Returns the active pointer-capture session, if any.
    pub fn captured_pointer(&self) -> Option<PointerCapture> {
        self.captured_pointer
    }

    /// Returns whether this window owns an in-progress pressed, captured, or drag session.
    pub fn has_active_pointer_session(&self, cx: &App) -> bool {
        !self.pressed_mouse_buttons.is_empty()
            || self.captured_pointer.is_some()
            || cx
                .active_drag
                .as_ref()
                .is_some_and(|drag| drag.window_id == self.handle.window_id())
    }

    pub(crate) fn has_pointer_capture(&self) -> bool {
        self.captured_pointer.is_some()
    }

    /// Dispatches one terminal cancellation and clears the current pointer session.
    ///
    /// Callers inside an input handler must defer this operation until the current dispatch ends.
    pub fn cancel_pointer_session(&mut self, reason: PointerCancelReason, cx: &mut App) {
        if self.flush_pending_pointer_cancellation(cx) {
            return;
        }
        if !self.has_active_pointer_session(cx) {
            return;
        }

        let _ = self.dispatch_event(
            PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
            cx,
        );
        self.clear_pointer_session(cx);
    }

    pub(super) fn clear_pointer_session(&mut self, cx: &mut App) {
        self.pressed_mouse_buttons.clear();
        self.captured_pointer = None;
        if cx.clear_active_drag_for_window(self.handle.window_id()) {
            self.refresh();
        }
    }

    fn ensure_pointer_capture_window(
        &self,
        handle: &PointerCaptureHandle,
    ) -> Result<(), PointerCaptureError> {
        let target_window = self.handle.window_id();
        if handle.window_id != target_window {
            return Err(PointerCaptureError::WrongWindow {
                handle_window: handle.window_id,
                target_window,
            });
        }
        Ok(())
    }

    pub(super) fn captured_pointer_hitbox(&self) -> Option<HitboxId> {
        self.captured_pointer_hitbox_in_frame(&self.rendered_frame)
    }

    pub(super) fn captured_pointer_hitbox_in_frame(&self, frame: &Frame) -> Option<HitboxId> {
        self.pointer_capture_hitbox_for_handle_in_frame(self.captured_pointer?.handle, frame)
    }

    pub(super) fn pointer_capture_hitbox_for_handle_in_frame(
        &self,
        handle: PointerCaptureHandle,
        frame: &Frame,
    ) -> Option<HitboxId> {
        if handle.window_id != self.handle.window_id() {
            return None;
        }
        let captured = handle.id;
        let hitbox = frame
            .pointer_capture_bindings
            .iter()
            .find_map(|(id, hitbox)| (*id == captured).then_some(*hitbox))?;
        frame
            .hitboxes
            .iter()
            .any(|candidate| candidate.id == hitbox && candidate.is_active())
            .then_some(hitbox)
    }

    pub(super) fn queue_pointer_session_cancellation(
        &mut self,
        owner: PointerCaptureHandle,
        reason: PointerCancelReason,
        cx: &mut App,
    ) {
        if self.pending_pointer_cancellation.is_some() {
            return;
        }
        let target = self.pointer_capture_hitbox_for_handle_in_frame(owner, &self.rendered_frame);

        if self
            .captured_pointer
            .is_some_and(|captured| captured.handle == owner)
        {
            self.captured_pointer = None;
        }
        if cx.active_drag.as_ref().is_some_and(|drag| {
            drag.window_id == self.handle.window_id() && drag.source == Some(owner)
        }) {
            cx.active_drag = None;
            self.refresh();
        }
        let mut listeners = self.rendered_frame.pointer_cancel_listeners.clone();
        listeners.retain(|output| output.value.is_some());
        self.pending_pointer_cancellation = Some(PendingPointerCancellation {
            event: PointerCancelEvent { reason },
            target,
            listeners,
        });

        let window = self.handle;
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    window.flush_pending_pointer_cancellation(cx);
                })
                .ok();
        });
    }

    pub(super) fn flush_pending_pointer_cancellation(&mut self, cx: &mut App) -> bool {
        let Some(pending) = self.pending_pointer_cancellation.take() else {
            return false;
        };

        let mut current_listeners = mem::replace(
            &mut self.rendered_frame.pointer_cancel_listeners,
            pending.listeners,
        );
        let _target = pending
            .target
            .map(|target| MouseEventTargetGuard::enter(self.mouse_event_target.clone(), target));
        self.dispatch_event(PlatformInput::PointerCanceled(pending.event), cx);
        mem::swap(
            &mut self.rendered_frame.pointer_cancel_listeners,
            &mut current_listeners,
        );
        true
    }

    /// Binds a stable pointer capture handle to a hitbox in the frame being built.
    ///
    /// Each handle and hitbox may appear in at most one binding per frame. Custom elements should
    /// call this during prepaint after [`Window::insert_hitbox`]. Standard interactive elements can
    /// use [`crate::InteractiveElement::track_pointer_capture`] instead.
    pub fn bind_pointer_capture(
        &mut self,
        handle: &PointerCaptureHandle,
        hitbox: HitboxId,
    ) -> Result<(), PointerCaptureError> {
        self.invalidator.debug_assert_prepaint();
        self.ensure_pointer_capture_window(handle)?;
        if !self.subtree_presentation().is_interactive() {
            return Ok(());
        }

        if !self
            .next_frame
            .hitboxes
            .iter()
            .any(|entry| entry.id == hitbox)
        {
            return Err(PointerCaptureError::HitboxNotInCurrentFrame { hitbox });
        }
        if self
            .next_frame
            .pointer_capture_bindings
            .iter()
            .any(|(id, _)| *id == handle.id)
        {
            return Err(PointerCaptureError::HandleAlreadyBound { handle: *handle });
        }
        if self
            .next_frame
            .pointer_capture_bindings
            .iter()
            .any(|(_, bound_hitbox)| *bound_hitbox == hitbox)
        {
            return Err(PointerCaptureError::HitboxAlreadyBound { hitbox });
        }

        self.next_frame
            .pointer_capture_bindings
            .push((handle.id, hitbox));
        Ok(())
    }

    pub(super) fn finish_mouse_session_event(&mut self, event: &dyn std::any::Any, cx: &mut App) {
        if event.is::<PointerCancelEvent>() {
            self.clear_pointer_session(cx);
            return;
        }

        let window_id = self.handle.window_id();
        if event.is::<MouseMoveEvent>()
            && cx
                .active_drag
                .as_ref()
                .is_some_and(|drag| drag.window_id == window_id)
        {
            self.refresh();
        }

        if let Some(event) = event.downcast_ref::<MouseUpEvent>() {
            if cx
                .active_drag
                .as_ref()
                .is_some_and(|drag| drag.window_id == window_id && drag.button == event.button)
            {
                cx.active_drag = None;
                self.refresh();
            }

            if self
                .captured_pointer
                .is_some_and(|captured| captured.button == event.button)
            {
                self.captured_pointer = None;
            }
            self.pressed_mouse_buttons.remove(event.button);
        }
    }
}
