use crate::{
    DockHost,
    drag::DockDragPayload,
    interaction::{
        DockOutsideReleasePollDecision, DockOutsideReleasePollRequest,
        DockOutsideReleasePollSession,
    },
};
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
        let payload = cx.active_drag_value::<DockDragPayload>().cloned();
        let request = DockOutsideReleasePollRequest::new(
            session.clone(),
            payload,
            cx.mouse_button_is_pressed(MouseButton::Left),
            self.space().clone(),
            window.mouse_position(),
        );
        let decision = self.interaction_mut().poll_outside_release(request);

        match decision {
            DockOutsideReleasePollDecision::Continue => true,
            DockOutsideReleasePollDecision::CommitRelease(release) => {
                let changed = self.drop_payload_release_from_render(release, window, cx);
                cx.stop_active_drag(window);
                changed
            }
            DockOutsideReleasePollDecision::Inactive | DockOutsideReleasePollDecision::Stop => {
                false
            }
        }
    }
}
