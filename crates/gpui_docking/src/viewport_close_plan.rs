use crate::{
    DockActionOutcome, DockController, DockItemId, DockMergeBackTarget, DockSpaceId,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportMergeBackClosePlan,
    DockViewportShouldCloseOutcome, DockViewportShouldCloseStatus,
};
use open_gpui::{App, Entity, WindowId};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct DockViewportCloseCoordinator {
    window_close_plans: HashMap<WindowId, DockViewportClosePlanState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockViewportClosePlanState {
    Pending(DockViewportMergeBackClosePlan),
    Discarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportClosePlanEffect {
    Unchanged,
    DiscardedPending,
    Cleared,
}

impl DockViewportClosePlanEffect {
    pub(crate) fn changed(self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}

impl DockViewportCloseCoordinator {
    pub(crate) fn discard_window(&mut self, window_id: WindowId) -> DockViewportClosePlanEffect {
        match self.window_close_plans.get_mut(&window_id) {
            Some(state @ DockViewportClosePlanState::Pending(_)) => {
                *state = DockViewportClosePlanState::Discarded;
                DockViewportClosePlanEffect::DiscardedPending
            }
            _ => DockViewportClosePlanEffect::Unchanged,
        }
    }

    pub(crate) fn cancel_window(&mut self, window_id: WindowId) -> DockViewportClosePlanEffect {
        if self.window_close_plans.remove(&window_id).is_some() {
            DockViewportClosePlanEffect::Cleared
        } else {
            DockViewportClosePlanEffect::Unchanged
        }
    }

    pub(crate) fn take_window_close_state(
        &mut self,
        window_id: WindowId,
    ) -> Option<DockViewportClosePlanState> {
        self.window_close_plans.remove(&window_id)
    }

    pub(crate) fn apply_should_close_plan(
        &mut self,
        mut outcome: DockViewportShouldCloseOutcome,
        close_policy: DockViewportClosePolicy,
        focus_item: Option<DockItemId>,
        controller: &Entity<DockController>,
        cx: &App,
    ) -> DockViewportShouldCloseOutcome {
        if !matches!(outcome.status, DockViewportShouldCloseStatus::Allowed) {
            return outcome;
        }

        let Some(space) = outcome.space.as_ref() else {
            return outcome;
        };

        let merge_target = match Self::validated_merge_target(space, &close_policy, controller, cx)
        {
            Some(target) => target,
            None => {
                outcome.status = DockViewportShouldCloseStatus::Vetoed;
                self.window_close_plans.remove(&outcome.window_id);
                return outcome;
            }
        };

        if let DockViewportClosePolicy::MergeBack { target_space } = close_policy {
            self.window_close_plans.insert(
                outcome.window_id,
                DockViewportClosePlanState::Pending(
                    DockViewportMergeBackClosePlan::new(space.clone(), target_space, focus_item)
                        .with_target(merge_target),
                ),
            );
        } else {
            self.window_close_plans.remove(&outcome.window_id);
        }
        outcome
    }

    fn validated_merge_target(
        space: &DockSpaceId,
        close_policy: &DockViewportClosePolicy,
        controller: &Entity<DockController>,
        cx: &App,
    ) -> Option<DockMergeBackTarget> {
        let controller = controller.read(cx);
        let workspace = controller.workspace();
        match close_policy {
            DockViewportClosePolicy::RetainLayout => workspace
                .validate_close_space(space)
                .is_ok()
                .then_some(DockMergeBackTarget::SpaceOnly),
            DockViewportClosePolicy::MergeBack { target_space } => {
                workspace.resolve_merge_target(space, target_space).ok()
            }
            DockViewportClosePolicy::Prevent => None,
        }
    }
}

pub(crate) fn commit_prevalidated_merge_back_plan(
    controller: &Entity<DockController>,
    plan: &DockViewportMergeBackClosePlan,
    cx: &mut App,
) -> DockViewportCloseStatus {
    controller
        .update(cx, |controller, cx| {
            let outcome = controller
                .workspace_mut()
                .commit_prevalidated_merge_space_into_target(
                    plan.source_space(),
                    plan.target_space(),
                    plan.target(),
                );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockController, DockGraph, DockNode, DockViewportShouldCloseStatus, DockWorkspace,
        viewport_test_support::{handle, space},
    };
    use open_gpui::{AppContext as _, TestAppContext};

    #[test]
    fn close_plan_state_table_distinguishes_pending_discarded_and_cleared() {
        let mut coordinator = DockViewportCloseCoordinator::default();
        let window = handle(1);
        let plan = DockViewportMergeBackClosePlan::new(space("source"), space("target"), None);

        coordinator.window_close_plans.insert(
            window.window_id(),
            DockViewportClosePlanState::Pending(plan.clone()),
        );
        assert_eq!(
            coordinator.discard_window(window.window_id()),
            DockViewportClosePlanEffect::DiscardedPending
        );
        assert_eq!(
            coordinator.take_window_close_state(window.window_id()),
            Some(DockViewportClosePlanState::Discarded)
        );
        assert_eq!(
            coordinator.take_window_close_state(window.window_id()),
            None
        );

        coordinator.window_close_plans.insert(
            window.window_id(),
            DockViewportClosePlanState::Pending(plan.clone()),
        );
        assert_eq!(
            coordinator.take_window_close_state(window.window_id()),
            Some(DockViewportClosePlanState::Pending(plan))
        );
        assert_eq!(
            coordinator.discard_window(window.window_id()),
            DockViewportClosePlanEffect::Unchanged
        );
        assert_eq!(
            coordinator.take_window_close_state(window.window_id()),
            None
        );

        let plan = DockViewportMergeBackClosePlan::new(space("source"), space("target"), None);
        coordinator.window_close_plans.insert(
            window.window_id(),
            DockViewportClosePlanState::Pending(plan),
        );
        assert_eq!(
            coordinator.cancel_window(window.window_id()),
            DockViewportClosePlanEffect::Cleared
        );
        assert_eq!(
            coordinator.cancel_window(window.window_id()),
            DockViewportClosePlanEffect::Unchanged
        );
    }

    #[open_gpui::test]
    fn merge_back_should_close_replaces_stale_plan_state(cx: &mut TestAppContext) {
        let mut coordinator = DockViewportCloseCoordinator::default();
        let mut graph = DockGraph::new();
        let source = space("source");
        let target = space("target");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![crate::DockItemId::from("a")],
            selected: Some(crate::DockItemId::from("a")),
        });
        graph.set_root(source.clone(), tabs);
        let controller = cx.new(|_| {
            let mut workspace = DockWorkspace::new(source.clone(), graph);
            workspace.policy_mut().set_allow_platform_viewports(true);
            DockController::new(workspace)
        });

        coordinator
            .window_close_plans
            .insert(WindowId::from(1), DockViewportClosePlanState::Discarded);
        let outcome = cx.update(|app| {
            coordinator.apply_should_close_plan(
                DockViewportShouldCloseOutcome {
                    space: Some(source.clone()),
                    window_id: WindowId::from(1),
                    status: DockViewportShouldCloseStatus::Allowed,
                },
                DockViewportClosePolicy::MergeBack {
                    target_space: target,
                },
                None,
                &controller,
                app,
            )
        });

        assert_eq!(outcome.status, DockViewportShouldCloseStatus::Allowed);
        assert!(matches!(
            coordinator.window_close_plans.get(&WindowId::from(1)),
            Some(DockViewportClosePlanState::Pending(_))
        ));
    }
}
