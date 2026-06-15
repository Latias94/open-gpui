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
        let platform_viewports_allowed = self.with_workspace(cx, |workspace| {
            workspace.policy().allows_platform_viewports()
        });
        if !platform_viewports_allowed || cx.mouse_button_is_pressed(MouseButton::Left).is_none() {
            return false;
        }
        let drag_session = self.active_payload_drag_session(payload);
        let Some(session) = self
            .interaction_mut()
            .begin_outside_release_poll_with_session(payload, drag_session)
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
                let changed = self.commit_payload_drop_release(release, window, cx);
                cx.stop_active_drag(window);
                changed
            }
            DockOutsideReleasePollDecision::Stop(drag_session) => {
                self.finish_payload_drag_session(&drag_session, cx);
                self.clear_drop_preview_interaction();
                self.viewport_runtime().clear_routed_drop_preview(cx);
                window.refresh();
                false
            }
            DockOutsideReleasePollDecision::Inactive => false,
        }
    }
}
