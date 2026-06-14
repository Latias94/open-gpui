use crate::{
    DockActionOutcome, DockController, DockSpaceId, DockViewportClosePolicy,
    DockViewportCloseStatus, DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus,
};
use open_gpui::{App, Entity, WindowId};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(crate) struct DockViewportCloseCoordinator {
    pre_closed_merge_back_windows: HashSet<WindowId>,
}

impl DockViewportCloseCoordinator {
    pub(crate) fn was_merge_back_precommitted(&mut self, window_id: WindowId) -> bool {
        self.pre_closed_merge_back_windows.remove(&window_id)
    }

    pub(crate) fn apply_should_close_plan(
        &mut self,
        mut outcome: DockViewportShouldCloseOutcome,
        close_policy: DockViewportClosePolicy,
        controller: &Entity<DockController>,
        cx: &mut App,
    ) -> DockViewportShouldCloseOutcome {
        if !matches!(outcome.status, DockViewportShouldCloseStatus::Allowed) {
            return outcome;
        }

        let Some(space) = outcome.space.as_ref() else {
            return outcome;
        };

        let mut allowed = self.validate_should_close(space, &close_policy, controller, cx);
        if allowed && let DockViewportClosePolicy::MergeBack { target_space } = &close_policy {
            let status = merge_space_back(controller, space, target_space, cx);
            match status {
                DockViewportCloseStatus::MergedBack => {
                    self.pre_closed_merge_back_windows.insert(outcome.window_id);
                }
                DockViewportCloseStatus::Closed => {}
                DockViewportCloseStatus::MergeBackFailed
                | DockViewportCloseStatus::UnknownWindow => {
                    allowed = false;
                }
            }
        }

        if !allowed {
            outcome.status = DockViewportShouldCloseStatus::Vetoed;
        }
        outcome
    }

    fn validate_should_close(
        &self,
        space: &DockSpaceId,
        close_policy: &DockViewportClosePolicy,
        controller: &Entity<DockController>,
        cx: &App,
    ) -> bool {
        let controller = controller.read(cx);
        let workspace = controller.workspace();
        match close_policy {
            DockViewportClosePolicy::RetainLayout => workspace.validate_close_space(space).is_ok(),
            DockViewportClosePolicy::MergeBack { target_space } => workspace
                .validate_merge_space_into(space, target_space)
                .is_ok(),
            DockViewportClosePolicy::Prevent => false,
        }
    }
}

pub(crate) fn merge_space_back(
    controller: &Entity<DockController>,
    source_space: &DockSpaceId,
    target_space: &DockSpaceId,
    cx: &mut App,
) -> DockViewportCloseStatus {
    controller
        .update(cx, |controller, cx| {
            let outcome = controller
                .workspace_mut()
                .commit_merge_space_into(source_space, target_space);
            if outcome
                .as_ref()
                .map(|outcome| outcome.changed())
                .unwrap_or(false)
            {
                cx.notify();
            }
            outcome
        })
        .map(close_status_from_action)
        .unwrap_or(DockViewportCloseStatus::MergeBackFailed)
}

fn close_status_from_action(outcome: DockActionOutcome) -> DockViewportCloseStatus {
    if outcome.changed() {
        DockViewportCloseStatus::MergedBack
    } else {
        DockViewportCloseStatus::Closed
    }
}
