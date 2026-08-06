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

    pub(crate) fn can_commit_pointer_session_start(&self) -> bool {
        !self.removed && self.removal_state == super::WindowRemovalState::Open
    }

    pub(crate) fn owns_pointer_capture(
        &self,
        handle: PointerCaptureHandle,
        button: MouseButton,
    ) -> bool {
        self.captured_pointer
            .is_some_and(|captured| captured.handle == handle && captured.button == button)
            && self.pressed_mouse_buttons.contains(button)
    }

    /// Dispatches one terminal cancellation and clears the current pointer session.
    ///
    /// Callers inside an input handler must defer this operation until the current dispatch ends.
    pub fn cancel_pointer_session(&mut self, reason: PointerCancelReason, cx: &mut App) {
        self.flush_pending_pointer_cancellations(cx);
        if !self.has_active_pointer_session(cx) {
            return;
        }

        let _ = self.dispatch_event(
            PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
            cx,
        );
        if self.has_active_pointer_session(cx) {
            self.clear_pointer_session(reason, cx);
        }
    }

    pub(super) fn clear_pointer_session(&mut self, reason: PointerCancelReason, cx: &mut App) {
        let had_session = self.has_active_pointer_session(cx);
        let native_release = cx.reserve_active_native_captured_drag_pointer_cancellation(
            self.handle.window_id(),
            reason,
        );
        self.pressed_mouse_buttons.clear();
        self.captured_pointer = None;
        let (active_drag_cleared, captured_drag_generation) =
            cx.clear_active_drag_for_window(self.handle.window_id());
        if native_release.is_some() || active_drag_cleared {
            self.refresh();
        }
        let native_release = native_release.or_else(|| {
            if had_session {
                cx.reserve_native_pointer_capture_release(
                    self.handle.window_id(),
                    captured_drag_generation,
                )
            } else {
                None
            }
        });
        if let Some(release) = native_release {
            self.platform_command_sink
                .settle_pointer_capture_release(release, true);
        }
    }

    pub(crate) fn queue_native_captured_drag_pointer_cancellation(
        &mut self,
        owner: Option<PointerCaptureHandle>,
        button: MouseButton,
        reason: PointerCancelReason,
        native_release: crate::NativePointerCaptureReleaseToken,
        cx: &mut App,
    ) {
        self.refresh();
        self.queue_exact_pointer_session_cancellation(
            owner,
            button,
            reason,
            Some(native_release),
            true,
            cx,
        );
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

    pub(crate) fn queue_pointer_session_cancellation(
        &mut self,
        owner: PointerCaptureHandle,
        reason: PointerCancelReason,
        cx: &mut App,
    ) {
        self.queue_pointer_session_cancellation_with_refresh(owner, reason, true, cx);
    }

    pub(super) fn queue_candidate_pointer_session_cancellation(
        &mut self,
        owner: PointerCaptureHandle,
        reason: PointerCancelReason,
        cx: &mut App,
    ) {
        self.queue_pointer_session_cancellation_with_refresh(owner, reason, false, cx);
    }

    fn queue_pointer_session_cancellation_with_refresh(
        &mut self,
        owner: PointerCaptureHandle,
        reason: PointerCancelReason,
        refresh_after_drag_removal: bool,
        cx: &mut App,
    ) {
        let button = self
            .captured_pointer
            .filter(|captured| captured.handle == owner)
            .map(|captured| captured.button)
            .or_else(|| {
                cx.active_drag.as_ref().and_then(|drag| {
                    (drag.window_id == self.handle.window_id() && drag.source == Some(owner))
                        .then_some(drag.button)
                })
            });
        let Some(button) = button else {
            return;
        };
        let native_release =
            cx.reserve_native_pointer_capture_release(self.handle.window_id(), None);
        self.queue_exact_pointer_session_cancellation(
            Some(owner),
            button,
            reason,
            native_release,
            refresh_after_drag_removal,
            cx,
        );
    }

    fn queue_exact_pointer_session_cancellation(
        &mut self,
        owner: Option<PointerCaptureHandle>,
        button: MouseButton,
        reason: PointerCancelReason,
        native_release: Option<crate::NativePointerCaptureReleaseToken>,
        refresh_after_drag_removal: bool,
        cx: &mut App,
    ) {
        let target = owner.and_then(|owner| {
            self.pointer_capture_hitbox_for_handle_in_frame(owner, &self.rendered_frame)
        });
        let captured_owner_matches = self
            .captured_pointer
            .is_some_and(|captured| Some(captured.handle) == owner && captured.button == button);
        if captured_owner_matches {
            self.captured_pointer = None;
        }
        if cx.active_drag.as_ref().is_some_and(|drag| {
            drag.window_id == self.handle.window_id()
                && drag.button == button
                && (drag.source == owner || (drag.source.is_none() && captured_owner_matches))
        }) {
            cx.active_drag = None;
            cx.retire_native_captured_drag_authority();
            if refresh_after_drag_removal {
                self.refresh();
            }
        }
        let button_remains_owned = self
            .captured_pointer
            .is_some_and(|captured| captured.button == button)
            || cx.active_drag.as_ref().is_some_and(|drag| {
                drag.window_id == self.handle.window_id() && drag.button == button
            });
        if !button_remains_owned && self.pressed_mouse_buttons.contains(button) {
            self.pressed_mouse_buttons.remove(button);
        }
        let mut listeners = self.rendered_frame.pointer_cancel_listeners.clone();
        listeners.retain(|output| output.value.is_some());
        let schedule_flush = self.pending_pointer_cancellations.is_empty();
        self.pending_pointer_cancellations
            .push_back(PendingPointerCancellation {
                event: PointerCancelEvent { reason },
                target,
                listeners,
                native_release,
            });

        if schedule_flush {
            let window = self.handle;
            cx.defer(move |cx| {
                window
                    .update(cx, |_, window, cx| {
                        window.flush_pending_pointer_cancellations(cx);
                    })
                    .ok();
            });
        }
    }

    pub(crate) fn flush_pending_pointer_cancellations(&mut self, cx: &mut App) -> bool {
        let mut flushed = false;
        let mut first_panic = None;
        while let Some(pending) = self.pending_pointer_cancellations.pop_front() {
            flushed = true;
            let mut current_listeners = mem::replace(
                &mut self.rendered_frame.pointer_cancel_listeners,
                pending.listeners,
            );
            let target = pending.target.map(|target| {
                MouseEventTargetGuard::enter(self.mouse_event_target.clone(), target)
            });
            let previous_settlement =
                mem::replace(&mut self.pointer_cancel_session_already_settled, true);
            let dispatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.dispatch_event_without_pending_pointer_cancellations(
                    PlatformInput::PointerCanceled(pending.event),
                    cx,
                );
            }));
            self.pointer_cancel_session_already_settled = previous_settlement;
            drop(target);
            mem::swap(
                &mut self.rendered_frame.pointer_cancel_listeners,
                &mut current_listeners,
            );
            if let Some(release) = pending.native_release {
                self.platform_command_sink
                    .settle_pointer_capture_release(release, !self.has_active_pointer_session(cx));
            }
            if first_panic.is_none()
                && let Err(payload) = dispatch
            {
                first_panic = Some(payload);
            }
        }
        if let Some(payload) = first_panic {
            std::panic::resume_unwind(payload);
        }
        flushed
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
        if let Some(event) = event.downcast_ref::<PointerCancelEvent>() {
            if !self.pointer_cancel_session_already_settled {
                self.clear_pointer_session(event.reason, cx);
            }
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
                cx.retire_native_captured_drag_authority();
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
