use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use crate::{
    App, Bounds, FileDropEvent, NativeCapturedDragGeneration, PlatformExternalDragSession,
    PlatformInput, Point, Window,
};

impl Window {
    pub(super) fn promote_external_drag_to_platform(
        &mut self,
        event: &PlatformInput,
        cx: &mut App,
    ) {
        let PlatformInput::MouseMove(mouse_move) = event else {
            return;
        };
        if mouse_move.pressed_button != Some(crate::MouseButton::Left)
            || Bounds::new(Point::default(), self.viewport_size).contains(&mouse_move.position)
            || !self.user_interaction_is_admitted()
            || !self.platform_window.can_start_external_drag()
        {
            return;
        }
        let source_window = self.handle.window_id();
        let Some(generation) = cx.active_drag_generation_for_window(source_window) else {
            return;
        };
        let Some(payload_source) = cx
            .active_drag
            .as_mut()
            .and_then(|drag| drag.external_payload_source.take())
        else {
            return;
        };
        let Some(payload) = payload_source(self, cx) else {
            return;
        };
        if !self.user_interaction_is_admitted()
            || cx.active_drag_generation_for_window(source_window) != Some(generation)
        {
            return;
        }
        let session = PlatformExternalDragSession::new(cx.to_async(), source_window, generation);
        if !self.platform_window.start_external_drag(&payload, session)
            || !self.user_interaction_is_admitted()
            || !cx.hand_active_drag_to_platform(source_window, generation)
        {
            return;
        }
        self.captured_pointer = None;
        self.pressed_mouse_buttons.clear();
        self.refresh();
    }

    pub(crate) fn dispatch_external_drag_event(
        &mut self,
        generation: NativeCapturedDragGeneration,
        event: FileDropEvent,
        cx: &mut App,
    ) -> bool {
        let source_window = self.handle.window_id();
        if !cx.platform_drag_matches(source_window, generation) {
            return false;
        }
        if matches!(event, FileDropEvent::Ended) {
            let ended = cx.end_platform_drag_exact(source_window, generation);
            if ended && self.user_interaction_is_admitted() {
                let _ = self.dispatch_event(PlatformInput::FileDrop(event), cx);
            }
            self.refresh();
            return ended;
        }
        if !cx.platform_drag_is_current(source_window, generation)
            || !self.user_interaction_is_admitted()
        {
            return false;
        }
        match event {
            FileDropEvent::Entered { .. } => {
                cx.restore_platform_drag(source_window, generation);
            }
            FileDropEvent::Exited => {
                cx.hand_restored_drag_to_platform(source_window, generation);
            }
            _ => {}
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.dispatch_event(PlatformInput::FileDrop(event), cx);
        }));
        if let Err(payload) = result {
            cx.end_platform_drag_exact(source_window, generation);
            resume_unwind(payload);
        }
        true
    }
}

#[cfg(test)]
mod tests;
