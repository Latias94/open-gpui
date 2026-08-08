use crate::{
    DockActionApplyError, DockActionOutcome, DockGraph, DockGraphDropTarget, DockNode, DockSpaceId,
    DockViewportDropPayload, DockViewportTearOffPending, DockViewportTearOffRequest, DockWorkspace,
    locked_drop_identity::{DockLockedPayloadForwardProjectionError, DockLockedPayloadIdentity},
    workspace_drop_transaction::DockWorkspaceDropPayload,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffSourceStatus {
    Ready,
    Unavailable,
}

fn capture_tear_off_payload_identity(
    graph: &DockGraph,
    request: &DockViewportTearOffRequest,
) -> Result<DockLockedPayloadIdentity, DockActionApplyError> {
    let payload = match request.payload() {
        DockViewportDropPayload::Item(item) => DockWorkspaceDropPayload::Item {
            source_tabs: request.source_node(),
            item,
        },
        DockViewportDropPayload::Tabs => DockWorkspaceDropPayload::Tabs {
            source_tabs: request.source_node(),
        },
        DockViewportDropPayload::Floating(floating) => {
            if request.source_node() != *floating {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    request.source_node(),
                ));
            }
            DockWorkspaceDropPayload::Floating {
                floating: *floating,
            }
        }
    };
    DockLockedPayloadIdentity::capture(graph, request.source_space(), payload)
}

pub(crate) fn tear_off_source_status(
    graph: &DockGraph,
    pending: &DockViewportTearOffPending,
) -> DockViewportTearOffSourceStatus {
    if let Some(plan) = pending.move_plan() {
        return plan.source_status(graph);
    }
    let request = pending.request();
    match request.payload() {
        DockViewportDropPayload::Item(item) => graph
            .find_item_in_space(request.source_space(), item)
            .map(|(tabs, _)| {
                if tabs == request.source_node() {
                    DockViewportTearOffSourceStatus::Ready
                } else {
                    DockViewportTearOffSourceStatus::Unavailable
                }
            })
            .unwrap_or_else(|| DockViewportTearOffSourceStatus::Unavailable),
        DockViewportDropPayload::Tabs => {
            let source_tabs = request.source_node();
            let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
                return DockViewportTearOffSourceStatus::Unavailable;
            };
            if graph
                .root_for_node_in_space(request.source_space(), source_tabs)
                .is_some()
                && !items.is_empty()
            {
                DockViewportTearOffSourceStatus::Ready
            } else {
                DockViewportTearOffSourceStatus::Unavailable
            }
        }
        DockViewportDropPayload::Floating(floating) => {
            if request.source_node() != *floating {
                return DockViewportTearOffSourceStatus::Unavailable;
            }
            if graph
                .floating_containers(request.source_space())
                .iter()
                .all(|container| container.node != *floating)
            {
                return DockViewportTearOffSourceStatus::Unavailable;
            }
            if !graph.collect_items_in_subtree(*floating).is_empty() {
                DockViewportTearOffSourceStatus::Ready
            } else {
                DockViewportTearOffSourceStatus::Unavailable
            }
        }
    }
}

pub(crate) fn preflight_tear_off_move(
    workspace: &DockWorkspace,
    request: &DockViewportTearOffRequest,
    target_space: &DockSpaceId,
) -> Result<DockActionOutcome, DockActionApplyError> {
    let plan = DockViewportTearOffMovePlan::new(workspace, request, target_space)?;
    plan.preflight(workspace)
}

