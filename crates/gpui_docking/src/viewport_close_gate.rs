use crate::{DockViewportAdapter, DockViewportClosePolicy, DockWorkspace};
use open_gpui::WindowId;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Clone, Debug)]
pub(crate) struct DockViewportCloseGate {
    close_policy: Rc<RefCell<DockViewportClosePolicy>>,
    window_spaces: Rc<RefCell<HashMap<WindowId, crate::DockSpaceId>>>,
}

impl DockViewportCloseGate {
    pub(crate) fn new(close_policy: DockViewportClosePolicy) -> Self {
        Self {
            close_policy: Rc::new(RefCell::new(close_policy)),
            window_spaces: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub(crate) fn close_policy(&self) -> DockViewportClosePolicy {
        self.close_policy.borrow().clone()
    }

    pub(crate) fn set_close_policy(&self, close_policy: DockViewportClosePolicy) {
        *self.close_policy.borrow_mut() = close_policy;
    }

    pub(crate) fn sync_adapter(&self, adapter: &DockViewportAdapter) {
        *self.window_spaces.borrow_mut() = adapter
            .spaces()
            .into_iter()
            .filter_map(|space| {
                adapter
                    .window_for_space(&space)
                    .map(|window| (window.window_id(), space))
            })
            .collect();
    }

    pub(crate) fn should_allow_close(&self, window_id: WindowId) -> bool {
        if !self.window_spaces.borrow().contains_key(&window_id) {
            return true;
        }

        !matches!(self.close_policy(), DockViewportClosePolicy::Prevent)
    }

    pub(crate) fn should_allow_close_with_workspace(
        &self,
        window_id: WindowId,
        workspace: &DockWorkspace,
    ) -> bool {
        let Some(space) = self.window_spaces.borrow().get(&window_id).cloned() else {
            return true;
        };

        match self.close_policy() {
            DockViewportClosePolicy::Prevent => false,
            DockViewportClosePolicy::RetainLayout => workspace.validate_close_space(&space).is_ok(),
            DockViewportClosePolicy::MergeBack { .. } => true,
        }
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
