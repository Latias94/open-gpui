use crate::{
    DockActionOutcome, DockController, DockItemId, DockNodeId, DockSpaceId, DockViewportAdapter,
};
use open_gpui::{AnyWindowHandle, App, Entity, WindowId};
use std::collections::HashMap;

/// Default behavior for a platform viewport close request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DockViewportClosePolicy {
    /// Unregister the runtime window and keep the logical dock layout available for reopen.
    #[default]
    RetainLayout,
    /// Reject the close request and leave the runtime mapping intact.
    ///
    /// This policy prevents platform closes only when viewports are opened through
    /// [`crate::DockViewportRuntimeHandle`], which installs GPUI should-close hooks.
    /// Adapter-level cleanup methods run after the platform close decision has already happened,
    /// so vetoes are reported only by should-close outcomes.
    Prevent,
    /// Allow the platform close, then move the viewport's dock content into a fallback space.
    MergeBack {
        /// Logical dock space that should receive the closing viewport's tab stacks.
        target_space: DockSpaceId,
    },
}

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

/// Runtime result of closing a platform viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportCloseOutcome {
    /// Logical dock space that was associated with the closed window, when known.
    space: Option<DockSpaceId>,
    /// GPUI window id received from the close callback.
    window_id: WindowId,
    /// How the close request resolved.
    status: DockViewportCloseStatus,
    /// Pre-close merge-back plan that was committed before the platform close finished.
    merge_back: Option<DockViewportMergeBackClosePlan>,
}

impl DockViewportCloseOutcome {
    pub(crate) fn new(
        space: Option<DockSpaceId>,
        window_id: WindowId,
        status: DockViewportCloseStatus,
    ) -> Self {
        Self {
            space,
            window_id,
            status,
            merge_back: None,
        }
    }

    pub(crate) fn with_status(mut self, status: DockViewportCloseStatus) -> Self {
        self.status = status;
        if status != DockViewportCloseStatus::MergedBack {
            self.merge_back = None;
        }
        self
    }

    pub(crate) fn with_merge_back(mut self, plan: DockViewportMergeBackClosePlan) -> Self {
        self.status = DockViewportCloseStatus::MergedBack;
        self.merge_back = Some(plan);
        self
    }

    /// Logical dock space that was associated with the closed window, when known.
    pub fn space(&self) -> Option<&DockSpaceId> {
        self.space.as_ref()
    }

    /// GPUI window id received from the close callback.
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// How the close request resolved.
    pub fn status(&self) -> DockViewportCloseStatus {
        self.status
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.merge_back
            .as_ref()
            .and_then(DockViewportMergeBackClosePlan::focus_item)
    }

