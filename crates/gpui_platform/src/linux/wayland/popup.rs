pub use open_gpui::popup::*;

use wayland_protocols::xdg::shell::client::xdg_positioner;

pub(crate) fn wayland_anchor(anchor: PopupAnchor) -> xdg_positioner::Anchor {
    match anchor {
        PopupAnchor::Center => xdg_positioner::Anchor::None,
        PopupAnchor::Top => xdg_positioner::Anchor::Top,
        PopupAnchor::Bottom => xdg_positioner::Anchor::Bottom,
        PopupAnchor::Left => xdg_positioner::Anchor::Left,
        PopupAnchor::Right => xdg_positioner::Anchor::Right,
        PopupAnchor::TopLeft => xdg_positioner::Anchor::TopLeft,
        PopupAnchor::BottomLeft => xdg_positioner::Anchor::BottomLeft,
        PopupAnchor::TopRight => xdg_positioner::Anchor::TopRight,
        PopupAnchor::BottomRight => xdg_positioner::Anchor::BottomRight,
    }
}

pub(crate) fn wayland_gravity(gravity: PopupGravity) -> xdg_positioner::Gravity {
    match gravity {
        PopupGravity::Center => xdg_positioner::Gravity::None,
        PopupGravity::Top => xdg_positioner::Gravity::Top,
        PopupGravity::Bottom => xdg_positioner::Gravity::Bottom,
        PopupGravity::Left => xdg_positioner::Gravity::Left,
        PopupGravity::Right => xdg_positioner::Gravity::Right,
        PopupGravity::TopLeft => xdg_positioner::Gravity::TopLeft,
        PopupGravity::BottomLeft => xdg_positioner::Gravity::BottomLeft,
        PopupGravity::TopRight => xdg_positioner::Gravity::TopRight,
        PopupGravity::BottomRight => xdg_positioner::Gravity::BottomRight,
    }
}

pub(crate) fn wayland_constraint_adjustment(
    adjustment: PopupConstraintAdjustment,
) -> xdg_positioner::ConstraintAdjustment {
    // The flag values match the protocol bitfield, so the bits map across directly.
    xdg_positioner::ConstraintAdjustment::from_bits_truncate(adjustment.bits())
}

#[derive(Default)]
pub(super) struct PopupRepositionState {
    next_token: u32,
    generation: Option<u64>,
    pending: Option<(u32, u64)>,
    configured_token: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PopupConfigure {
    Initial,
    Compositor,
    Reposition { token: u32, generation: u64 },
    Expired,
}

impl PopupRepositionState {
    pub(super) fn prepare(&mut self, generation: u64) {
        self.invalidate();
        self.generation = Some(generation);
    }

    pub(super) fn invalidate(&mut self) {
        self.generation = None;
        self.pending = None;
    }

    pub(super) fn finish_unobserved(&mut self, generation: u64) {
        if self.generation == Some(generation) {
            self.invalidate();
        }
    }

    pub(super) fn request(&mut self, generation: u64) -> Option<u32> {
        if self.generation != Some(generation) {
            return None;
        }
        let token = self.next_token;
        self.next_token = token.wrapping_add(1);
        self.pending = Some((token, generation));
        Some(token)
    }

    pub(super) fn repositioned(&mut self, token: u32) {
        self.configured_token = Some(token);
    }

    pub(super) fn configure(&mut self, initial: bool) -> PopupConfigure {
        let Some(token) = self.configured_token.take() else {
            return if initial {
                PopupConfigure::Initial
            } else {
                PopupConfigure::Compositor
            };
        };
        match self.pending {
            Some((pending_token, generation))
                if pending_token == token && self.generation == Some(generation) =>
            {
                PopupConfigure::Reposition { token, generation }
            }
            _ => PopupConfigure::Expired,
        }
    }

    pub(super) fn apply_configure<T>(
        &self,
        configure: PopupConfigure,
        initial: bool,
        apply: impl FnOnce() -> T,
    ) -> Option<T> {
        let current = match configure {
            PopupConfigure::Initial => initial,
            PopupConfigure::Compositor => !initial,
            PopupConfigure::Reposition { token, generation } => {
                self.generation == Some(generation) && self.pending == Some((token, generation))
            }
            PopupConfigure::Expired => false,
        };
        current.then(apply)
    }

