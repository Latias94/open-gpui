use open_gpui::{
    Context, DispatchPhase, IntoElement, MouseButton, PointerCaptureHandle, Window, WindowId,
    canvas, prelude::*,
};

pub(crate) struct PointerDrag<T> {
    binding: Option<(WindowId, PointerCaptureHandle)>,
    pub(crate) state: Option<T>,
}

impl<T> Default for PointerDrag<T> {
    fn default() -> Self {
        Self {
            binding: None,
            state: None,
        }
    }
}

impl<T> PointerDrag<T> {
    pub(crate) fn bind(&mut self, window: &mut Window) -> PointerCaptureHandle {
        let id = window.window_handle().window_id();
        if let Some((bound, handle)) = self.binding
            && bound == id
        {
            return handle;
        }
        let handle = window.new_pointer_capture_handle();
        self.state = None;
        self.binding = Some((id, handle));
        handle
    }

    pub(crate) fn begin(&mut self, state: T, window: &mut Window) -> bool {
        let Some((_, handle)) = self.binding else {
            return false;
        };
        if self.state.is_some() || window.capture_pointer(&handle, MouseButton::Left).is_err() {
            return false;
        }
        self.state = Some(state);
        true
    }

    pub(crate) fn take(&mut self, window: &mut Window) -> Option<T> {
        let (id, handle) = self.binding?;
        if id != window.window_handle().window_id() {
            return None;
        }
        let state = self.state.take()?;
        let _ = window.release_pointer(&handle);
        Some(state)
    }
}

pub(crate) fn on_pointer_cancel<T: 'static>(
    cx: &Context<T>,
    cancel: fn(&mut T, &mut Window, &mut Context<T>),
) -> impl IntoElement {
    let view = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            window.on_pointer_cancel(move |_, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    let _ = view.update(cx, |view, cx| cancel(view, window, cx));
                }
            });
        },
    )
    .absolute()
    .size_full()
}
