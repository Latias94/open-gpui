use crate::{
    DockSpaceId, DockViewportActivationTransaction, DockViewportFocusCommand,
    DockViewportFocusCoordinator, viewport_registry::DockViewportRegistrationKey,
};
use open_gpui::{App, MouseButton, WindowId};
use std::collections::HashMap;

/// Backend focus state used to mirror ImGui's platform viewport focus semantics.
#[derive(Debug, Default)]
pub(crate) struct DockViewportBackendFocusState {
    pending_activation: Option<DockViewportActivationTransaction>,
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

/// Result of replacing the one backend-focus activation slot.
///
/// The displaced transaction is returned to the runtime owner instead of being silently dropped.
/// A surface activation carried by that transaction must be settled before the replacement can
/// dispatch a focus command.
#[derive(Debug, Default)]
pub(crate) struct DockViewportPendingActivationUpdate {
    changed: bool,
    displaced: Option<DockViewportActivationTransaction>,
}

impl DockViewportPendingActivationUpdate {
    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn displaced(self) -> Option<DockViewportActivationTransaction> {
        self.displaced
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportBackendFocusRecord {
    focused_changed: bool,
    cleared_pending_activation: Option<DockViewportActivationTransaction>,
    z_order_stamp_changed: bool,
}

impl DockViewportBackendFocusRecord {
    pub(crate) fn changed(self) -> bool {
        self.focused_changed
            || self.cleared_pending_activation.is_some()
            || self.z_order_stamp_changed
    }

    pub(crate) fn cleared_pending_activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.cleared_pending_activation.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DockViewportConfirmedBackendFocusEffect {
    #[default]
    None,
    PendingActivation,
    PlatformRestore,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DockViewportConfirmedBackendFocusOutcome {
    changed: bool,
    effect: DockViewportConfirmedBackendFocusEffect,
    focus_command: Option<DockViewportFocusCommand>,
}

impl DockViewportConfirmedBackendFocusOutcome {
    fn no_effect(changed: bool) -> Self {
        Self {
            changed,
            effect: DockViewportConfirmedBackendFocusEffect::None,
            focus_command: None,
        }
    }

    fn pending_activation(changed: bool, focus_command: DockViewportFocusCommand) -> Self {
        Self {
            changed,
            effect: DockViewportConfirmedBackendFocusEffect::PendingActivation,
            focus_command: Some(focus_command),
        }
    }

    fn platform_restore(changed: bool, focus_command: Option<DockViewportFocusCommand>) -> Self {
        Self {
            changed,
            effect: DockViewportConfirmedBackendFocusEffect::PlatformRestore,
            focus_command,
        }
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    #[cfg(test)]
    pub(crate) fn effect(&self) -> DockViewportConfirmedBackendFocusEffect {
        self.effect
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
/// `MouseDown` mirrors `!IsAnyMouseDown()` in the reference implementation: explicit pending
/// viewport activations still win, but ordinary focus restoration is suppressed while pointer
/// input is active.
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
    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<&DockViewportActivationTransaction> {
        self.pending_activation.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn record_pending_activation(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> bool {
        self.record_pending_activation_with_displaced(activation)
            .changed()
    }

    pub(crate) fn record_pending_activation_with_displaced(
        &mut self,
        activation: DockViewportActivationTransaction,
    ) -> DockViewportPendingActivationUpdate {
        if self.pending_activation.as_ref() == Some(&activation) {
            return DockViewportPendingActivationUpdate::default();
        }
        DockViewportPendingActivationUpdate {
            changed: true,
            displaced: self.pending_activation.replace(activation),
        }
    }

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
        let cleared_pending_activation = if focused_changed {
            self.clear_pending_activation_except_window(window_id)
        } else {
            None
        };
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
            cleared_pending_activation,
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
        )
        .into_focus_command()
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &mut self,
        focus: &DockViewportFocusCoordinator,
        registration: &DockViewportRegistrationKey,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        platform_focus_restore_policy: DockViewportPlatformFocusRestorePolicy,
    ) -> DockViewportConfirmedBackendFocusOutcome {
        let space = registration.space();
        let window_id = registration.window_id();
        let suppress_destroyed_previous_focus_restore =
            self.take_destroyed_previous_focus_suppression(window_id);
        let mut changed = suppress_destroyed_previous_focus_restore;
        // Mouse-down mirrors ImGui's platform-focus restore gate, but explicit viewport
        // activations from drop, tear-off, or close recovery already carry their target focus.
        if let Some(activation) = self.take_pending_activation_for_registration(registration) {
            changed = true;
            let focus_command = activation
                .surface_activation_binding()
                .cloned()
                .map(|binding| {
                    DockViewportFocusCommand::surface_activation(
                        activation.focus_request().clone(),
                        binding,
                    )
                })
                .unwrap_or_else(|| {
                    DockViewportFocusCommand::new(
                        activation.focus_source(),
                        activation.focus_request().clone(),
                    )
                });
            return DockViewportConfirmedBackendFocusOutcome::pending_activation(
                changed,
                focus_command,
            );
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

    fn clear_pending_activation_except_window(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockViewportActivationTransaction> {
        if !self
            .pending_activation
            .as_ref()
            .is_some_and(|activation| activation.window_id() != window_id)
        {
            return None;
        }
        self.pending_activation.take()
    }

    pub(crate) fn take_pending_activation_for(
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

    pub(crate) fn take_pending_activation_for_registration(
        &mut self,
        registration: &DockViewportRegistrationKey,
    ) -> Option<DockViewportActivationTransaction> {
        if self
            .pending_activation
            .as_ref()
            .is_some_and(|activation| activation.registration_key() == registration)
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
    fn replacing_pending_activation_returns_displaced_transaction_once() {
        let mut state = DockViewportBackendFocusState::default();
        let first_window = handle(1);
        let second_window = handle(2);
        let first = DockViewportActivationTransaction::new(
            space("first"),
            first_window,
            DockViewportFocusRequest::panel("first"),
        );
        let second = DockViewportActivationTransaction::new(
            space("second"),
            second_window,
            DockViewportFocusRequest::panel("second"),
        );

        let first_update = state.record_pending_activation_with_displaced(first.clone());
        assert!(first_update.changed());
        assert_eq!(first_update.displaced(), None);

        let replacement = state.record_pending_activation_with_displaced(second.clone());
        assert!(replacement.changed());
        assert_eq!(replacement.displaced(), Some(first));
        assert_eq!(state.pending_activation(), Some(&second));

        let duplicate = state.record_pending_activation_with_displaced(second);
        assert!(!duplicate.changed());
        assert_eq!(duplicate.displaced(), None);
    }

    #[test]
    fn replacement_registration_does_not_consume_stale_pending_activation() {
        let mut state = DockViewportBackendFocusState::default();
        let window = handle(1);
        let dock_space = space("main");
        let first_registration = DockViewportRegistrationKey::for_test_generation(
            dock_space.clone(),
            window.window_id(),
            1,
        );
        let second_registration =
            DockViewportRegistrationKey::for_test_generation(dock_space, window.window_id(), 2);
        let pending = DockViewportActivationTransaction::registered(
            first_registration,
            window,
            DockViewportFocusRequest::panel("a"),
        );
        assert!(state.record_pending_activation(pending.clone()));
        let focus = DockViewportFocusCoordinator::default();

        let outcome = state.confirmed_backend_window_focus_outcome(
            &focus,
            &second_registration,
            DockViewportPlatformFocusRestoreGate::MouseDown,
            DockViewportPlatformFocusRestorePolicy::PreserveDockFocus,
        );

        assert_eq!(
            outcome.effect(),
            DockViewportConfirmedBackendFocusEffect::None
        );
        assert_eq!(
            state.pending_activation(),
            Some(&pending),
            "a backend-focus signal for G2 must not consume G1 activation intent"
        );
    }

    #[test]
    fn backend_focus_clear_is_reported_only_on_focus_transition() {
        let mut state = DockViewportBackendFocusState::default();
        let pending_window = handle(1);
        let confirmed_window = handle(2);
        let pending = DockViewportActivationTransaction::new(
            space("pending"),
            pending_window,
            DockViewportFocusRequest::panel("pending"),
        );
        state.record_pending_activation(pending.clone());

        let first_record = state
            .record_confirmed_backend_focused_window(confirmed_window.window_id(), |window| {
                window == pending_window.window_id() || window == confirmed_window.window_id()
            })
            .expect("confirmed window should be live");
        assert_eq!(
            first_record.cleared_pending_activation(),
            Some(&pending),
            "a focus transition should return the displaced pending activation"
        );

        let second_record = state
            .record_confirmed_backend_focused_window(confirmed_window.window_id(), |_| true)
            .expect("repeated confirmed window should remain live");
        assert_eq!(
            second_record.cleared_pending_activation(),
            None,
            "a no-op backend sample must not manufacture a second cancellation"
        );
    }

    #[test]
    fn backend_focus_on_another_live_window_clears_pending_activation() {
        let mut state = DockViewportBackendFocusState::default();
        let alpha_window = handle(1);
        let beta_window = handle(2);
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
    }

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
        assert_eq!(
            state.pending_activation(),
            None,
            "creation z-order is not backend focus confirmation"
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
        );

        assert_eq!(
            platform_restore.effect(),
            DockViewportConfirmedBackendFocusEffect::PlatformRestore
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

        assert!(
            state.record_pending_activation(DockViewportActivationTransaction::new(
                dock_space.clone(),
                window,
                DockViewportFocusRequest::panel("a"),
            ))
        );
        let pending_activation = state.confirmed_backend_window_focus_outcome(
            &focus,
            &registration,
            DockViewportPlatformFocusRestoreGate::MouseDown,
            DockViewportPlatformFocusRestorePolicy::PreserveDockFocus,
        );

        assert_eq!(
            pending_activation.effect(),
            DockViewportConfirmedBackendFocusEffect::PendingActivation
        );
        assert!(
            pending_activation.changed(),
            "consuming a pending activation mutates backend focus state"
        );
        let command = pending_activation
            .into_focus_command()
            .expect("pending activation should carry an explicit focus command");
        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::ViewportActivation
        );
        assert_eq!(state.pending_activation(), None);
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
                DockViewportPlatformFocusRestoreGate::MouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
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
                DockViewportPlatformFocusRestoreGate::MouseDown,
                DockViewportPlatformFocusRestorePolicy::RestoreDockFocus,
            )
            .expect("explicit viewport activation wins over the mouse-down gate");

        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::ViewportActivation
        );
        assert_eq!(state.pending_activation(), None);
    }

    #[test]
    fn policy_disabled_platform_focus_restore_preserves_explicit_pending_activation() {
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
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::PreserveDockFocus,
            ),
            None,
            "policy opt-out suppresses ordinary platform focus restoration"
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
                DockViewportPlatformFocusRestoreGate::NoMouseDown,
                DockViewportPlatformFocusRestorePolicy::PreserveDockFocus,
            )
            .expect("explicit viewport activation is not gated by platform restore policy");

        assert_eq!(command.request(), &DockViewportFocusRequest::panel("a"));
        assert_eq!(
            command.source(),
            DockViewportFocusCommandSource::ViewportActivation
        );
        assert_eq!(state.pending_activation(), None);
    }
}
