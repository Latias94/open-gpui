use crate::interaction::DockRuntimeDragSession;
use open_gpui::AnyWindowHandle;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportWindowEffects {
    close_now: Vec<AnyWindowHandle>,
    refresh: Vec<AnyWindowHandle>,
    close_after_current_effect: Vec<AnyWindowHandle>,
}

impl DockViewportWindowEffects {
    pub(crate) fn new(
        close_now: impl IntoIterator<Item = AnyWindowHandle>,
        refresh: impl IntoIterator<Item = AnyWindowHandle>,
        close_after_current_effect: impl IntoIterator<Item = AnyWindowHandle>,
    ) -> Self {
        let mut effects = Self::default();
        extend_unique_windows(&mut effects.close_now, close_now);
        extend_unique_windows(&mut effects.refresh, refresh);
        extend_unique_windows(
            &mut effects.close_after_current_effect,
            close_after_current_effect,
        );
        effects
    }

    pub(crate) fn refresh_only(refresh: impl IntoIterator<Item = AnyWindowHandle>) -> Self {
        Self::new(Vec::new(), refresh, Vec::new())
    }

    pub(crate) fn close_now(&self) -> &[AnyWindowHandle] {
        &self.close_now
    }

    pub(crate) fn refresh(&self) -> &[AnyWindowHandle] {
        &self.refresh
    }

    pub(crate) fn close_after_current_effect(&self) -> &[AnyWindowHandle] {
        &self.close_after_current_effect
    }

    pub(crate) fn has_effects(&self) -> bool {
        !self.close_now.is_empty()
            || !self.refresh.is_empty()
            || !self.close_after_current_effect.is_empty()
    }
}

#[derive(Debug, Default)]
pub(crate) struct DockViewportRuntimeUpdate {
    changed: bool,
    windows: Vec<AnyWindowHandle>,
    pointer_input_sync: Option<DockViewportPointerInputSyncRequest>,
}

impl DockViewportRuntimeUpdate {
    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn mark_changed(&mut self, changed: bool) {
        self.changed |= changed;
    }

    pub(crate) fn extend_windows(&mut self, windows: impl IntoIterator<Item = AnyWindowHandle>) {
        extend_unique_windows(&mut self.windows, windows);
    }

    pub(crate) fn set_pointer_input_sync(
        &mut self,
        pointer_input_sync: Option<DockViewportPointerInputSyncRequest>,
    ) {
        if let Some(next) = pointer_input_sync {
            debug_assert!(
                self.pointer_input_sync.is_none() || self.pointer_input_sync == Some(next)
            );
            if self.pointer_input_sync.is_none() {
                self.pointer_input_sync = Some(next);
            }
        }
    }

    pub(crate) fn merge(&mut self, update: DockViewportRuntimeUpdate) {
        self.mark_changed(update.changed);
        self.extend_windows(update.windows);
        self.set_pointer_input_sync(update.pointer_input_sync);
    }

    pub(crate) fn pointer_input_sync(&self) -> Option<DockViewportPointerInputSyncRequest> {
        self.pointer_input_sync
    }

    pub(crate) fn without_pointer_input_sync(mut self) -> Self {
        self.pointer_input_sync = None;
        self
    }

    pub(crate) fn into_windows(self) -> Vec<AnyWindowHandle> {
        self.windows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportPointerInputSyncRequest {
    window: AnyWindowHandle,
    /// Desired live platform state. Route facts only change after a later window-facts refresh
    /// observes whether the backend actually applied this request.
    accepts_pointer_input: bool,
}

pub(crate) struct DockViewportPayloadDragBegin {
    pub(crate) session: DockRuntimeDragSession,
    pub(crate) pointer_input_sync: Option<DockViewportPointerInputSyncRequest>,
}

impl DockViewportPointerInputSyncRequest {
    pub(crate) fn new(window: AnyWindowHandle, accepts_pointer_input: bool) -> Self {
        Self {
            window,
            accepts_pointer_input,
        }
    }

    pub(crate) fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn requested_accepts_pointer_input(&self) -> bool {
        self.accepts_pointer_input
    }
}

pub(crate) fn extend_unique_windows(
    windows: &mut Vec<AnyWindowHandle>,
    next_windows: impl IntoIterator<Item = AnyWindowHandle>,
) {
    for window in next_windows {
        if windows
            .iter()
            .any(|existing| existing.window_id() == window.window_id())
        {
            continue;
        }
        windows.push(window);
    }
}
