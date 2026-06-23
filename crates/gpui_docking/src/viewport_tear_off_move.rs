use crate::{
    DockActionApplyError, DockActionOutcome, DockGraph, DockGraphDropTarget,
    DockGraphMutationError, DockNode, DockOp, DockSpaceId, DockViewportDropPayload,
    DockViewportTearOffPending, DockViewportTearOffRequest, DockWorkspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffSourceStatus {
    Ready,
    Unavailable,
}

pub(crate) fn validate_tear_off_request(
    graph: &DockGraph,
    request: &DockViewportTearOffRequest,
) -> Result<(), DockActionApplyError> {
    match request.payload() {
        DockViewportDropPayload::Item(item) => {
            let source_tabs = request.source_node();
            if graph
                .find_item_in_space(request.source_space(), item)
                .is_none_or(|(tabs, _)| tabs != source_tabs)
            {
                return Err(DockActionApplyError::ItemNotInTabs {
                    tabs: source_tabs,
                    item: item.clone(),
                });
            }
        }
        DockViewportDropPayload::Tabs => {
            let source_tabs = request.source_node();
            if graph
                .root_for_node_in_space(request.source_space(), source_tabs)
                .is_none()
                || !matches!(
                    graph.node(source_tabs),
                    Some(DockNode::Tabs { items, .. }) if !items.is_empty()
                )
            {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_tabs,
                ));
            }
        }
        DockViewportDropPayload::Floating(floating) => {
            let source_floating = request.source_node();
            if source_floating != *floating {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_floating,
                ));
            }
            if graph
                .floating_containers(request.source_space())
                .iter()
                .all(|container| container.node != *floating)
            {
                return Err(DockGraphMutationError::FloatingContainerNotFound {
                    space: request.source_space().clone(),
                    floating: *floating,
                }
                .into());
            }
            if graph.collect_items_in_subtree(*floating).is_empty() {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_floating,
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn tear_off_source_status(
    graph: &DockGraph,
    pending: &DockViewportTearOffPending,
) -> DockViewportTearOffSourceStatus {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportTearOffMovePlan {
    op: DockOp,
}

impl DockViewportTearOffMovePlan {
    fn new(
        workspace: &DockWorkspace,
        request: &DockViewportTearOffRequest,
        target_space: &DockSpaceId,
    ) -> Result<Self, DockActionApplyError> {
        Ok(Self {
            op: build_tear_off_move_op(workspace, request, target_space)?,
        })
    }

    fn preflight(
        &self,
        workspace: &DockWorkspace,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let mut next = workspace.graph().clone();
        let changed = next.apply_op_checked(&self.op)?;
        Ok(DockActionOutcome::from_changed(changed))
    }

    fn commit(
        self,
        workspace: &mut DockWorkspace,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        workspace.commit_graph_op(self.op)
    }
}

pub(crate) fn commit_tear_off_move(
    workspace: &mut DockWorkspace,
    pending: &DockViewportTearOffPending,
) -> Result<DockActionOutcome, DockActionApplyError> {
    DockViewportTearOffMovePlan::new(workspace, pending.request(), pending.target_space())?
        .commit(workspace)
}

fn build_tear_off_move_op(
    workspace: &DockWorkspace,
    request: &DockViewportTearOffRequest,
    target_space: &DockSpaceId,
) -> Result<DockOp, DockActionApplyError> {
    validate_tear_off_request(workspace.graph(), request)?;
    workspace.policy().validate_platform_viewports()?;

    match request.payload() {
        DockViewportDropPayload::Item(item) => {
            workspace
                .move_validation()
                .validate_item_target_space(target_space, item)?;
            Ok(DockOp::MoveItem {
                source_space: request.source_space().clone(),
                item: item.clone(),
                target_space: target_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
        }
        DockViewportDropPayload::Tabs => {
            workspace
                .move_validation()
                .validate_tabs_target_space(target_space, request.source_node())?;
            Ok(DockOp::MoveTabs {
                source_space: request.source_space().clone(),
                source_tabs: request.source_node(),
                target_space: target_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
        }
        DockViewportDropPayload::Floating(floating) => {
            workspace
                .move_validation()
                .validate_floating_target_space(target_space, *floating)?;
            Ok(DockOp::MoveFloating {
                source_space: request.source_space().clone(),
                floating: *floating,
                target_space: target_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
        }
    }
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
        DockClassId, DockItemId, DockPanelDescriptor, DockPolicyError,
        DockViewportTearOffBeginOutcome, DockViewportTearOffMachine,
    };
    use open_gpui::{point, px};

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
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
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a"), item("b")],
            "preflight should dry-run the planned graph op"
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
    fn commit_tear_off_move_moves_item_to_empty_target_space() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("detached");
        let (mut workspace, tabs) = item_workspace(vec![item("a"), item("b")]);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let request = request(source_space.clone(), tabs, item("a"));
        let mut machine = DockViewportTearOffMachine::default();
        let pending = match machine.begin(request, target_space.clone(), None, None) {
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