    pub(crate) fn merge_target_space(&self) -> Option<&DockSpaceId> {
        self.merge_back
            .as_ref()
            .map(DockViewportMergeBackClosePlan::target_space)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportMergeBackClosePlan {
    source_space: DockSpaceId,
    target_space: DockSpaceId,
    target: DockMergeBackTarget,
    focus_item: Option<DockItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockMergeBackTarget {
    /// Merge into the target space without binding to a specific target tabs node.
    SpaceOnly,
    /// Merge source root tabs into a prevalidated target tabs node.
    Tabs(DockNodeId),
}

impl DockViewportMergeBackClosePlan {
    pub(crate) fn new(
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        focus_item: Option<DockItemId>,
    ) -> Self {
        Self {
            source_space,
            target_space,
            target: DockMergeBackTarget::SpaceOnly,
            focus_item,
        }
    }

    pub(crate) fn with_target(mut self, target: DockMergeBackTarget) -> Self {
        self.target = target;
        self
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        &self.source_space
    }

    pub(crate) fn target(&self) -> DockMergeBackTarget {
        self.target
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

/// How a close request resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportCloseStatus {
    /// The window closed and its runtime mapping was removed.
    Closed,
    /// The window closed, its runtime mapping was removed, and content moved to fallback space.
    MergedBack,
    /// The window closed and its runtime mapping was removed, but merge-back could not commit.
    MergeBackFailed,
    /// The runtime did not know the closed window id.
    UnknownWindow,
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

/// Runtime result of a platform should-close query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportShouldCloseOutcome {
    /// Logical dock space associated with the queried window, when known.
    pub space: Option<DockSpaceId>,
    /// GPUI window id received from the should-close callback.
    pub window_id: WindowId,
    /// Whether the close should be allowed, vetoed, or ignored as unknown.
    pub status: DockViewportShouldCloseStatus,
}

impl DockViewportShouldCloseOutcome {
    /// Returns true when GPUI should continue closing the platform window.
    pub fn allows_close(&self) -> bool {
        !matches!(self.status, DockViewportShouldCloseStatus::Vetoed)
    }
}

/// How a should-close query resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportShouldCloseStatus {
    /// Runtime policy allows the platform close to continue.
    Allowed,
    /// Runtime policy rejects the platform close before the window closes.
    Vetoed,
    /// Runtime does not know this window id, so docking should not block GPUI.
    UnknownWindow,
}

/// Runtime result of unregistering a platform viewport mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportUnregisterOutcome {
    /// Logical dock space removed from the adapter mapping.
    pub space: DockSpaceId,
    /// GPUI window removed from the adapter mapping.
    pub window: AnyWindowHandle,
    /// Why the mapping was removed.
    pub reason: DockViewportUnregisterReason,
}

/// Reason a platform viewport mapping was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportUnregisterReason {
    /// The platform window closed.
    Closed,
    /// A new window replaced the previous mapping.
    Replaced,
    /// The application discarded runtime placement for the space.
    Discarded,
}

impl DockViewportAdapter {
    /// Applies viewport close policy before a GPUI platform window closes.
    ///
    /// Unknown windows are allowed to close because docking has no mapping to protect.
    pub(crate) fn should_close_viewport(
        &self,
        window_id: WindowId,
        policy: DockViewportClosePolicy,
    ) -> DockViewportShouldCloseOutcome {
        let Some(space) = self.space_for_window_id(window_id).cloned() else {
            return DockViewportShouldCloseOutcome {
                space: None,
                window_id,
                status: DockViewportShouldCloseStatus::UnknownWindow,
            };
        };

        let status = match policy {
            DockViewportClosePolicy::RetainLayout | DockViewportClosePolicy::MergeBack { .. } => {
                DockViewportShouldCloseStatus::Allowed
            }
            DockViewportClosePolicy::Prevent => DockViewportShouldCloseStatus::Vetoed,
        };
        DockViewportShouldCloseOutcome {
            space: Some(space),
            window_id,
            status,
        }
    }

    /// Removes a viewport by GPUI window id and returns a lifecycle outcome.
    ///
    /// This is the cleanup path for close callbacks that report only [`WindowId`].
    pub(crate) fn unregister_window_id(
        &mut self,
        window_id: WindowId,
        reason: DockViewportUnregisterReason,
    ) -> Option<DockViewportUnregisterOutcome> {
        let (space, snapshot) = self.unregister_window_id_snapshot(window_id)?;
        Some(DockViewportUnregisterOutcome {
            space,
            window: snapshot.window,
            reason,
        })
    }

    /// Handles an already-accepted GPUI window close by removing runtime mapping.
    pub(crate) fn handle_window_closed(&mut self, window_id: WindowId) -> DockViewportCloseOutcome {
        if let Some(outcome) =
            self.unregister_window_id(window_id, DockViewportUnregisterReason::Closed)
        {
            DockViewportCloseOutcome::new(
                Some(outcome.space),
                window_id,
                DockViewportCloseStatus::Closed,
            )
        } else {
            DockViewportCloseOutcome::new(None, window_id, DockViewportCloseStatus::UnknownWindow)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockItemId, DockNode, DockViewportAdapter, DockViewportOpenOutcome,
        DockViewportOpenStatus,
        viewport_test_support::{handle, register_viewport, space},
    };
    use open_gpui::{AppContext as _, TestAppContext};

    #[test]
    fn unregistering_by_window_id_clears_close_callback_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        register_viewport(&mut adapter, main.clone(), first);
        register_viewport(&mut adapter, secondary.clone(), second);

        let removed = adapter
            .unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed)
            .expect("window id should be registered");
        assert_eq!(removed.space, main);
        assert_eq!(removed.window, first);
        assert_eq!(removed.reason, DockViewportUnregisterReason::Closed);
        assert_eq!(adapter.space_for_window_id(first.window_id()), None);
        assert_eq!(adapter.window_for_space(&removed.space), None);
        assert_eq!(adapter.window_for_space(&secondary), Some(second));

        assert_eq!(
            adapter.unregister_window_id(first.window_id(), DockViewportUnregisterReason::Closed),
            None
        );
    }

