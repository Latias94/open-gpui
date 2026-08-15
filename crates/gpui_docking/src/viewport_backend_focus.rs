#[cfg(test)]
use crate::DockSpaceId;
use crate::{
    DockViewportFocusCommand, DockViewportFocusCoordinator,
    viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::{App, MouseButton, WindowId};
use std::collections::HashMap;

/// Backend focus state used to mirror ImGui's platform viewport focus semantics.
#[derive(Debug, Default)]
pub(crate) struct DockViewportBackendFocusState {
    /// Last live docking window observed as backend-focused. Mirrors ImGui's
    /// `PlatformLastFocusedViewportId` for activation and destroyed-focus suppression.
    last_confirmed_backend_focused_window: Option<WindowId>,
    viewport_z_order_stamp_count: u64,
    viewport_z_order_stamps: HashMap<WindowId, u64>,
    /// One-shot gate for ImGui's `prev_focused_has_been_destroyed` behavior: when backend focus
    /// moves to another viewport only because the previously focused viewport was destroyed, the
    /// next platform-focus restoration for the newly focused window must be skipped once.
    destroyed_previous_focus_suppression: Option<DockViewportDestroyedPreviousFocusSuppression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportBackendFocusRecord {
    focused_changed: bool,
    z_order_stamp_changed: bool,
}

impl DockViewportBackendFocusRecord {
    pub(crate) fn changed(self) -> bool {
        self.focused_changed || self.z_order_stamp_changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DockViewportConfirmedBackendFocusOutcome {
    changed: bool,
    focus_command: Option<DockViewportFocusCommand>,
}

impl DockViewportConfirmedBackendFocusOutcome {
    fn no_effect(changed: bool) -> Self {
        Self {
            changed,
            focus_command: None,
        }
    }

    fn platform_restore(changed: bool, focus_command: Option<DockViewportFocusCommand>) -> Self {
        Self {
            changed,
            focus_command,
        }
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn with_additional_changed(mut self, changed: bool) -> Self {
        self.changed |= changed;
        self
    }

    pub(crate) fn into_focus_command(self) -> Option<DockViewportFocusCommand> {
        self.focus_command
    }
}

/// Gate for applying ImGui-style platform focus restoration after backend focus changes.
///
/// `MouseDown` mirrors `!IsAnyMouseDown()` in the reference implementation. Explicit activation is
/// owned by the separate ticket executor; this gate controls only ordinary focus restoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPlatformFocusRestoreGate {
    NoMouseDown,
    MouseDown,
}

impl DockViewportPlatformFocusRestoreGate {
    pub(crate) fn from_app(cx: &App) -> Self {
        if MouseButton::all()
            .into_iter()
            .any(|button| cx.mouse_button_is_pressed(button) == Some(true))
        {
            Self::MouseDown
        } else {
            Self::NoMouseDown
        }
    }

    #[cfg(test)]
    pub(crate) fn from_mouse_down(mouse_down: bool) -> Self {
        if mouse_down {
            Self::MouseDown
        } else {
            Self::NoMouseDown
        }
    }

    fn allows_restore(self) -> bool {
        matches!(self, Self::NoMouseDown)
    }
}

/// Policy for whether backend platform focus is allowed to restore dock focus.
///
/// This mirrors ImGui's `ConfigViewportsPlatformFocusSetsImGuiFocus`: explicit viewport
/// activations still apply, while ordinary platform-focus restoration follows this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPlatformFocusRestorePolicy {
    RestoreDockFocus,
    PreserveDockFocus,
}

impl DockViewportPlatformFocusRestorePolicy {
    pub(crate) fn from_platform_focus_sets_dock_focus(enabled: bool) -> Self {
        if enabled {
            Self::RestoreDockFocus
        } else {
            Self::PreserveDockFocus
        }
    }

    fn allows_restore(self) -> bool {
        matches!(self, Self::RestoreDockFocus)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DockViewportDestroyedPreviousFocusSuppression {
    /// Backend-focused window that should consume the destroyed-previous-focus gate.
    focused_window: WindowId,
}

impl DockViewportBackendFocusState {
    pub(crate) fn record_viewport_created(&mut self, window_id: WindowId) {
        // Mirrors ImGui's AddUpdateViewport(): new platform viewports are assumed front-most for
        // fallback z-order even when they are opened without requesting platform focus.
        self.stamp_viewport_z_order(window_id);
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
        if focused_changed {
            self.destroyed_previous_focus_suppression = if previous_focused_window
                .is_none_or(|previous| !is_live_docking_window(previous))
            {
                Some(DockViewportDestroyedPreviousFocusSuppression {
                    focused_window: window_id,
                })
            } else {
                None
            };
        }
        self.last_confirmed_backend_focused_window = Some(window_id);
        let z_order_stamp_changed =
            focused_changed || !self.viewport_z_order_stamps.contains_key(&window_id);
        if z_order_stamp_changed {
            self.stamp_viewport_z_order(window_id);
        }
        Some(DockViewportBackendFocusRecord {
            focused_changed,
            z_order_stamp_changed,
        })
    }

    pub(crate) fn front_to_back_z_order_windows(
        &self,
        is_live_docking_window: impl Fn(WindowId) -> bool,
    ) -> Vec<WindowId> {
        let mut stamped_windows = self
            .viewport_z_order_stamps
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
        self.viewport_z_order_stamps.remove(&window_id);
        if self
            .destroyed_previous_focus_suppression
            .is_some_and(|suppression| suppression.focused_window == window_id)
        {
            self.destroyed_previous_focus_suppression = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &mut self,
        focus: &DockViewportFocusCoordinator,
        space: &DockSpaceId,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        platform_focus_restore_policy: DockViewportPlatformFocusRestorePolicy,
    ) -> Option<DockViewportFocusCommand> {
        let registration = DockViewportRegistrationKey::for_test(space.clone(), window_id);
        self.confirmed_backend_window_focus_outcome(
            focus,
            &registration,
            platform_focus_restore_gate,
            platform_focus_restore_policy,
            false,
        )
        .into_focus_command()
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &mut self,
        focus: &DockViewportFocusCoordinator,
        registration: &DockViewportRegistrationKey,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        platform_focus_restore_policy: DockViewportPlatformFocusRestorePolicy,
        suppress_platform_restore: bool,
    ) -> DockViewportConfirmedBackendFocusOutcome {
        let space = registration.space();
        let window_id = registration.window_id();
        let suppress_destroyed_previous_focus_restore =
            self.take_destroyed_previous_focus_suppression(window_id);
        let changed = suppress_destroyed_previous_focus_restore;
        if suppress_platform_restore {
            return DockViewportConfirmedBackendFocusOutcome::no_effect(changed);
        }
        if !platform_focus_restore_policy.allows_restore() {
            return DockViewportConfirmedBackendFocusOutcome::no_effect(changed);
        }
        if !platform_focus_restore_gate.allows_restore() {
            return DockViewportConfirmedBackendFocusOutcome::no_effect(changed);
        }
        if suppress_destroyed_previous_focus_restore {
            return DockViewportConfirmedBackendFocusOutcome::no_effect(changed);
        }
        let focus_command = focus
            .request_for_platform_activation(space)
            .map(DockViewportFocusCommand::platform_activation);
        DockViewportConfirmedBackendFocusOutcome::platform_restore(changed, focus_command)
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

    fn stamp_viewport_z_order(&mut self, window_id: WindowId) {
        self.viewport_z_order_stamp_count = self.viewport_z_order_stamp_count.wrapping_add(1);
        self.viewport_z_order_stamps
            .insert(window_id, self.viewport_z_order_stamp_count);
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
    fn viewport_z_order_stamps_expose_front_to_back_fallback_order() {
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
            state.front_to_back_z_order_windows(|_| true),
            vec![alpha_window.window_id(), beta_window.window_id()]
        );
        assert_eq!(
            state.front_to_back_z_order_windows(|window| window == beta_window.window_id()),
            vec![beta_window.window_id()],
            "stale focus stamps are filtered by current live docking-window ownership"
        );

        state.discard_window(alpha_window.window_id());
        assert_eq!(
            state.front_to_back_z_order_windows(|_| true),
            vec![beta_window.window_id()]
        );
    }

    #[test]
    fn restamping_same_confirmed_backend_focus_reports_changed() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);

        state
            .record_confirmed_backend_focused_window(window.window_id(), |_| true)
            .expect("window is live");
        state.discard_window(window.window_id());

        let record = state
            .record_confirmed_backend_focused_window(window.window_id(), |_| true)
            .expect("same backend-focused window is still live");

        assert!(
            record.changed(),
            "repairing a missing focus-stamp fallback for the confirmed backend focus is a runtime state change"
        );
        assert_eq!(
            state.front_to_back_z_order_windows(|_| true),
            vec![window.window_id()]
        );
    }

    #[test]
    fn viewport_creation_stamps_front_to_back_fallback_order_without_backend_focus() {
        let mut state = DockViewportBackendFocusState::default();
        let alpha_window = handle(1);
        let beta_window = handle(2);

        state.record_viewport_created(alpha_window.window_id());
        state.record_viewport_created(beta_window.window_id());

        assert_eq!(
            state.front_to_back_z_order_windows(|_| true),
            vec![beta_window.window_id(), alpha_window.window_id()],
            "new viewport creation should mirror ImGui AddUpdateViewport z-order stamping"
        );
    }

    #[test]
    fn initial_backend_focus_suppresses_platform_restore_once() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);
        let dock_space = space("main");
        let mut focus = DockViewportFocusCoordinator::default();
        focus.record_panel_focus(dock_space.clone(), item("a"));

        state
            .record_confirmed_backend_focused_window(window.window_id(), |_| true)
            .expect("initial viewport is live");

        assert_eq!(
            state.focus_command_for_confirmed_backend_window_focus(
                &focus,
                &dock_space,
                window.window_id(),
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            ),
            None,
            "initial backend focus mirrors ImGui PlatformLastFocusedViewportId=0 suppression"
        );
        let command = state
            .focus_command_for_confirmed_backend_window_focus(
                &focus,
                &dock_space,
                window.window_id(),
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            )
            .expect("initial suppression is consumed after one platform activation");
        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::PlatformActivation
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
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            ),
            None,
            "first activation after the previously focused viewport was destroyed mirrors ImGui's suppression gate"
        );
        let command = state
            .focus_command_for_confirmed_backend_window_focus(
                &focus,
                &beta_space,
                beta_window.window_id(),
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            )
            .expect("suppression is consumed after one activation");
        assert_eq!(command.request(), &DockViewportFocusRequest::panel("b"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::PlatformActivation
        );
    }

    #[test]
    fn confirmed_backend_focus_outcome_names_focus_effect_source() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);
        let dock_space = space("main");
        let registration =
            DockViewportRegistrationKey::for_test(dock_space.clone(), window.window_id());
        let mut focus = DockViewportFocusCoordinator::default();
        focus.record_panel_focus(dock_space.clone(), item("a"));

        let platform_restore = state.confirmed_backend_window_focus_outcome(
            &focus,
            &registration,
            DockViewportPlatformFocusRestoreGate::NoMouseDown,
            DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            false,
        );

        assert!(
            !platform_restore.changed(),
            "requesting a platform restore command does not mutate backend focus state by itself"
        );
        let command = platform_restore
            .into_focus_command()
            .expect("platform restore should replay recorded panel focus");
        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::PlatformActivation
        );
    }

    #[test]
    fn suppress_platform_restore_prevents_ordinary_restore_command() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);
        let dock_space = space("main");
        let registration =
            DockViewportRegistrationKey::for_test(dock_space.clone(), window.window_id());
        let mut focus = DockViewportFocusCoordinator::default();
        focus.record_panel_focus(dock_space, item("a"));

        let outcome = state.confirmed_backend_window_focus_outcome(
            &focus,
            &registration,
            DockViewportPlatformFocusRestoreGate::NoMouseDown,
            DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            true,
        );

        assert!(
            outcome.into_focus_command().is_none(),
            "ticket-backed activation authority suppresses the generic platform restore path"
        );
    }
}
