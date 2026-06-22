use open_gpui::{AnyWindowHandle, AppContext, WindowId};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(crate) struct DockViewportWindowOwnership {
    owned_windows: HashSet<WindowId>,
    retired_windows: HashSet<WindowId>,
    render_passthrough_windows: HashSet<WindowId>,
}

impl DockViewportWindowOwnership {
    pub(crate) fn register_runtime_window(&mut self, window_id: WindowId) {
        self.retired_windows.remove(&window_id);
        self.owned_windows.insert(window_id);
    }

    pub(crate) fn discard_owned_window(&mut self, window_id: WindowId) -> bool {
        let removed = self.owned_windows.remove(&window_id);
        if removed {
            self.retired_windows.insert(window_id);
        }
        self.render_passthrough_windows.remove(&window_id);
        removed
    }

    pub(crate) fn is_retired(&self, window_id: WindowId) -> bool {
        self.retired_windows.contains(&window_id)
    }

    pub(crate) fn is_owned(&self, window_id: WindowId) -> bool {
        self.owned_windows.contains(&window_id)
    }

    pub(crate) fn is_window_unrefreshable<C: AppContext>(
        &self,
        window: AnyWindowHandle,
        cx: &mut C,
    ) -> bool {
        window.update(cx, |_, _, _| ()).is_err()
    }

    pub(crate) fn is_unowned_window_unrefreshable<C: AppContext>(
        &self,
        window: AnyWindowHandle,
        cx: &mut C,
    ) -> bool {
        !self.is_owned(window.window_id()) && self.is_window_unrefreshable(window, cx)
    }

    pub(crate) fn record_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.render_passthrough_windows.insert(window_id)
    }

    pub(crate) fn take_render_passthrough_pointer_input(&mut self, window_id: WindowId) -> bool {
        self.render_passthrough_windows.remove(&window_id)
    }

    pub(crate) fn clear_window_state(&mut self, window_id: WindowId) {
        self.render_passthrough_windows.remove(&window_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_windows_are_retired_only_after_owned_discard() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(7);

        assert!(!ownership.is_owned(window_id));
        assert!(!ownership.discard_owned_window(window_id));
        assert!(!ownership.is_retired(window_id));

        ownership.register_runtime_window(window_id);
        assert!(ownership.is_owned(window_id));
        assert!(!ownership.is_retired(window_id));

        assert!(ownership.discard_owned_window(window_id));
        assert!(!ownership.is_owned(window_id));
        assert!(ownership.is_retired(window_id));

        ownership.register_runtime_window(window_id);
        assert!(ownership.is_owned(window_id));
        assert!(!ownership.is_retired(window_id));
    }

    #[test]
    fn clearing_window_state_removes_render_passthrough_record() {
        let mut ownership = DockViewportWindowOwnership::default();
        let window_id = WindowId::from(9);

        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        assert!(!ownership.record_render_passthrough_pointer_input(window_id));
        assert!(ownership.take_render_passthrough_pointer_input(window_id));
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));

        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        ownership.clear_window_state(window_id);
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));

        ownership.register_runtime_window(window_id);
        assert!(ownership.record_render_passthrough_pointer_input(window_id));
        assert!(ownership.discard_owned_window(window_id));
        assert!(!ownership.take_render_passthrough_pointer_input(window_id));
    }
}