    #[test]
    fn window_closed_cleanup_removes_only_runtime_mapping() {
        let mut graph = DockGraph::new();
        let main = space("main");
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![DockItemId::from("a")],
            selected: Some(DockItemId::from("a")),
        });
        graph.set_root(main.clone(), tabs);

        let mut adapter = DockViewportAdapter::new();
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);

        let outcome = adapter.handle_window_closed(window.window_id());
        assert_eq!(
            outcome,
            DockViewportCloseOutcome::new(
                Some(main.clone()),
                window.window_id(),
                DockViewportCloseStatus::Closed
            )
        );
        assert!(adapter.spaces().is_empty());
        assert!(
            graph.root(&main).is_some(),
            "runtime cleanup must not mutate the logical docking graph"
        );

        let reopened = handle(2);
        register_viewport(&mut adapter, main.clone(), reopened);
        assert_eq!(adapter.window_for_space(&main), Some(reopened));
        assert_eq!(
            adapter.space_for_window_id(reopened.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn window_closed_unknown_window_reports_unknown() {
        let mut adapter = DockViewportAdapter::new();
        let unknown = WindowId::from(99);

        assert_eq!(
            adapter.handle_window_closed(unknown),
            DockViewportCloseOutcome::new(None, unknown, DockViewportCloseStatus::UnknownWindow)
        );
    }

    #[test]
    fn window_closed_discards_stale_window_index() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window_id = WindowId::from(1);
        adapter.insert_stale_window_index_for_test(window_id, main);

        assert_eq!(
            adapter.handle_window_closed(window_id),
            DockViewportCloseOutcome::new(None, window_id, DockViewportCloseStatus::UnknownWindow)
        );
        assert_eq!(adapter.space_for_window_id(window_id), None);
        assert!(adapter.spaces().is_empty());
    }

    #[test]
    fn window_closed_stale_index_to_live_space_does_not_remove_current_viewport() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let stale_window_id = WindowId::from(1);
        let current_window = handle(2);
        register_viewport(&mut adapter, main.clone(), current_window);
        adapter.insert_stale_window_index_for_test(stale_window_id, main.clone());

        assert_eq!(
            adapter.handle_window_closed(stale_window_id),
            DockViewportCloseOutcome::new(
                None,
                stale_window_id,
                DockViewportCloseStatus::UnknownWindow
            )
        );
        assert_eq!(adapter.window_for_space(&main), Some(current_window));
        assert_eq!(
            adapter.space_for_window_id(current_window.window_id()),
            Some(&main)
        );
    }

    #[test]
    fn viewport_lifecycle_types_preserve_runtime_boundaries() {
        let main = space("main");
        let window = handle(7);
        let open =
            DockViewportOpenOutcome::new(main.clone(), window, DockViewportOpenStatus::Opened);
        assert_eq!(open.space(), &main);
        assert_eq!(open.window(), window);
        assert_eq!(open.status(), DockViewportOpenStatus::Opened);
        assert_eq!(
            DockViewportClosePolicy::default(),
            DockViewportClosePolicy::RetainLayout
        );

        let close = DockViewportCloseOutcome::new(
            Some(main.clone()),
            window.window_id(),
            DockViewportCloseStatus::Closed,
        );
        assert_eq!(close.space(), Some(&main));
        assert_eq!(close.window_id(), window.window_id());
        assert_eq!(close.status(), DockViewportCloseStatus::Closed);

        let unregister = DockViewportUnregisterOutcome {
            space: main,
            window,
            reason: DockViewportUnregisterReason::Closed,
        };
        assert_eq!(unregister.reason, DockViewportUnregisterReason::Closed);
    }

    #[test]
    fn should_close_policy_reports_pre_close_veto_without_mutating_mapping() {
        let mut adapter = DockViewportAdapter::new();
        let main = space("main");
        let window = handle(1);
        register_viewport(&mut adapter, main.clone(), window);

        let allowed = adapter
            .should_close_viewport(window.window_id(), DockViewportClosePolicy::RetainLayout);
        assert_eq!(
            allowed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Allowed,
            }
        );
        assert!(allowed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let vetoed =
            adapter.should_close_viewport(window.window_id(), DockViewportClosePolicy::Prevent);
        assert_eq!(
            vetoed,
            DockViewportShouldCloseOutcome {
                space: Some(main.clone()),
                window_id: window.window_id(),
                status: DockViewportShouldCloseStatus::Vetoed,
            }
        );
        assert!(!vetoed.allows_close());
        assert_eq!(adapter.window_for_space(&main), Some(window));

        let unknown =
            adapter.should_close_viewport(WindowId::from(99), DockViewportClosePolicy::Prevent);
        assert_eq!(unknown.status, DockViewportShouldCloseStatus::UnknownWindow);
        assert!(unknown.allows_close());
    }

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
            let mut workspace = crate::DockWorkspace::new(source.clone(), graph);
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
