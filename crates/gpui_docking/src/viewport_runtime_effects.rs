use crate::interaction::DockRuntimeDragSession;
use open_gpui::{AnyWindowHandle, App};

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

    pub(crate) fn merge(mut self, other: Self) -> Self {
        extend_unique_windows(&mut self.close_now, other.close_now);
        extend_unique_windows(&mut self.refresh, other.refresh);
        extend_unique_windows(
            &mut self.close_after_current_effect,
            other.close_after_current_effect,
        );
        self
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

pub(crate) fn unique_windows(windows: Vec<AnyWindowHandle>) -> Vec<AnyWindowHandle> {
    let mut unique = Vec::new();
    extend_unique_windows(&mut unique, windows);
    unique
}

pub(crate) fn refresh_windows<C: open_gpui::AppContext>(windows: Vec<AnyWindowHandle>, cx: &mut C) {
    for window in unique_windows(windows) {
        let _ = window.update(cx, |_, window, _| window.refresh());
    }
}

pub(crate) fn refresh_runtime_update<C: open_gpui::AppContext>(
    update: DockViewportRuntimeUpdate,
    cx: &mut C,
) -> bool {
    let changed = update.changed();
    refresh_windows(update.into_windows(), cx);
    changed
}

pub(crate) fn close_window_quietly(window: AnyWindowHandle, cx: &mut App) {
    let _ = window.update(cx, |_, window, cx| window.remove_window(cx));
}

fn close_windows_quietly(windows: Vec<AnyWindowHandle>, cx: &mut App) {
    for window in windows {
        close_window_quietly(window, cx);
    }
}

fn close_windows_after_current_effect(windows: Vec<AnyWindowHandle>, cx: &mut App) {
    if windows.is_empty() {
        return;
    }
    cx.defer(move |cx| close_windows_quietly(windows, cx));
}

pub(crate) fn apply_viewport_window_effects(effects: DockViewportWindowEffects, cx: &mut App) {
    close_windows_after_current_effect(effects.close_now().to_vec(), cx);
    refresh_windows(effects.refresh().to_vec(), cx);
    close_windows_after_current_effect(effects.close_after_current_effect().to_vec(), cx);
}

pub(crate) fn refresh_viewport_window_effects<C: open_gpui::AppContext>(
    effects: DockViewportWindowEffects,
    cx: &mut C,
) {
    debug_assert!(effects.close_now().is_empty());
    debug_assert!(effects.close_after_current_effect().is_empty());
    refresh_windows(effects.refresh().to_vec(), cx);
}

#[cfg(test)]
mod tests {
    use super::unique_windows;
    use crate::viewport_test_support::handle;

    #[test]
    fn unique_windows_preserves_first_occurrence_order() {
        let first = handle(1);
        let second = handle(2);

        assert_eq!(
            unique_windows(vec![first, second, first, second, first]),
            vec![first, second]
        );
    }
}
