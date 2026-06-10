use crate::{DockHost, drag::DockDragPayload, interaction::DockOutsideReleasePollSession};
use open_gpui::{Context, MouseButton, Window};
use std::time::Duration;

impl DockHost {
    pub(crate) fn schedule_outside_release_poll_from_host(
        &mut self,
        payload: &DockDragPayload,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.viewport_runtime().is_none()
            || cx.mouse_button_is_pressed(MouseButton::Left).is_none()
        {
            return false;
        }
        let Some(session) = self
            .interaction_mut()
            .begin_outside_release_poll(payload.identity())
        else {
            return false;
        };

        cx.spawn_in(window, async move |host, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let should_continue = host
                    .update_in(cx, |host, window, cx| {
                        host.poll_outside_release_from_host(&session, window, cx)
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
        true
    }

    fn poll_outside_release_from_host(
        &mut self,
        session: &DockOutsideReleasePollSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .interaction()
            .outside_release_poll_session_active(session)
        {
            return false;
        }

        let Some(payload) = cx.active_drag_value::<DockDragPayload>().cloned() else {
            self.interaction_mut().finish_outside_release_poll(session);
            return false;
        };
        if !self
            .interaction()
            .outside_release_poll_session_accepts_payload(session, &payload.identity())
        {
            self.interaction_mut().finish_outside_release_poll(session);
            return false;
        }

        match cx.mouse_button_is_pressed(MouseButton::Left) {
            Some(true) => true,
            Some(false) => {
                self.interaction_mut().finish_outside_release_poll(session);
                let target_space = self.space().clone();
                let release_position = window.mouse_position();
                let changed = self.drop_payload_from_render(
                    &payload,
                    target_space,
                    release_position,
                    window,
                    cx,
                );
                cx.stop_active_drag(window);
                changed
            }
            None => {
                self.interaction_mut().finish_outside_release_poll(session);
                false
            }
        }
    }
}
