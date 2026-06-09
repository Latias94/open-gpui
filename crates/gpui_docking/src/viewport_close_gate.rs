use crate::{DockViewportAdapter, DockViewportClosePolicy};
use open_gpui::WindowId;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

#[derive(Clone)]
pub(crate) struct DockViewportCloseGate {
    close_policy: Rc<Cell<DockViewportClosePolicy>>,
    known_windows: Rc<RefCell<HashSet<WindowId>>>,
}

impl DockViewportCloseGate {
    pub(crate) fn new(close_policy: DockViewportClosePolicy) -> Self {
        Self {
            close_policy: Rc::new(Cell::new(close_policy)),
            known_windows: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.get()
    }

    pub(crate) fn set_close_policy(&self, close_policy: DockViewportClosePolicy) {
        self.close_policy.set(close_policy);
    }

    pub(crate) fn sync_adapter(&self, adapter: &DockViewportAdapter) {
        *self.known_windows.borrow_mut() = adapter
            .spaces()
            .into_iter()
            .filter_map(|space| {
                adapter
                    .window_for_space(&space)
                    .map(|window| window.window_id())
            })
            .collect();
    }

    pub(crate) fn should_allow_close(&self, window_id: WindowId) -> bool {
        if !self.known_windows.borrow().contains(&window_id) {
            return true;
        }

        self.close_policy() != DockViewportClosePolicy::Prevent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockSpaceId, viewport_test_support::handle};

    #[test]
    fn close_gate_applies_policy_only_to_synced_adapter_windows() {
        let mut adapter = DockViewportAdapter::new();
        let window = handle(1);
        let unknown = WindowId::from(99);
        adapter.register_viewport(DockSpaceId::from("secondary"), window);

        let gate = DockViewportCloseGate::new(DockViewportClosePolicy::Prevent);
        gate.sync_adapter(&adapter);

        assert!(!gate.should_allow_close(window.window_id()));
        assert!(gate.should_allow_close(unknown));

        gate.set_close_policy(DockViewportClosePolicy::RetainLayout);
        assert!(gate.should_allow_close(window.window_id()));

        adapter.handle_window_closed(window.window_id());
        gate.sync_adapter(&adapter);
        gate.set_close_policy(DockViewportClosePolicy::Prevent);
        assert!(gate.should_allow_close(window.window_id()));
    }
}