pub(crate) fn lock_tear_off_move(
    workspace: &DockWorkspace,
    request: &DockViewportTearOffRequest,
    target_space: &DockSpaceId,
) -> Result<DockViewportTearOffMovePlan, DockActionApplyError> {
    let plan = DockViewportTearOffMovePlan::new(workspace, request, target_space)?;
    plan.preflight(workspace)?;
    Ok(plan)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffMovePlan {
    source: DockLockedPayloadIdentity,
    target_space: DockSpaceId,
}

impl DockViewportTearOffMovePlan {
    fn new(
        workspace: &DockWorkspace,
        request: &DockViewportTearOffRequest,
        target_space: &DockSpaceId,
    ) -> Result<Self, DockActionApplyError> {
        let source = capture_tear_off_payload_identity(workspace.graph(), request)?;
        validate_tear_off_move(workspace, request, target_space)?;
        Ok(Self {
            source,
            target_space: target_space.clone(),
        })
    }

    fn preflight(
        &self,
        workspace: &DockWorkspace,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let (_, changed) = self.project_graph(workspace)?;
        Ok(DockActionOutcome::from_changed(changed))
    }

    /// Builds the exact post-move graph without mutating the authoritative workspace.
    ///
    /// Live undock uses this projection to render the source-without-payload and payload-only
    /// provisional spaces while the durable graph remains unchanged. The same locked identity and
    /// operation are validated again by `commit` at the final swap boundary.
    pub(crate) fn project_graph(
        &self,
        workspace: &DockWorkspace,
    ) -> Result<(DockGraph, bool), DockActionApplyError> {
        self.source.validate(workspace.graph())?;
        let mut next = workspace.graph().clone();
        let changed = next.apply_op_checked(
            &self
                .source
                .graph_op(&self.target_space, DockGraphDropTarget::empty_space()),
        )?;
        Ok((next, changed))
    }

    /// Builds a forward-only projection from the payload's unique current graph locations.
    ///
    /// Callers must try `project_graph` first. This seam is only for crossing an already
    /// irreversible promotion boundary after synchronous reentry made the locked source identity
    /// stale while leaving every payload item uniquely reachable.
    pub(crate) fn project_graph_forward_rebased(
        &self,
        workspace: &DockWorkspace,
    ) -> Result<(DockGraph, bool), DockLockedPayloadForwardProjectionError> {
        self.source
            .project_forward_rebased_to_empty_space(workspace.graph(), &self.target_space)
    }

    pub(crate) fn source_identity(&self) -> &DockLockedPayloadIdentity {
        &self.source
    }

    fn commit(
        self,
        workspace: &mut DockWorkspace,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.source.validate(workspace.graph())?;
        workspace.commit_graph_op(
            self.source
                .graph_op(&self.target_space, DockGraphDropTarget::empty_space()),
        )
    }

    fn source_status(&self, graph: &DockGraph) -> DockViewportTearOffSourceStatus {
        if self.source.validate(graph).is_ok() {
            DockViewportTearOffSourceStatus::Ready
        } else {
            DockViewportTearOffSourceStatus::Unavailable
        }
    }
}

pub(crate) fn commit_tear_off_move(
    workspace: &mut DockWorkspace,
    pending: &DockViewportTearOffPending,
) -> Result<DockActionOutcome, DockActionApplyError> {
    let plan = match pending.move_plan() {
        Some(plan) => plan.clone(),
        None => {
            DockViewportTearOffMovePlan::new(workspace, pending.request(), pending.target_space())?
        }
    };
    plan.commit(workspace)
}

fn validate_tear_off_move(
    workspace: &DockWorkspace,
    request: &DockViewportTearOffRequest,
    target_space: &DockSpaceId,
) -> Result<(), DockActionApplyError> {
    workspace.policy().validate_platform_viewports()?;

    match request.payload() {
        DockViewportDropPayload::Item(item) => {
            workspace
                .move_validation()
                .validate_item_target_space(target_space, item)?;
        }
        DockViewportDropPayload::Tabs => {
            workspace
                .move_validation()
                .validate_tabs_target_space(target_space, request.source_node())?;
        }
        DockViewportDropPayload::Floating(floating) => {
            workspace
                .move_validation()
                .validate_floating_target_space(target_space, *floating)?;
        }
    }
    Ok(())
}

fn tear_off_payload_mismatch(
    source_space: &DockSpaceId,
    source_tabs: crate::DockNodeId,
) -> DockActionApplyError {
    DockActionApplyError::DropPayloadMismatch {
        space: source_space.clone(),
        tabs: source_tabs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockClassId, DockFloatingContainer, DockGraphMutationError, DockItemId, DockOp,
        DockPanelDescriptor, DockPolicyError, DockViewportTearOffBeginOutcome,
        DockViewportTearOffMachine,
    };
    use open_gpui::{Bounds, point, px, size};

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn bounds() -> Bounds<open_gpui::Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)))
    }

    fn request(
        source_space: DockSpaceId,
        source_tabs: crate::DockNodeId,
        item: DockItemId,
    ) -> DockViewportTearOffRequest {
        DockViewportTearOffRequest::new(
            source_space,
            source_tabs,
            DockViewportDropPayload::Item(item),
            point(px(900.0), px(900.0)),
            None,
        )
    }

    fn item_workspace(items: Vec<DockItemId>) -> (DockWorkspace, crate::DockNodeId) {
        let source_space = DockSpaceId::from("source");
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            selected: items.first().cloned(),
            items,
        });
        graph.set_root(source_space.clone(), tabs);
        (DockWorkspace::new(source_space, graph), tabs)
    }

    #[test]
    fn preflight_tear_off_move_rejects_platform_viewport_policy_without_mutating_graph() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (workspace, tabs) = item_workspace(vec![item("a")]);
        let request = request(source_space.clone(), tabs, item("a"));

        let err = preflight_tear_off_move(&workspace, &request, &target_space)
            .expect_err("platform viewport policy should block tear-off preflight");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::PlatformViewportsDisabled)
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert_eq!(workspace.graph().root(&target_space), None);
    }

    #[test]
    fn preflight_tear_off_move_rejects_non_empty_target_without_mutating_graph() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(source_space.clone(), tabs);
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("existing")],
            selected: Some(item("existing")),
        });
        graph.set_root(target_space.clone(), target_tabs);
        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));

        let err = preflight_tear_off_move(&workspace, &request, &target_space)
            .expect_err("non-empty target should block tear-off preflight");

        assert_eq!(
            err,
            DockActionApplyError::Graph(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone()
            })
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("existing")]
        );
    }

    #[test]
    fn preflight_tear_off_move_rejects_dock_class_policy_without_mutating_graph() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace
            .policy_mut()
            .set_allowed_dock_classes_for_space(target_space.clone(), ["inspector"]);
        workspace.register_panel_descriptor(
            item("a"),
            DockPanelDescriptor::new("Panel A").with_dock_class("editor"),
        );
        let request = request(source_space.clone(), tabs, item("a"));

        let err = preflight_tear_off_move(&workspace, &request, &target_space)
            .expect_err("dock class policy should block tear-off preflight");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
                space: target_space.clone(),
                item: item("a"),
                dock_class: Some(DockClassId::from("editor")),
            })
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert_eq!(workspace.graph().root(&target_space), None);
    }

    #[test]
    fn preflight_tear_off_move_succeeds_without_mutating_graph() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));

        let outcome = preflight_tear_off_move(&workspace, &request, &target_space)
            .expect("valid tear-off should pass preflight");

        assert_eq!(outcome, DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")]
        );
        assert_eq!(workspace.graph().root(&target_space), None);
    }

    #[test]
    fn tear_off_move_plan_preflights_without_mutating_then_commits() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));
        let plan = DockViewportTearOffMovePlan::new(&workspace, &request, &target_space)
            .expect("valid tear-off should produce a plan");

        assert_eq!(
            plan.preflight(&workspace)
                .expect("valid tear-off plan should preflight"),
            DockActionOutcome::Changed
        );
        let (projected, changed) = plan
            .project_graph(&workspace)
            .expect("valid tear-off plan should produce an immutable projection");
        assert!(changed);
        assert_eq!(
            projected.collect_items_in_space(&source_space),
            vec![item("b")]
        );
        assert_eq!(
            projected.collect_items_in_space(&target_space),
            vec![item("a")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")],
            "projection should not mutate the authoritative graph"
        );
        assert_eq!(
            plan.commit(&mut workspace)
                .expect("valid tear-off plan should commit"),
            DockActionOutcome::Changed
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("a")]
        );
    }

    #[test]
    fn pending_locked_tear_off_uses_mouse_up_policy_after_policy_tightens() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("MouseUp-time policy should admit the tear-off");
        let mut machine = DockViewportTearOffMachine::default();
        let pending = match machine.begin_with_move_plan(
            request,
            target_space.clone(),
            None,
            None,
            None,
            Some(plan),
        ) {
            DockViewportTearOffBeginOutcome::Pending(pending) => pending,
            DockViewportTearOffBeginOutcome::Duplicate(_) => {
                panic!("fresh locked tear-off request should not be duplicate")
            }
        };

        workspace.policy_mut().set_allow_platform_viewports(false);
        let outcome = commit_tear_off_move(&mut workspace, &pending)
            .expect("a locked tear-off must not reread post-MouseUp policy");

        assert_eq!(outcome, DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("a")]
        );
    }

    #[test]
    fn locked_tear_off_rejects_a_stale_source_tabs_identity() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let other_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: crate::SplitAxis::Horizontal,
            children: vec![source_tabs, other_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(source_space.clone(), root);
        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), source_tabs, item("a"));
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("the original source identity should lock");

        workspace
            .commit_tab_move(
                &source_space,
                source_tabs,
                &item("a"),
                &source_space,
                DockGraphDropTarget::center(other_tabs),
            )
            .expect("the listener-time source move should commit");
        let err = plan
            .commit(&mut workspace)
            .expect_err("the locked tear-off must reject a stale source tabs identity");

        assert_eq!(
            err,
            DockActionApplyError::ItemNotInTabs {
                tabs: source_tabs,
                item: item("a"),
            }
        );
        assert_eq!(workspace.graph().root(&target_space), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("b"), item("a")]
        );
    }

    #[test]
    fn forward_rebased_item_projection_follows_unique_reentered_location() {
        let source_space = DockSpaceId::from("source");
        let reentered_space = DockSpaceId::from("listener");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, source_tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), source_tabs, item("a"));
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("the original item tear-off should lock");

        workspace
            .commit_graph_op(DockOp::MoveItem {
                source_space: source_space.clone(),
                item: item("a"),
                target_space: reentered_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
            .expect("listener-time item rehome should commit");
        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: source_space.clone(),
                target_tabs: Some(source_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("an unrelated listener-time graph change should commit");

        assert!(plan.project_graph(&workspace).is_err());
        let (projected, changed) = plan
            .project_graph_forward_rebased(&workspace)
            .expect("the unique current item location should allow forward rebase");

        assert!(changed);
        assert_eq!(
            projected.collect_items_in_space(&source_space),
            vec![item("b"), item("x")]
        );
        assert_eq!(projected.collect_items_in_space(&reentered_space), []);
        assert_eq!(
            projected.collect_items_in_space(&target_space),
            vec![item("a")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&reentered_space),
            vec![item("a")],
            "forward projection must not mutate the current authoritative graph"
        );
        assert_eq!(workspace.graph().root(&target_space), None);
    }

    #[test]
    fn forward_rebased_tabs_projection_preserves_unrelated_reentry_changes() {
        let source_space = DockSpaceId::from("source");
        let reentered_space = DockSpaceId::from("listener");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, source_tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = DockViewportTearOffRequest::new(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            point(px(900.0), px(900.0)),
            None,
        );
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("the original tabs tear-off should lock");

        workspace
            .commit_graph_op(DockOp::MoveTabs {
                source_space: source_space.clone(),
                source_tabs,
                target_space: reentered_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
            .expect("listener-time tabs rehome should commit");
        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: source_space.clone(),
                target_tabs: None,
                item: item("x"),
                insert_index: None,
            })
            .expect("an unrelated listener-time source change should commit");

        assert!(plan.project_graph(&workspace).is_err());
        let (projected, changed) = plan
            .project_graph_forward_rebased(&workspace)
            .expect("the uniquely rehomed tabs items should allow forward rebase");

        assert!(changed);
        assert_eq!(
            projected.collect_items_in_space(&source_space),
            vec![item("x")]
        );
        assert_eq!(projected.collect_items_in_space(&reentered_space), []);
        assert_eq!(
            projected.collect_items_in_space(&target_space),
            vec![item("a"), item("b")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&reentered_space),
            vec![item("a"), item("b")],
            "forward projection must retain the authoritative reentry graph"
        );
        assert_eq!(workspace.graph().root(&target_space), None);
    }

    #[test]
    fn locked_tabs_tear_off_rejects_changed_payload_items() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, source_tabs) = item_workspace(vec![item("a")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = DockViewportTearOffRequest::new(
            source_space.clone(),
            source_tabs,
            DockViewportDropPayload::Tabs,
            point(px(900.0), px(900.0)),
            None,
        );
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("the original tabs tear-off should lock");

        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: source_space.clone(),
                target_tabs: Some(source_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("the listener-time tabs mutation should commit");
        let err = plan
            .commit(&mut workspace)
            .expect_err("the locked tear-off must reject changed tabs contents");

        assert_eq!(
            err,
            DockActionApplyError::DropPayloadMismatch {
                space: source_space.clone(),
                tabs: source_tabs,
            }
        );
        assert_eq!(workspace.graph().root(&target_space), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("x")]
        );
    }

    #[test]
    fn locked_floating_tear_off_rejects_changed_payload_items() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let mut graph = DockGraph::new();
        let root_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_tabs,
        });
        graph.set_root(source_space.clone(), root_tabs);
        graph
            .floating_containers_mut(source_space.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(),
            });
        let mut workspace = DockWorkspace::new(source_space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = DockViewportTearOffRequest::new(
            source_space.clone(),
            floating,
            DockViewportDropPayload::Floating(floating),
            point(px(900.0), px(900.0)),
            None,
        );
        let plan = lock_tear_off_move(&workspace, &request, &target_space)
            .expect("the original floating tear-off should lock");

        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: source_space.clone(),
                target_tabs: Some(floating_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("the listener-time floating mutation should commit");
        let err = plan
            .commit(&mut workspace)
            .expect_err("the locked tear-off must reject changed floating contents");

        assert_eq!(
            err,
            DockActionApplyError::DropPayloadMismatch {
                space: source_space.clone(),
                tabs: floating,
            }
        );
        assert_eq!(workspace.graph().root(&target_space), None);
        assert_eq!(
            workspace.graph().collect_items_in_subtree(floating),
            vec![item("a"), item("x")]
        );
    }

    #[test]
    fn commit_tear_off_move_moves_item_to_empty_target_space() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));
        let mut machine = DockViewportTearOffMachine::default();
        let pending = match machine.begin(request, target_space.clone(), None, None, None) {
            DockViewportTearOffBeginOutcome::Pending(pending) => pending,
            DockViewportTearOffBeginOutcome::Duplicate(_) => {
                panic!("fresh tear-off request should not be duplicate")
            }
        };

        let outcome = commit_tear_off_move(&mut workspace, &pending)
            .expect("valid tear-off commit should move the payload");

        assert_eq!(outcome, DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("b")]
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("a")]
        );
    }
}
