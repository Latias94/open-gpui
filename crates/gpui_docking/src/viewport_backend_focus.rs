use crate::{
    DockSpaceId, DockViewportActivationTransaction, DockViewportFocusCommand,
    DockViewportFocusCoordinator,
};
use open_gpui::WindowId;
use std::collections::HashMap;

/// Backend focus state used to mirror ImGui's platform viewport focus semantics.
#[derive(Debug, Default)]
pub(crate) struct DockViewportBackendFocusState {
    pending_activation: Option<DockViewportActivationTransaction>,
    /// Last live docking window observed as backend-focused. Mirrors ImGui's
    /// `PlatformLastFocusedViewportId` for activation and destroyed-focus suppression.
    last_confirmed_backend_focused_window: Option<WindowId>,
    focused_stamp_count: u64,
    focused_window_stamps: HashMap<WindowId, u64>,
    /// One-shot gate for ImGui's `prev_focused_has_been_destroyed` behavior: when backend focus
    /// moves to another viewport only because the previously focused viewport was destroyed, the
    /// next platform-focus restoration for the newly focused window must be skipped once.
    destroyed_previous_focus_suppression: Option<DockViewportDestroyedPreviousFocusSuppression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockViewportBackendFocusRecord {
    focused_changed: bool,
    pending_activation_changed: bool,
}

impl DockViewportBackendFocusRecord {
    pub(crate) fn changed(self) -> bool {
        self.focused_changed || self.pending_activation_changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DockViewportDestroyedPreviousFocusSuppression {
    /// Backend-focused window that should consume the destroyed-previous-focus gate.
    focused_window: WindowId,
}

impl DockViewportBackendFocusState {
    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.pending_activation.as_ref()
    }

    pub(crate) fn record_pending_activation(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> bool {
        if self.pending_activation.as_ref() == Some(&activation) {
            return false;
        }
        self.pending_activation = Some(activation);
        true
    }

    pub(crate) fn clear_pending_activation_for(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        if !self
            .pending_activation
            .as_ref()
            .is_some_and(|activation| activation.matches_window(space, window_id))
        {
            return false;
        }
        self.pending_activation = None;
        true
    }

    pub(crate) fn record_confirmed_backend_focused_window(
        &mut self,
        window_id: WindowId,
        is_live_docking_window: impl Fn(WindowId) -> bool,
    ) -> Option<DockViewportBackendFocusRecord> {
        if !is_live_docking_window(window_id) {
            return None;
        }

        let previous_focused_window = self.last_confirmed_backend_focused_window;
        let focused_changed = previous_focused_window != Some(window_id);
        let mut pending_activation_changed = false;
        if focused_changed {
            self.destroyed_previous_focus_suppression = if previous_focused_window
                .is_some_and(|previous| !is_live_docking_window(previous))
            {
                Some(DockViewportDestroyedPreviousFocusSuppression {
                    focused_window: window_id,
                })
            } else {
                None
            };
            pending_activation_changed = self.clear_pending_activation_except_window(window_id);
        }
        self.last_confirmed_backend_focused_window = Some(window_id);
        if focused_changed || !self.focused_window_stamps.contains_key(&window_id) {
            self.focused_stamp_count = self.focused_stamp_count.wrapping_add(1);
            self.focused_window_stamps
                .insert(window_id, self.focused_stamp_count);
        }
        Some(DockViewportBackendFocusRecord {
            focused_changed,
            pending_activation_changed,
        })
    }

    pub(crate) fn front_to_back_focused_windows(
        &self,
        is_live_docking_window: impl Fn(WindowId) -> bool,
    ) -> Vec<WindowId> {
        let mut stamped_windows = self
            .focused_window_stamps
            .iter()
            .filter_map(|(window_id, stamp)| {
                is_live_docking_window(*window_id).then_some((*window_id, *stamp))
            })
            .collect::<Vec<_>>();
        stamped_windows.sort_by(|(lhs_window, lhs_stamp), (rhs_window, rhs_stamp)| {
            rhs_stamp
                .cmp(lhs_stamp)
                .then_with(|| lhs_window.as_u64().cmp(&rhs_window.as_u64()))
        });
        stamped_windows
            .into_iter()
            .map(|(window_id, _)| window_id)
            .collect()
    }

    pub(crate) fn discard_window(&mut self, window_id: WindowId) {
        self.focused_window_stamps.remove(&window_id);
        if self
            .destroyed_previous_focus_suppression
            .is_some_and(|suppression| suppression.focused_window == window_id)
        {
            self.destroyed_previous_focus_suppression = None;
        }
    }

    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &mut self,
        focus: &DockViewportFocusCoordinator,
        space: &DockSpaceId,
        window_id: WindowId,
        mouse_down: bool,
    ) -> Option<DockViewportFocusCommand> {
        let suppress_destroyed_previous_focus_restore =
            self.take_destroyed_previous_focus_suppression(window_id);
        // Mouse-down mirrors ImGui's platform-focus restore gate, but explicit viewport
        // activations from drop, tear-off, or close recovery already carry their target focus.
        if let Some(activation) = self.take_pending_activation_for(space, window_id) {
            return Some(DockViewportFocusCommand::new(
                activation.focus_source(),
                activation.focus_request().clone(),
            ));
        }
        if mouse_down {
            return None;
        }
        if suppress_destroyed_previous_focus_restore {
            return None;
        }
        focus
            .request_for_platform_activation(space)
            .map(DockViewportFocusCommand::platform_activation)
    }

    fn clear_pending_activation_except_window(&mut self, window_id: WindowId) -> bool {
        if !self
            .pending_activation
            .as_ref()
            .is_some_and(|activation| activation.window_id() != window_id)
        {
            return false;
        }
        self.pending_activation = None;
        true
    }

    fn take_pending_activation_for(
        &mut self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportActivationTransaction> {
        if self
            .pending_activation
            .as_ref()
            .is_some_and(|activation| activation.matches_window(space, window_id))
        {
            self.pending_activation.take()
        } else {
            None
        }
    }

    fn take_destroyed_previous_focus_suppression(&mut self, window_id: WindowId) -> bool {
        if !self
            .destroyed_previous_focus_suppression
            .is_some_and(|suppression| suppression.focused_window == window_id)
        {
            return false;
        }
        self.destroyed_previous_focus_suppression = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockViewportFocusCommandSource, DockViewportFocusRequest,
        viewport_test_support::{handle, item, space},
    };

    #[test]
    fn backend_focus_on_another_live_window_clears_pending_activation() {
        let mut state = DockViewportBackendFocusState::default();
        let alpha_window = handle(1);
        let beta_window = handle(2);
        let alpha_space = space("alpha");
        let beta_space = space("beta");

        assert!(
            state.record_pending_activation(DockViewportActivationTransaction::new(
                beta_space.clone(),
                beta_window,
                DockViewportFocusRequest::panel("b"),
            ))
        );

        let record = state
            .record_confirmed_backend_focused_window(alpha_window.window_id(), |window| {
                window == alpha_window.window_id() || window == beta_window.window_id()
            })
            .expect("alpha is a live docking viewport");

        assert!(record.changed());
        assert_eq!(
            state.pending_activation(),
            None,
            "confirmed backend focus on a different docking viewport cancels stale activation intent"
        );
        assert!(!state.clear_pending_activation_for(&beta_space, beta_window.window_id()));
        assert!(!state.clear_pending_activation_for(&alpha_space, alpha_window.window_id()));
    }

    #[test]
    fn focused_window_stamps_expose_front_to_back_fallback_order() {
        let mut state = DockViewportBackendFocusState::default();
        let alpha_window = handle(1);
        let beta_window = handle(2);

        state
            .record_confirmed_backend_focused_window(alpha_window.window_id(), |_| true)
            .expect("alpha is live");
        state
            .record_confirmed_backend_focused_window(beta_window.window_id(), |_| true)
            .expect("beta is live");
        state
            .record_confirmed_backend_focused_window(alpha_window.window_id(), |_| true)
            .expect("alpha returns to front");

        assert_eq!(
            state.front_to_back_focused_windows(|_| true),
            vec![alpha_window.window_id(), beta_window.window_id()]
        );
        assert_eq!(
            state.front_to_back_focused_windows(|window| window == beta_window.window_id()),
            vec![beta_window.window_id()],
            "stale focus stamps are filtered by current live docking-window ownership"
        );

        state.discard_window(alpha_window.window_id());
        assert_eq!(
            state.front_to_back_focused_windows(|_| true),
            vec![beta_window.window_id()]
        );
    }

    #[test]
    fn destroyed_previous_focus_suppression_is_one_shot() {
        let mut state = DockViewportBackendFocusState::default();
        let alpha_window = handle(1);
        let beta_window = handle(2);
        let beta_space = space("beta");
        let mut focus = DockViewportFocusCoordinator::default();
        focus.record_panel_focus(beta_space.clone(), item("b"));

        state
            .record_confirmed_backend_focused_window(alpha_window.window_id(), |window| {
                window == alpha_window.window_id() || window == beta_window.window_id()
            })
            .expect("alpha starts live");
        state
            .record_confirmed_backend_focused_window(beta_window.window_id(), |window| {
                window == beta_window.window_id()
            })
            .expect("beta is live after alpha was destroyed");

        assert_eq!(
            state.focus_command_for_confirmed_backend_window_focus(
                &focus,
                &beta_space,
                beta_window.window_id(),
                false,
            ),
            None,
            "first activation after the previously focused viewport was destroyed mirrors ImGui's suppression gate"
        );
        let command = state
            .focus_command_for_confirmed_backend_window_focus(
                &focus,
                &beta_space,
                beta_window.window_id(),
                false,
            )
            .expect("suppression is consumed after one activation");
        assert_eq!(command.request(), &DockViewportFocusRequest::panel("b"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::PlatformActivation
        );
    }

    #[test]
    fn mouse_down_suppresses_platform_restore_but_not_pending_activation() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);
        let dock_space = space("main");
        let mut focus = DockViewportFocusCoordinator::default();
        focus.record_panel_focus(dock_space.clone(), item("a"));

        assert_eq!(
            state.focus_command_for_confirmed_backend_window_focus(
                &focus,
                &dock_space,
                window.window_id(),
                true,
            ),
            None,
            "mouse-down suppresses ordinary platform focus restoration"
        );

        assert!(
            state.record_pending_activation(DockViewportActivationTransaction::new(
                dock_space.clone(),
                window,
                DockViewportFocusRequest::panel("a"),
            ))
        );
        let command = state
            .focus_command_for_confirmed_backend_window_focus(
                &focus,
                &dock_space,
                window.window_id(),
                true,
            )
            .expect("explicit viewport activation wins over the mouse-down gate");

        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::ViewportActivation
        );
        assert_eq!(state.pending_activation(), None);
    }
}
