//! DOM pointer-session state that remains host-testable without a browser runtime.

use open_gpui::{MouseButton, NavigationDirection, Pixels, Point, PointerCancelReason};

pub(crate) struct ClickState {
    last_button: Option<MouseButton>,
    last_position: Point<Pixels>,
    last_time: f64,
    current_count: usize,
}

impl Default for ClickState {
    fn default() -> Self {
        Self {
            last_button: None,
            last_position: Point::default(),
            last_time: 0.0,
            current_count: 0,
        }
    }
}

impl ClickState {
    pub(crate) fn register_click(
        &mut self,
        button: MouseButton,
        position: Point<Pixels>,
        time: f64,
    ) -> usize {
        let distance = ((f32::from(position.x) - f32::from(self.last_position.x)).powi(2)
            + (f32::from(position.y) - f32::from(self.last_position.y)).powi(2))
        .sqrt();

        if self.last_button == Some(button) && (time - self.last_time) < 400.0 && distance < 5.0 {
            self.current_count += 1;
        } else {
            self.current_count = 1;
        }

        self.last_button = Some(button);
        self.last_position = position;
        self.last_time = time;
        self.current_count
    }

    #[cfg(target_family = "wasm")]
    pub(crate) const fn current_count(&self) -> usize {
        self.current_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveWebPointer {
    pointer_id: i32,
    pressed_buttons: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WebPointerCaptureState {
    active: Option<ActiveWebPointer>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WebPointerCaptureCommand {
    #[default]
    None,
    Set(i32),
    Release(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebPointerButtonChange {
    Down(MouseButton),
    Up(MouseButton),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WebPointerTransition {
    pub(crate) capture_command: WebPointerCaptureCommand,
    pub(crate) button_change: Option<WebPointerButtonChange>,
    pub(crate) accept_event: bool,
    pub(crate) cancel_reason: Option<PointerCancelReason>,
}

impl WebPointerCaptureState {
    pub(crate) fn pointer_down(
        self,
        pointer_id: i32,
        button: i16,
        reported_buttons: u16,
    ) -> (Self, WebPointerTransition) {
        let button_mask = dom_mouse_button_mask(button);
        match self.active {
            None if button_mask != 0 && reported_buttons == button_mask => (
                Self {
                    active: Some(ActiveWebPointer {
                        pointer_id,
                        pressed_buttons: reported_buttons,
                    }),
                },
                WebPointerTransition {
                    capture_command: WebPointerCaptureCommand::Set(pointer_id),
                    accept_event: true,
                    ..WebPointerTransition::default()
                },
            ),
            None => (self, WebPointerTransition::default()),
            Some(active) if active.pointer_id == pointer_id => {
                let changed_buttons = active.pressed_buttons ^ reported_buttons;
                if button_mask == 0
                    || changed_buttons != button_mask
                    || active.pressed_buttons & button_mask != 0
                    || reported_buttons & button_mask == 0
                {
                    return self.unexpected_button_change(pointer_id);
                }
                (
                    Self {
                        active: Some(ActiveWebPointer {
                            pointer_id,
                            pressed_buttons: reported_buttons,
                        }),
                    },
                    WebPointerTransition {
                        accept_event: true,
                        ..WebPointerTransition::default()
                    },
                )
            }
            Some(_) => (self, WebPointerTransition::default()),
        }
    }

    pub(crate) fn pointer_up(
        self,
        pointer_id: i32,
        button: i16,
        reported_buttons: u16,
    ) -> (Self, WebPointerTransition) {
        let Some(active) = self.active.filter(|active| active.pointer_id == pointer_id) else {
            return (self, WebPointerTransition::default());
        };
        let button_mask = dom_mouse_button_mask(button);
        let changed_buttons = active.pressed_buttons ^ reported_buttons;
        if button_mask == 0
            || changed_buttons != button_mask
            || active.pressed_buttons & button_mask == 0
            || reported_buttons & button_mask != 0
        {
            return self.unexpected_button_change(pointer_id);
        }

        if reported_buttons == 0 {
            (
                Self::default(),
                WebPointerTransition {
                    capture_command: WebPointerCaptureCommand::Release(pointer_id),
                    accept_event: true,
                    ..WebPointerTransition::default()
                },
            )
        } else {
            (
                Self {
                    active: Some(ActiveWebPointer {
                        pointer_id,
                        pressed_buttons: reported_buttons,
                    }),
                },
                WebPointerTransition {
                    accept_event: true,
                    ..WebPointerTransition::default()
                },
            )
        }
    }

    pub(crate) fn pointer_motion(
        self,
        pointer_id: i32,
        button: i16,
        reported_buttons: u16,
    ) -> (Self, WebPointerTransition) {
        let Some(active) = self.active else {
            return (
                self,
                WebPointerTransition {
                    accept_event: true,
                    ..WebPointerTransition::default()
                },
            );
        };
        if active.pointer_id != pointer_id {
            return (self, WebPointerTransition::default());
        }
        let changed_buttons = active.pressed_buttons ^ reported_buttons;
        if changed_buttons == 0 {
            return (
                self,
                WebPointerTransition {
                    accept_event: true,
                    ..WebPointerTransition::default()
                },
            );
        }

        let button_mask = dom_mouse_button_mask(button);
        if button_mask == 0 || changed_buttons != button_mask {
            return self.unexpected_button_change(pointer_id);
        }
        let button = dom_mouse_button_to_gpui(button);
        let button_change = if reported_buttons & button_mask != 0 {
            WebPointerButtonChange::Down(button)
        } else {
            WebPointerButtonChange::Up(button)
        };
        let capture_command = if reported_buttons == 0 {
            WebPointerCaptureCommand::Release(pointer_id)
        } else {
            WebPointerCaptureCommand::None
        };
        let next_state = if reported_buttons == 0 {
            Self::default()
        } else {
            Self {
                active: Some(ActiveWebPointer {
                    pointer_id,
                    pressed_buttons: reported_buttons,
                }),
            }
        };

        (
            next_state,
            WebPointerTransition {
                capture_command,
                button_change: Some(button_change),
                accept_event: true,
                ..WebPointerTransition::default()
            },
        )
    }

    pub(crate) fn pointer_cancel(self, pointer_id: i32) -> (Self, WebPointerTransition) {
        let Some(active) = self.active.filter(|active| active.pointer_id == pointer_id) else {
            return (self, WebPointerTransition::default());
        };
        debug_assert_ne!(active.pressed_buttons, 0);
        (
            Self::default(),
            WebPointerTransition {
                capture_command: WebPointerCaptureCommand::Release(pointer_id),
                cancel_reason: Some(PointerCancelReason::PlatformCaptureLost),
                ..WebPointerTransition::default()
            },
        )
    }

    pub(crate) fn pointer_capture_lost(self, pointer_id: i32) -> (Self, WebPointerTransition) {
        let Some(active) = self.active.filter(|active| active.pointer_id == pointer_id) else {
            return (self, WebPointerTransition::default());
        };
        if active.pressed_buttons == 0 {
            return (Self::default(), WebPointerTransition::default());
        }
        (
            Self::default(),
            WebPointerTransition {
                cancel_reason: Some(PointerCancelReason::PlatformCaptureLost),
                ..WebPointerTransition::default()
            },
        )
    }

    pub(crate) fn cleanup(self, reason: PointerCancelReason) -> (Self, WebPointerTransition) {
        let Some(active) = self.active else {
            return (self, WebPointerTransition::default());
        };
        (
            Self::default(),
            WebPointerTransition {
                capture_command: WebPointerCaptureCommand::Release(active.pointer_id),
                cancel_reason: (active.pressed_buttons != 0).then_some(reason),
                ..WebPointerTransition::default()
            },
        )
    }

    fn unexpected_button_change(self, pointer_id: i32) -> (Self, WebPointerTransition) {
        (
            Self::default(),
            WebPointerTransition {
                capture_command: WebPointerCaptureCommand::Release(pointer_id),
                cancel_reason: Some(PointerCancelReason::PlatformCaptureLost),
                ..WebPointerTransition::default()
            },
        )
    }
}

pub(crate) fn dom_mouse_button_to_gpui(button: i16) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Navigate(NavigationDirection::Back),
        4 => MouseButton::Navigate(NavigationDirection::Forward),
        _ => MouseButton::Left,
    }
}

fn dom_mouse_button_mask(button: i16) -> u16 {
    match button {
        0 => 1,
        1 => 4,
        2 => 2,
        3 => 8,
        4 => 16,
        _ => 0,
    }
}

pub(crate) fn dom_buttons_to_pressed_button(buttons: u16) -> Option<MouseButton> {
    if buttons & 1 != 0 {
        Some(MouseButton::Left)
    } else if buttons & 2 != 0 {
        Some(MouseButton::Right)
    } else if buttons & 4 != 0 {
        Some(MouseButton::Middle)
    } else if buttons & 8 != 0 {
        Some(MouseButton::Navigate(NavigationDirection::Back))
    } else if buttons & 16 != 0 {
        Some(MouseButton::Navigate(NavigationDirection::Forward))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_count_is_scoped_to_the_changed_button() {
        let mut click_state = ClickState::default();
        let position = Point::default();

        assert_eq!(
            click_state.register_click(MouseButton::Left, position, 100.0),
            1
        );
        assert_eq!(
            click_state.register_click(MouseButton::Left, position, 200.0),
            2
        );
        assert_eq!(
            click_state.register_click(MouseButton::Right, position, 250.0),
            1
        );
    }

    #[test]
    fn pointer_capture_tracks_first_companion_and_final_buttons() {
        let (state, transition) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        assert_eq!(transition.capture_command, WebPointerCaptureCommand::Set(7));
        assert!(transition.accept_event);

        let (state, unchanged) = state.pointer_motion(7, -1, 1);
        assert!(unchanged.button_change.is_none());
        let (state, transition) = state.pointer_motion(7, 2, 3);
        assert_eq!(
            transition.button_change,
            Some(WebPointerButtonChange::Down(MouseButton::Right))
        );
        let (state, transition) = state.pointer_motion(7, 2, 1);
        assert_eq!(
            transition.button_change,
            Some(WebPointerButtonChange::Up(MouseButton::Right))
        );
        let (state, transition) = state.pointer_up(7, 0, 0);
        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert_eq!(transition.cancel_reason, None);
    }

    #[test]
    fn lost_capture_with_held_buttons_cancels_once() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_capture_lost(7);
        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );
        assert_eq!(state.pointer_capture_lost(7).1.cancel_reason, None);
    }

    #[test]
    fn pointer_cancel_then_lost_capture_does_not_duplicate_cancel() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_cancel(7);
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );
        assert_eq!(state.pointer_capture_lost(7).1.cancel_reason, None);
    }

    #[test]
    fn final_pointer_up_then_lost_capture_does_not_cancel() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_up(7, 0, 0);
        assert_eq!(transition.cancel_reason, None);
        assert_eq!(state.pointer_capture_lost(7).1.cancel_reason, None);
    }

    #[test]
    fn unexpected_held_button_loss_is_terminal() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, _) = state.pointer_motion(7, 2, 3);
        let (state, transition) = state.pointer_motion(7, -1, 1);

        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert!(!transition.accept_event);
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );
    }

    #[test]
    fn repeated_cleanup_cancels_only_the_active_session() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, cleanup) = state.cleanup(PointerCancelReason::PlatformCaptureLost);
        assert_eq!(
            cleanup.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert_eq!(
            cleanup.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );
        assert_eq!(
            state
                .cleanup(PointerCancelReason::PlatformCaptureLost)
                .1
                .cancel_reason,
            None
        );
    }

    #[test]
    fn window_deactivation_cleanup_preserves_cancel_reason() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, cleanup) = state.cleanup(PointerCancelReason::WindowDeactivated);

        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            cleanup.cancel_reason,
            Some(PointerCancelReason::WindowDeactivated)
        );
        assert_eq!(
            state
                .cleanup(PointerCancelReason::WindowDeactivated)
                .1
                .cancel_reason,
            None
        );
    }

    #[test]
    fn dom_button_masks_choose_the_primary_pressed_button() {
        assert_eq!(
            dom_buttons_to_pressed_button(1 | 2),
            Some(MouseButton::Left)
        );
        assert_eq!(dom_buttons_to_pressed_button(2), Some(MouseButton::Right));
        assert_eq!(dom_buttons_to_pressed_button(0), None);
    }
}