    pub(super) fn finish(&mut self, configure: PopupConfigure) -> Option<u64> {
        let PopupConfigure::Reposition { generation, .. } = configure else {
            return None;
        };
        self.apply_configure(configure, false, || ())?;
        self.invalidate();
        Some(generation)
    }
}

#[cfg(test)]
pub(super) fn test_options() -> PopupOptions {
    struct Parent;

    impl open_gpui::Render for Parent {
        fn render(
            &mut self,
            _: &mut open_gpui::Window,
            _: &mut open_gpui::Context<Self>,
        ) -> impl open_gpui::IntoElement {
            open_gpui::Empty
        }
    }

    PopupOptions {
        parent: open_gpui::WindowHandle::<Parent>::new(open_gpui::WindowId::from(0x1_0000_0001))
            .into(),
        anchor_rect: open_gpui::Bounds::default(),
        anchor: PopupAnchor::default(),
        gravity: PopupGravity::default(),
        constraint_adjustment: PopupConstraintAdjustment::default(),
        offset: open_gpui::Point::default(),
        grab: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{Pixels, Size, px, size};

    impl PopupRepositionState {
        fn apply_size(
            &self,
            configure: PopupConfigure,
            initial: bool,
            actual: &mut Size<Pixels>,
            configured: Size<Pixels>,
        ) -> bool {
            self.apply_configure(configure, initial, || {
                *actual = configured;
            })
            .is_some()
        }
    }

    #[test]
    fn superseded_reposition_cannot_settle_the_current_generation() {
        let mut state = PopupRepositionState::default();
        state.prepare(7);
        let first = state.request(7).unwrap();
        state.prepare(8);
        let second = state.request(8).unwrap();

        state.repositioned(first);
        assert_eq!(state.configure(false), PopupConfigure::Expired);
        assert_eq!(
            state.finish(PopupConfigure::Reposition {
                token: first,
                generation: 7,
            }),
            None
        );
        state.repositioned(second);
        let configure = state.configure(false);
        assert_eq!(
            configure,
            PopupConfigure::Reposition {
                token: second,
                generation: 8,
            }
        );
        assert_eq!(state.finish(configure), Some(8));
        assert_eq!(state.finish(configure), None);
    }

    #[test]
    fn invalidation_between_popup_and_surface_configure_refuses_geometry_and_settlement() {
        let mut state = PopupRepositionState::default();
        state.prepare(1);
        let token = state.request(1).unwrap();
        state.repositioned(token);
        let configure = state.configure(false);
        assert_eq!(
            configure,
            PopupConfigure::Reposition {
                token,
                generation: 1,
            }
        );
        state.invalidate();
        let mut actual = size(px(100.0), px(100.0));
        assert!(!state.apply_size(configure, false, &mut actual, size(px(200.0), px(200.0))));
        assert_eq!(actual, size(px(100.0), px(100.0)));
        assert_eq!(state.finish(configure), None);
        assert_eq!(state.request(1), None);
    }

    #[test]
    fn late_configure_after_unchanged_request_cannot_apply_old_size() {
        let mut state = PopupRepositionState::default();
        let mut actual = size(px(100.0), px(100.0));
        state.prepare(1);
        let old_token = state.request(1).unwrap();
        state.prepare(2);
        state.finish_unobserved(2);

        state.repositioned(old_token);
        let configure = state.configure(false);
        assert_eq!(configure, PopupConfigure::Expired);
        assert!(!state.apply_size(configure, false, &mut actual, size(px(200.0), px(200.0))));
        assert_eq!(actual, size(px(100.0), px(100.0)));
        assert_eq!(state.finish(configure), None);
    }

    #[test]
    fn invalidation_between_repositioned_and_popup_configure_preserves_expiration() {
        let mut state = PopupRepositionState::default();
        state.prepare(1);
        let token = state.request(1).unwrap();
        state.repositioned(token);
        state.invalidate();
        let configure = state.configure(true);
        assert_eq!(configure, PopupConfigure::Expired);
        assert!(
            state
                .apply_configure(configure, true, || panic!("expired configure applied"))
                .is_none()
        );
    }

    #[test]
    fn supersession_between_popup_and_surface_configure_refuses_geometry_and_ack() {
        let mut state = PopupRepositionState::default();
        let mut actual = size(px(100.0), px(100.0));
        state.prepare(1);
        let old_token = state.request(1).unwrap();
        state.repositioned(old_token);
        let old_configure = state.configure(false);
        state.prepare(2);
        let current_token = state.request(2).unwrap();

        let mut acknowledged = Vec::new();
        assert_eq!(
            state.apply_configure(old_configure, false, || {
                actual = size(px(200.0), px(200.0));
                acknowledged.push(10);
            }),
            None
        );
        assert_eq!(actual, size(px(100.0), px(100.0)));
        assert!(acknowledged.is_empty());

        state.repositioned(current_token);
        let current_configure = state.configure(false);
        assert_eq!(
            state.apply_configure(current_configure, false, || {
                actual = size(px(300.0), px(300.0));
                acknowledged.push(11);
            }),
            Some(())
        );
        assert_eq!(actual, size(px(300.0), px(300.0)));
        assert_eq!(acknowledged, vec![11]);
        assert_eq!(state.finish(current_configure), Some(2));
    }

    #[test]
    fn initial_configure_applies_once_and_callback_invalidation_revokes_settlement() {
        let mut state = PopupRepositionState::default();
        let mut actual = size(px(100.0), px(100.0));
        let initial = state.configure(true);
        assert_eq!(initial, PopupConfigure::Initial);
        assert!(state.apply_size(initial, true, &mut actual, size(px(120.0), px(120.0))));
        assert_eq!(actual, size(px(120.0), px(120.0)));
        assert!(!state.apply_size(initial, false, &mut actual, size(px(200.0), px(200.0))));

        state.prepare(1);
        let token = state.request(1).unwrap();
        state.repositioned(token);
        let configure = state.configure(false);
        assert!(state.apply_size(configure, false, &mut actual, size(px(200.0), px(200.0))));
        assert_eq!(actual, size(px(200.0), px(200.0)));
        state.prepare(2);
        let current = state.request(2).unwrap();
        let mut acknowledged = false;
        assert_eq!(
            state.apply_configure(configure, false, || acknowledged = true),
            None
        );
        assert!(!acknowledged);
        assert_eq!(state.finish(configure), None);
        state.repositioned(current);
        let configure = state.configure(false);
        assert_eq!(state.finish(configure), Some(2));
    }

    #[test]
    fn compositor_configure_applies_geometry_without_settling_a_pending_reposition() {
        let mut state = PopupRepositionState::default();
        let mut actual = size(px(100.0), px(100.0));
        state.prepare(1);
        let token = state.request(1).unwrap();
        let configure = state.configure(false);
        assert_eq!(configure, PopupConfigure::Compositor);
        assert!(state.apply_size(configure, false, &mut actual, size(px(150.0), px(150.0))));
        assert_eq!(actual, size(px(150.0), px(150.0)));
        assert_eq!(state.finish(configure), None);
        state.repositioned(token);
        let configure = state.configure(false);
        assert!(state.apply_size(configure, false, &mut actual, size(px(200.0), px(200.0))));
        assert_eq!(actual, size(px(200.0), px(200.0)));
        assert_eq!(state.finish(configure), Some(1));
    }

    #[test]
    fn reposition_tokens_wrap_without_reusing_the_generation() {
        let mut state = PopupRepositionState {
            next_token: u32::MAX,
            ..Default::default()
        };
        state.prepare(1);
        assert_eq!(state.request(1), Some(u32::MAX));
        state.prepare(2);
        assert_eq!(state.request(2), Some(0));
        state.repositioned(u32::MAX);
        assert_eq!(state.configure(false), PopupConfigure::Expired);
        state.repositioned(0);
        let configure = state.configure(false);
        assert_eq!(
            configure,
            PopupConfigure::Reposition {
                token: 0,
                generation: 2,
            }
        );
        assert_eq!(state.finish(configure), Some(2));
    }

    #[test]
    fn stale_unobserved_completion_preserves_the_new_request() {
        let mut state = PopupRepositionState::default();
        state.prepare(1);
        state.prepare(2);
        let token = state.request(2).unwrap();
        state.finish_unobserved(1);
        state.repositioned(token);
        let configure = state.configure(false);
        assert_eq!(
            configure,
            PopupConfigure::Reposition {
                token,
                generation: 2,
            }
        );
        assert_eq!(state.finish(configure), Some(2));
    }
}
