use crate::{
    DockActionApplyError, DockActionOutcome, DockGraphDropTarget, DockItemId, DockNodeId, DockOp,
    DockSpaceId, DockWorkspace,
    locked_drop_identity::{DockLockedPayloadIdentity, DockLockedTargetIdentity},
    workspace_drop_target::{
        DockWorkspaceDropCommitTarget, DockWorkspaceDropCommitTargetKind,
        DockWorkspaceResolvedDropTarget, resolve_workspace_drop_commit_target,
    },
    workspace_move_validation::dock_target_validator,
};

pub(crate) struct DockWorkspacePayloadDropRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) payload: DockWorkspaceDropPayload<'a>,
    pub(crate) target: DockWorkspaceResolvedDropTarget,
    pub(crate) frozen_focus_item: Option<&'a DockItemId>,
}

pub(crate) struct DockWorkspaceLockedPayloadDropRequest<'a> {
    pub(crate) plan: DockWorkspaceLockedPayloadDropPlan,
    pub(crate) frozen_focus_item: Option<&'a DockItemId>,
}

#[derive(Clone, Debug)]
#[must_use = "a prepared locked payload drop must commit its projected graph"]
pub(crate) struct DockWorkspacePreparedLockedPayloadDrop {
    commit_id: DockWorkspaceLockedPayloadDropCommitId,
    expected_graph: crate::DockGraph,
    graph: crate::DockGraph,
    outcome: DockWorkspacePayloadDropOutcome,
}

impl DockWorkspacePreparedLockedPayloadDrop {
    pub(crate) const fn commit_id(&self) -> DockWorkspaceLockedPayloadDropCommitId {
        self.commit_id
    }

    pub(crate) fn space_is_empty(&self, space: &DockSpaceId) -> bool {
        self.graph.collect_items_in_space(space).is_empty()
    }

    pub(crate) fn commit_or_replay(
        &self,
        workspace: &mut DockWorkspace,
    ) -> Option<DockWorkspaceLockedPayloadDropCommitReceipt> {
        workspace.commit_or_replay_locked_payload_drop(
            self.commit_id,
            &self.expected_graph,
            self.graph.clone(),
            self.outcome.clone(),
        )
    }

    pub(crate) fn commit(&self, workspace: &mut DockWorkspace) -> DockWorkspacePayloadDropOutcome {
        let receipt = self
            .commit_or_replay(workspace)
            .expect("prepared locked payload drop must retain exact graph authority");
        let outcome = receipt.outcome.clone();
        workspace.retire_locked_payload_drop_commit(&receipt);
        outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockWorkspaceLockedPayloadDropCommitId(u64);

impl DockWorkspaceLockedPayloadDropCommitId {
    pub(crate) const fn new(generation: u64) -> Self {
        Self(generation)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DockWorkspaceLockedPayloadDropCommitReceipt {
    commit_id: DockWorkspaceLockedPayloadDropCommitId,
    outcome: DockWorkspacePayloadDropOutcome,
}

impl DockWorkspaceLockedPayloadDropCommitReceipt {
    pub(crate) const fn commit_id(&self) -> DockWorkspaceLockedPayloadDropCommitId {
        self.commit_id
    }

    pub(crate) fn outcome(&self) -> &DockWorkspacePayloadDropOutcome {
        &self.outcome
    }

    pub(crate) fn new(
        commit_id: DockWorkspaceLockedPayloadDropCommitId,
        outcome: DockWorkspacePayloadDropOutcome,
    ) -> Self {
        Self { commit_id, outcome }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockWorkspaceLockedPayloadDropPlan {
    source: DockLockedPayloadIdentity,
    target: DockLockedTargetIdentity,
}

impl DockWorkspaceLockedPayloadDropPlan {
    pub(crate) fn source_space(&self) -> &DockSpaceId {
        self.source.source_space()
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        self.target.target_space()
    }

    fn validate(&self, workspace: &DockWorkspace) -> Result<(), DockActionApplyError> {
        self.source.validate(workspace.graph())?;
        self.target.validate(workspace.graph())
    }

    fn graph_op(&self) -> DockOp {
        self.source
            .graph_op(self.target.target_space(), self.target.graph_target())
    }

    fn payload(&self) -> DockWorkspaceDropPayload<'_> {
        self.source.as_workspace_payload()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockWorkspacePayloadDropOutcome {
    action: DockActionOutcome,
    focus_item: Option<DockItemId>,
}

impl DockWorkspacePayloadDropOutcome {
    pub(crate) fn new(action: DockActionOutcome, focus_item: Option<DockItemId>) -> Self {
        Self { action, focus_item }
    }

    pub(crate) fn action(&self) -> DockActionOutcome {
        self.action
    }

    pub(crate) fn changed(&self) -> bool {
        self.action.changed()
    }

    pub(crate) fn focus_item(&self) -> Option<&DockItemId> {
        self.focus_item.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockWorkspaceDropPayload<'a> {
    Item {
        source_tabs: DockNodeId,
        item: &'a DockItemId,
    },
    Tabs {
        source_tabs: DockNodeId,
    },
    Floating {
        floating: DockNodeId,
    },
}

impl DockWorkspace {
    pub(crate) fn commit_resolved_payload_drop(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
    ) -> Result<DockWorkspacePayloadDropOutcome, DockActionApplyError> {
        let DockWorkspacePayloadDropRequest {
            source_space,
            payload,
            target,
            frozen_focus_item,
        } = request;

        let commit_target = self.lock_resolved_payload_drop_target(payload, target)?;
        let focus_target_space = commit_target.target_space().clone();
        let commit_target = commit_target.into_parts();
        let action = match commit_target.kind {
            DockWorkspaceDropCommitTargetKind::Graph(target) => self
                .commit_resolved_payload_graph_target_drop(
                    source_space,
                    payload,
                    &commit_target.target_space,
                    target,
                ),
            DockWorkspaceDropCommitTargetKind::EmptyDockSpace(space) => match payload {
                DockWorkspaceDropPayload::Item { item, .. } => {
                    self.commit_item_to_empty_dock_space(source_space, item, &space)
                }
                DockWorkspaceDropPayload::Tabs { source_tabs } => {
                    self.commit_tabs_to_empty_dock_space(source_space, source_tabs, &space)
                }
                DockWorkspaceDropPayload::Floating { floating } => {
                    self.commit_floating_to_empty_dock_space(source_space, floating, &space)
                }
            },
        }?;
        Ok(DockWorkspacePayloadDropOutcome::new(
            action,
            self.activation_focus_item_for_workspace_payload(
                &payload,
                Some(&focus_target_space),
                frozen_focus_item,
            ),
        ))
    }

    pub(crate) fn lock_resolved_payload_drop_target(
        &self,
        payload: DockWorkspaceDropPayload<'_>,
        target: DockWorkspaceResolvedDropTarget,
    ) -> Result<DockWorkspaceDropCommitTarget, DockActionApplyError> {
        let target_space = target.target_space().clone();
        let payload_classes = self.payload_dock_classes_for_workspace_payload(&payload);
        let target_validator =
            dock_target_validator(&target_space, &payload_classes, self.policy());
        resolve_workspace_drop_commit_target(self, target, Some(&target_validator))
    }

    pub(crate) fn lock_resolved_payload_drop(
        &self,
        source_space: &DockSpaceId,
        payload: DockWorkspaceDropPayload<'_>,
        target: DockWorkspaceResolvedDropTarget,
    ) -> Result<DockWorkspaceLockedPayloadDropPlan, DockActionApplyError> {
        let source = DockLockedPayloadIdentity::capture(self.graph(), source_space, payload)?;
        let target = self
            .lock_resolved_payload_drop_target(payload, target)?
            .into_locked_identity();
        let plan = DockWorkspaceLockedPayloadDropPlan { source, target };
        plan.validate(self)?;
        let mut graph = self.graph().clone();
        graph.apply_op_checked(&plan.graph_op())?;
        Ok(plan)
    }

    pub(crate) fn commit_locked_payload_drop(
        &mut self,
        request: DockWorkspaceLockedPayloadDropRequest<'_>,
    ) -> Result<DockWorkspacePayloadDropOutcome, DockActionApplyError> {
        let prepared = self.prepare_locked_payload_drop(request)?;
        Ok(self.commit_prepared_locked_payload_drop(prepared))
    }

    pub(crate) fn prepare_locked_payload_drop(
        &self,
        request: DockWorkspaceLockedPayloadDropRequest<'_>,
    ) -> Result<DockWorkspacePreparedLockedPayloadDrop, DockActionApplyError> {
        let DockWorkspaceLockedPayloadDropRequest {
            plan,
            frozen_focus_item,
        } = request;
        plan.validate(self)?;
        let focus_target_space = plan.target_space().clone();
        let focus_item = match plan.payload() {
            DockWorkspaceDropPayload::Item { item, .. } => Some((*item).clone()),
            DockWorkspaceDropPayload::Tabs { .. } | DockWorkspaceDropPayload::Floating { .. } => {
                frozen_focus_item.cloned()
            }
        };
        let expected_graph = self.graph().clone();
        let mut graph = expected_graph.clone();
        let action = DockActionOutcome::from_changed(graph.apply_op_checked(&plan.graph_op())?);
        let focus_item = focus_item.filter(|item| {
            graph
                .find_item_in_space(&focus_target_space, item)
                .is_some()
        });
        Ok(DockWorkspacePreparedLockedPayloadDrop {
            commit_id: self.allocate_locked_payload_drop_commit_id(),
            expected_graph,
            graph,
            outcome: DockWorkspacePayloadDropOutcome::new(action, focus_item),
        })
    }

    pub(crate) fn commit_prepared_locked_payload_drop(
        &mut self,
        prepared: DockWorkspacePreparedLockedPayloadDrop,
    ) -> DockWorkspacePayloadDropOutcome {
        prepared.commit(self)
    }

    fn commit_resolved_payload_graph_target_drop(
        &mut self,
        source_space: &DockSpaceId,
        payload: DockWorkspaceDropPayload<'_>,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match payload {
            DockWorkspaceDropPayload::Item { source_tabs, item } => {
                self.commit_tab_move(source_space, source_tabs, item, target_space, target)
            }
            DockWorkspaceDropPayload::Tabs { source_tabs } => {
                self.commit_tabs_move(source_space, source_tabs, target_space, target)
            }
            DockWorkspaceDropPayload::Floating { floating } => {
                self.commit_floating_move(source_space, floating, target_space, target)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockFloatingContainer, DockGraph, DockItemId, DockNode, DockPolicyError, DockSpaceId,
        DropZone, SplitAxis,
        drop_target::{DockDropResolveSource, DockResolvedDropTarget, DockResolvedDropTargetKind},
        workspace_drop_target::expected_drop_box_kind,
    };
    use open_gpui::{Bounds, point, px, size};

    fn space() -> DockSpaceId {
        DockSpaceId::from("main")
    }

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn bounds() -> Bounds<open_gpui::Pixels> {
        Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)))
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {actual} to be close to {expected}"
        );
    }

    fn split_workspace() -> (DockWorkspace, DockNodeId, DockNodeId, DockNodeId) {
        let mut graph = DockGraph::new();
        let left = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let right = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![left, right],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        (DockWorkspace::new(space(), graph), root, left, right)
    }

    fn root_edge_workspace(
        source_items: Vec<DockItemId>,
    ) -> (
        DockWorkspace,
        DockSpaceId,
        DockSpaceId,
        DockNodeId,
        DockNodeId,
        DockNodeId,
        DockNodeId,
    ) {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let selected = source_items.first().cloned();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: source_items,
            selected,
        });
        let target_left = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let target_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("d")],
            selected: Some(item("d")),
        });
        let target_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_left, target_right],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(source_space.clone(), source_tabs);
        graph.set_root(target_space.clone(), target_root);

        (
            DockWorkspace::new(source_space.clone(), graph),
            source_space,
            target_space,
            source_tabs,
            target_root,
            target_left,
            target_right,
        )
    }

    fn resolved_target(kind: DockResolvedDropTargetKind) -> DockResolvedDropTarget {
        let drop_box = expected_drop_box_kind(&kind).map(|kind| crate::geometry::DockDropBox {
            kind,
            hit_bounds: bounds(),
            draw_bounds: bounds(),
            preview_bounds: bounds(),
        });
        let edge_sizing = match kind {
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. } => {
                Some(crate::DockEdgeDockSizing::fallback())
            }
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        };
        DockResolvedDropTarget {
            kind,
            source: DockDropResolveSource::LeafBody,
            target_bounds: Some(bounds()),
            inner_target_bounds: Some(bounds()),
            availability: crate::drop_target::DockResolvedDropTargetAvailability::all(),
            drop_box,
            hit_bounds: Some(bounds()),
            preview_bounds: Some(bounds()),
            tab_insertion_bounds: None,
            edge_sizing,
            edge_plan: None,
            is_central_region: false,
        }
    }

    fn workspace_target(
        target_space: &DockSpaceId,
        target: DockResolvedDropTarget,
    ) -> DockWorkspaceResolvedDropTarget {
        DockWorkspaceResolvedDropTarget::new(target_space.clone(), target)
    }

    fn workspace_kind_target(
        target_space: &DockSpaceId,
        kind: DockResolvedDropTargetKind,
    ) -> DockWorkspaceResolvedDropTarget {
        workspace_target(target_space, resolved_target(kind))
    }

    fn resolved_edge_target_with_plan(
        workspace: &DockWorkspace,
        target_space: &DockSpaceId,
        target_node: DockNodeId,
        kind: DockResolvedDropTargetKind,
    ) -> DockResolvedDropTarget {
        let mut target = resolved_target(kind);
        let zone = target.zone().expect("edge target should expose a zone");
        let sizing = target
            .edge_sizing
            .expect("edge target helper should attach sizing");
        target.edge_plan = Some(
            workspace
                .graph()
                .edge_dock_plan_with_sizing(target_space, target_node, zone, sizing)
                .expect("edge target helper should build a preview plan"),
        );
        target
    }

    #[test]
    fn resolved_targets_expose_center_tabs_for_reorder_holds() {
        let (_workspace, root, left, right) = split_workspace();

        assert_eq!(
            resolved_target(DockResolvedDropTargetKind::TabBar {
                target_tabs: left,
                insert_index: 1,
            })
            .center_target_tabs(),
            Some(left)
        );
        assert_eq!(
            resolved_target(DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: right,
            })
            .center_target_tabs(),
            Some(right)
        );
        assert_eq!(
            resolved_target(DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs: right,
                zone: DropZone::Right,
            })
            .center_target_tabs(),
            None
        );
        assert_eq!(
            resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                space: DockSpaceId::from("detached"),
            })
            .center_target_tabs(),
            None
        );
    }

    #[test]
    fn resolved_center_target_moves_item_without_graph_shaped_action() {
        let (mut workspace, root, left, right) = split_workspace();

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::LeafCenter {
                        root,
                        target_tabs: right,
                    },
                ),
            })
            .expect("resolved center drop should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert_eq!(outcome.focus_item(), Some(&item("a")));
        let DockNode::Tabs { items, selected } =
            workspace.graph().node(right).expect("target should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(1));
    }

    #[test]
    fn resolved_tab_bar_target_reorders_same_stack() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b"), item("c")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), tabs);
        let mut workspace = DockWorkspace::new(space(), graph);

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: tabs,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::TabBar {
                        target_tabs: tabs,
                        insert_index: 3,
                    },
                ),
            })
            .expect("same-stack reorder should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        let DockNode::Tabs { items, selected } =
            workspace.graph().node(tabs).expect("tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(2));
    }

    #[test]
    fn prepared_locked_drop_projects_without_mutating_before_infallible_commit() {
        let (mut workspace, _root, left, right) = split_workspace();
        let item_a = item("a");
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item_a,
                },
                workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::TabBar {
                        target_tabs: right,
                        insert_index: 1,
                    },
                ),
            )
            .expect("the exact source and target should lock");

        let prepared = workspace
            .prepare_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect("the locked drop should project against a graph clone");

        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")],
            "prepare must not mutate the authoritative graph"
        );

        let outcome = workspace.commit_prepared_locked_payload_drop(prepared);

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert_eq!(outcome.focus_item(), Some(&item_a));
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("b"), item("a")]
        );
        assert_eq!(workspace.locked_payload_drop_commit_count(), 0);
    }

    #[test]
    fn locked_tab_bar_target_rejects_a_changed_target_tab_sequence() {
        let (mut workspace, _root, left, right) = split_workspace();
        let item_a = item("a");
        let payload = DockWorkspaceDropPayload::Item {
            source_tabs: left,
            item: &item_a,
        };
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                payload,
                workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::TabBar {
                        target_tabs: right,
                        insert_index: 1,
                    },
                ),
            )
            .expect("the original target tab gap should lock");

        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: space(),
                target_tabs: Some(right),
                item: item("x"),
                insert_index: Some(0),
            })
            .expect("the target stack mutation should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a frozen tab gap must not redirect after the target sequence changes");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("x"), item("b")]
        );
    }

    #[test]
    fn locked_tabs_drop_rejects_changed_payload_items() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        workspace.policy_mut().set_allow_platform_viewports(true);
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                DockWorkspaceDropPayload::Tabs { source_tabs: left },
                workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            )
            .expect("the original tabs payload should lock");

        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: space(),
                target_tabs: Some(left),
                item: item("x"),
                insert_index: None,
            })
            .expect("the listener-time source mutation should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a locked tabs payload must retain its exact ordered items");

        assert_eq!(
            err,
            DockActionApplyError::DropPayloadMismatch {
                space: space(),
                tabs: left,
            }
        );
        assert_eq!(workspace.graph().root(&detached), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("x"), item("b")]
        );
    }

    #[test]
    fn locked_floating_drop_rejects_changed_payload_items() {
        let detached = DockSpaceId::from("detached");
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
        graph.set_root(space(), root_tabs);
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(),
            });
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                DockWorkspaceDropPayload::Floating { floating },
                workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            )
            .expect("the original floating payload should lock");

        workspace
            .commit_graph_op(DockOp::OpenItem {
                space: space(),
                target_tabs: Some(floating_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("the listener-time floating mutation should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a locked floating payload must retain its exact ordered items");

        assert_eq!(
            err,
            DockActionApplyError::DropPayloadMismatch {
                space: space(),
                tabs: floating,
            }
        );
        assert_eq!(workspace.graph().root(&detached), None);
        assert_eq!(
            workspace.graph().collect_items_in_subtree(floating),
            vec![item("a"), item("x")]
        );
    }

    #[test]
    fn locked_leaf_center_rejects_reparented_target_tabs() {
        let (
            mut workspace,
            source_space,
            target_space,
            source_tabs,
            target_root,
            target_left,
            _target_right,
        ) = root_edge_workspace(vec![item("a")]);
        let item_a = item("a");
        let plan = workspace
            .lock_resolved_payload_drop(
                &source_space,
                DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                workspace_kind_target(
                    &target_space,
                    DockResolvedDropTargetKind::LeafCenter {
                        root: target_root,
                        target_tabs: target_left,
                    },
                ),
            )
            .expect("the original leaf-center owner should lock");

        workspace
            .commit_graph_op(DockOp::FloatTabsInWindow {
                source_space: target_space.clone(),
                source_tabs: target_left,
                target_space: target_space.clone(),
                bounds: bounds(),
            })
            .expect("the listener-time target reparent should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a leaf-center target must retain its original root owner");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
        assert!(
            workspace
                .graph()
                .find_item_in_space(&target_space, &item("b"))
                .is_some(),
            "the listener-time target reparent must remain intact after rejection"
        );
    }

    #[test]
    fn locked_floating_title_bar_rejects_reparented_target_tabs() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let floating = graph.insert_node(DockNode::Floating { child: target_tabs });
        graph.set_root(space(), source_tabs);
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(),
            });
        let mut workspace = DockWorkspace::new(space(), graph);
        let item_a = item("a");
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::FloatingTitleBar {
                        floating,
                        target_tabs,
                    },
                ),
            )
            .expect("the original floating title-bar owner should lock");
        let edge_plan = workspace
            .graph()
            .edge_dock_plan(&space(), source_tabs, DropZone::Right)
            .expect("the floating subtree should be dockable beside the root");

        workspace
            .commit_graph_op(DockOp::MoveFloating {
                source_space: space(),
                floating,
                target_space: space(),
                target: DockGraphDropTarget::edge(edge_plan),
            })
            .expect("the listener-time floating reparent should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a floating title-bar target must retain its original owner");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().find_item_in_space(&space(), &item("a")),
            Some((source_tabs, 0))
        );
        assert_eq!(
            workspace.graph().find_item_in_space(&space(), &item("b")),
            Some((target_tabs, 0))
        );
    }

    #[test]
    fn resolved_central_center_target_respects_policy_before_mutation() {
        let (mut workspace, _root, left, right) = split_workspace();
        workspace
            .policy_mut()
            .set_allow_central_region_dock_over(false);
        let mut target = resolved_target(DockResolvedDropTargetKind::LeafCenter {
            root: right,
            target_tabs: right,
        });
        target.is_central_region = true;

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_target(&space(), target),
            })
            .expect_err("central center target should obey current policy");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::CentralRegionDockOverDisabled)
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_central_tab_bar_target_respects_policy_before_mutation() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b"), item("c")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), tabs);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace
            .policy_mut()
            .set_allow_central_region_dock_over(false);
        let mut target = resolved_target(DockResolvedDropTargetKind::TabBar {
            target_tabs: tabs,
            insert_index: 3,
        });
        target.is_central_region = true;

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: tabs,
                    item: &item("a"),
                },
                target: workspace_target(&space(), target),
            })
            .expect_err("central tab bar target should obey current policy");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::CentralRegionDockOverDisabled)
        );
        let DockNode::Tabs { items, selected } =
            workspace.graph().node(tabs).expect("tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("b"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(0));
    }

    #[test]
    fn resolved_edge_target_respects_policy_before_mutation() {
        let (mut workspace, _root, left, right) = split_workspace();
        workspace.policy_mut().set_allow_edge_split(false);

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::InnerEdge {
                        root: right,
                        target_tabs: right,
                        zone: DropZone::Right,
                    },
                ),
            })
            .expect_err("edge split policy should reject resolved target");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::EdgeSplitDisabled)
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_edge_target_requires_matching_drop_box_metadata() {
        let (mut workspace, _root, left, right) = split_workspace();
        let mut target = resolved_target(DockResolvedDropTargetKind::InnerEdge {
            root: right,
            target_tabs: right,
            zone: DropZone::Right,
        });
        target.drop_box = None;

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_target(&space(), target),
            })
            .expect_err("edge target without drop box should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_edge_target_requires_matching_preview_bounds_metadata() {
        let (mut workspace, _root, left, right) = split_workspace();
        let mut target = resolved_target(DockResolvedDropTargetKind::InnerEdge {
            root: right,
            target_tabs: right,
            zone: DropZone::Right,
        });
        target.preview_bounds = Some(Bounds::new(
            point(px(1.0), px(1.0)),
            size(px(50.0), px(50.0)),
        ));

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_target(&space(), target),
            })
            .expect_err("edge target with mismatched preview bounds should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_root_edge_drop_carries_sizing_plan_to_graph() {
        let (
            mut workspace,
            source_space,
            target_space,
            source_tabs,
            target_root,
            target_left,
            target_right,
        ) = root_edge_workspace(vec![item("a")]);
        let item_a = item("a");
        let mut target = resolved_target(DockResolvedDropTargetKind::RootEdge {
            root: target_root,
            leaf_tabs: Some(target_right),
            zone: DropZone::Right,
        });
        let sizing = crate::DockEdgeDockSizing::from_extents(px(240.0), px(1000.0));
        target.edge_sizing = Some(sizing);
        target.edge_plan = Some(
            workspace
                .graph()
                .edge_dock_plan_with_sizing(&target_space, target_root, DropZone::Right, sizing)
                .expect("preview should build a root edge plan"),
        );

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                target: workspace_target(&target_space, target),
            })
            .expect("edge target with sizing should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        let DockNode::Split {
            axis,
            children,
            fractions,
        } = workspace
            .graph()
            .node(target_root)
            .expect("target root should exist")
        else {
            panic!("target root should remain a split");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(&children[0..2], &[target_left, target_right]);
        assert_tabs_items(&workspace, children[2], &[item("a")], DropZone::Right);
        assert_close(fractions[0], 0.38);
        assert_close(fractions[1], 0.38);
        assert_close(fractions[2], 0.24);
    }

    #[test]
    fn resolved_edge_target_requires_preview_edge_plan() {
        let (
            mut workspace,
            source_space,
            target_space,
            source_tabs,
            target_root,
            _target_left,
            target_right,
        ) = root_edge_workspace(vec![item("a")]);
        let item_a = item("a");
        let target = resolved_target(DockResolvedDropTargetKind::RootEdge {
            root: target_root,
            leaf_tabs: Some(target_right),
            zone: DropZone::Right,
        });

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                target: workspace_target(&target_space, target),
            })
            .expect_err("edge target without preview plan should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("b"), item("d")]
        );
    }

    #[test]
    fn resolved_edge_target_rejects_plan_that_no_longer_matches_preview_target() {
        let (
            mut workspace,
            source_space,
            target_space,
            source_tabs,
            target_root,
            target_left,
            target_right,
        ) = root_edge_workspace(vec![item("a")]);
        let item_a = item("a");
        let mut target = resolved_edge_target_with_plan(
            &workspace,
            &target_space,
            target_root,
            DockResolvedDropTargetKind::RootEdge {
                root: target_root,
                leaf_tabs: Some(target_left),
                zone: DropZone::Left,
            },
        );
        target.kind = DockResolvedDropTargetKind::RootEdge {
            root: target_root,
            leaf_tabs: Some(target_right),
            zone: DropZone::Right,
        };
        target.drop_box =
            expected_drop_box_kind(&target.kind).map(|kind| crate::geometry::DockDropBox {
                kind,
                hit_bounds: bounds(),
                draw_bounds: bounds(),
                preview_bounds: bounds(),
            });

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                target: workspace_target(&target_space, target),
            })
            .expect_err("edge target with mismatched cached plan should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&target_space),
            vec![item("b"), item("d")]
        );
    }

    #[test]
    fn resolved_leaf_target_requires_target_tabs_under_declared_root() {
        let (mut workspace, root, left, right) = split_workspace();

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::LeafCenter {
                        root: right,
                        target_tabs: right,
                    },
                ),
            })
            .expect_err("leaf center target with a fake root should not commit");

        assert_ne!(root, right);
        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_root_edge_rejects_inner_drop_box_metadata() {
        let (
            mut workspace,
            source_space,
            target_space,
            source_tabs,
            target_root,
            target_left,
            _target_right,
        ) = root_edge_workspace(vec![item("a")]);
        let item_a = item("a");
        let mut target = resolved_target(DockResolvedDropTargetKind::RootEdge {
            root: target_root,
            leaf_tabs: Some(target_left),
            zone: DropZone::Left,
        });
        target.drop_box = Some(crate::geometry::DockDropBox {
            kind: crate::geometry::DockDropBoxKind::InnerEdge(DropZone::Left),
            hit_bounds: bounds(),
            draw_bounds: bounds(),
            preview_bounds: bounds(),
        });

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                target: workspace_target(&target_space, target),
            })
            .expect_err("root edge with inner box metadata should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&source_space),
            vec![item("a")]
        );
    }

    #[test]
    fn resolved_empty_space_target_creates_detached_root_when_policy_allows() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        workspace.policy_mut().set_allow_platform_viewports(true);

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            })
            .expect("empty-space target should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&detached),
            vec![item("a")]
        );
    }

    #[test]
    fn resolved_empty_space_target_still_requires_current_platform_viewport_policy() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            })
            .expect_err("ordinary drops must keep current-policy commit semantics");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::PlatformViewportsDisabled)
        );
        assert_eq!(workspace.graph().root(&detached), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn locked_item_drop_rejects_a_stale_source_tabs_identity() {
        let (mut workspace, _root, left, right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        let item_a = item("a");
        workspace.policy_mut().set_allow_platform_viewports(true);
        let payload = DockWorkspaceDropPayload::Item {
            source_tabs: left,
            item: &item_a,
        };
        let plan = workspace
            .lock_resolved_payload_drop(
                &space(),
                payload,
                workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            )
            .expect("the original item source should lock");

        workspace
            .commit_tab_move(
                &space(),
                left,
                &item_a,
                &space(),
                DockGraphDropTarget::center(right),
            )
            .expect("the listener-time source move should commit");

        let err = workspace
            .commit_locked_payload_drop(DockWorkspaceLockedPayloadDropRequest {
                plan,
                frozen_focus_item: None,
            })
            .expect_err("a frozen item drop must retain its exact source tabs identity");

        assert_eq!(
            err,
            DockActionApplyError::ItemNotInTabs {
                tabs: left,
                item: item_a,
            }
        );
        assert_eq!(workspace.graph().root(&detached), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("b"), item("a")]
        );
    }

    #[test]
    fn resolved_empty_space_target_requires_route_space_to_match_target_space() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        workspace.policy_mut().set_allow_platform_viewports(true);

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            })
            .expect_err("empty-space target should not conflict with route target space");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(workspace.graph().root(&detached), None);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_empty_central_target_respects_central_dock_over_policy_before_mutation() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let central = DockSpaceId::from("central");
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace
            .policy_mut()
            .set_allow_central_region_dock_over(false);

        let mut target = resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
            space: central.clone(),
        });
        target.is_central_region = true;

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target: workspace_target(&central, target),
            })
            .expect_err("central empty-space target should obey central dock-over policy");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::CentralRegionDockOverDisabled)
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
        assert_eq!(workspace.graph().root(&central), None);
    }

    #[test]
    fn resolved_center_target_moves_tabs_stack_preserving_order_and_selected_item() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            selected: Some(item("c")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![source_tabs, target_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        let mut workspace = DockWorkspace::new(space(), graph);

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::LeafCenter { root, target_tabs },
                ),
            })
            .expect("resolved center stack drop should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        let DockNode::Tabs { items, selected } = workspace
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(2));
    }

    #[test]
    fn resolved_center_target_moves_tabs_stack_with_frozen_focus_item() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            selected: Some(item("c")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![source_tabs, target_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        let mut workspace = DockWorkspace::new(space(), graph);
        let focused = item("c");

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: Some(&focused),
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::LeafCenter { root, target_tabs },
                ),
            })
            .expect("resolved center stack drop should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert_eq!(outcome.focus_item(), Some(&focused));
    }

    #[test]
    fn resolved_empty_space_target_moves_tabs_stack_when_policy_allows() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            selected: Some(item("c")),
        });
        graph.set_root(space(), source_tabs);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let detached = DockSpaceId::from("detached");

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target: workspace_kind_target(
                    &detached,
                    DockResolvedDropTargetKind::EmptyDockSpace {
                        space: detached.clone(),
                    },
                ),
            })
            .expect("resolved empty-space stack drop should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        let detached_root = workspace
            .graph()
            .root(&detached)
            .expect("detached space should have a root");
        let DockNode::Tabs { items, selected } = workspace
            .graph()
            .node(detached_root)
            .expect("detached root should exist")
        else {
            panic!("detached root should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(1));
    }

    #[test]
    fn resolved_root_edge_targets_commit_against_workspace_root_for_all_edges() {
        for move_tabs in [false, true] {
            for zone in [
                DropZone::Left,
                DropZone::Right,
                DropZone::Top,
                DropZone::Bottom,
            ] {
                let source_items = if move_tabs {
                    vec![item("a"), item("c")]
                } else {
                    vec![item("a")]
                };
                let expected_items = source_items.clone();
                let (
                    mut workspace,
                    source_space,
                    target_space,
                    source_tabs,
                    target_root,
                    target_left,
                    target_right,
                ) = root_edge_workspace(source_items);
                let item_a = item("a");
                let payload = if move_tabs {
                    DockWorkspaceDropPayload::Tabs { source_tabs }
                } else {
                    DockWorkspaceDropPayload::Item {
                        source_tabs,
                        item: &item_a,
                    }
                };
                let leaf_tabs = match zone {
                    DropZone::Left => target_left,
                    DropZone::Right | DropZone::Top | DropZone::Bottom => target_right,
                    DropZone::Center => unreachable!(),
                };
                let target = resolved_edge_target_with_plan(
                    &workspace,
                    &target_space,
                    target_root,
                    DockResolvedDropTargetKind::RootEdge {
                        root: target_root,
                        leaf_tabs: Some(leaf_tabs),
                        zone,
                    },
                );

                let outcome = workspace
                    .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                        frozen_focus_item: None,
                        source_space: &source_space,
                        payload,
                        target: workspace_target(&target_space, target),
                    })
                    .unwrap_or_else(|error| {
                        panic!("{zone:?} root-edge payload drop should commit: {error}")
                    });

                assert_eq!(outcome.action(), DockActionOutcome::Changed, "{zone:?}");
                assert_root_edge_commit_graph(
                    &workspace,
                    &target_space,
                    target_root,
                    target_left,
                    target_right,
                    zone,
                    &expected_items,
                );
            }
        }
    }

    #[test]
    fn resolved_same_space_root_edge_tabs_move_preserves_moved_stack() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            selected: Some(item("c")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![source_tabs, target_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        let mut workspace = DockWorkspace::new(space(), graph);

        let target = resolved_edge_target_with_plan(
            &workspace,
            &space(),
            root,
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: Some(target_tabs),
                zone: DropZone::Right,
            },
        );

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target: workspace_target(&space(), target),
            })
            .expect("same-space root-edge tabs move should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("b"), item("a"), item("c")]
        );
        let moved_tabs = workspace
            .graph()
            .find_item_in_space(&space(), &item("a"))
            .expect("moved item should remain reachable")
            .0;
        assert_eq!(
            workspace.graph().find_item_in_space(&space(), &item("c")),
            Some((moved_tabs, 1))
        );
        let DockNode::Tabs { items, selected } = workspace
            .graph()
            .node(moved_tabs)
            .expect("moved tabs should remain present")
        else {
            panic!("moved root-edge payload should stay a tabs node");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(selected.as_ref(), items.get(1));
    }

    #[test]
    fn resolved_root_edge_target_moves_floating_subtree_without_flattening() {
        let source_space = DockSpaceId::from("source");
        let target_space = DockSpaceId::from("target");
        let mut graph = DockGraph::new();
        let floating_left = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let floating_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        let floating_child = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Vertical,
            children: vec![floating_left, floating_right],
            fractions: vec![0.4, 0.6],
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_child,
        });
        graph
            .floating_containers_mut(source_space.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(),
            });
        let target_left = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let target_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("d")],
            selected: Some(item("d")),
        });
        let target_root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_left, target_right],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(target_space.clone(), target_root);
        let mut workspace = DockWorkspace::new(source_space.clone(), graph);

        let target = resolved_edge_target_with_plan(
            &workspace,
            &target_space,
            target_root,
            DockResolvedDropTargetKind::RootEdge {
                root: target_root,
                leaf_tabs: None,
                zone: DropZone::Right,
            },
        );

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Floating { floating },
                target: workspace_target(&target_space, target),
            })
            .expect("floating root-edge drop should commit");

        assert_eq!(outcome.action(), DockActionOutcome::Changed);
        assert!(
            workspace
                .graph()
                .floating_containers(&source_space)
                .is_empty()
        );
        let DockNode::Split { axis, children, .. } = workspace
            .graph()
            .node(target_root)
            .expect("target root should remain present")
        else {
            panic!("target root should remain a split");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children, &vec![target_left, target_right, floating_child]);
        let DockNode::Split { axis, children, .. } = workspace
            .graph()
            .node(floating_child)
            .expect("floating child should be docked intact")
        else {
            panic!("floating child should remain split");
        };
        assert_eq!(*axis, SplitAxis::Vertical);
        assert_eq!(children, &vec![floating_left, floating_right]);
        assert_eq!(
            workspace.graph().collect_items_in_subtree(floating_child),
            vec![item("a"), item("c")]
        );
    }

    #[test]
    fn resolved_floating_title_target_requires_tabs_inside_declared_floating() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_tabs,
        });
        let unrelated_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![source_tabs, unrelated_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(),
            });
        let mut workspace = DockWorkspace::new(space(), graph);

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                frozen_focus_item: None,
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item("a"),
                },
                target: workspace_kind_target(
                    &space(),
                    DockResolvedDropTargetKind::FloatingTitleBar {
                        floating,
                        target_tabs: unrelated_tabs,
                    },
                ),
            })
            .expect_err("floating title target should bind tabs to its floating subtree");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("c"), item("b")]
        );
    }

    fn assert_root_edge_commit_graph(
        workspace: &DockWorkspace,
        target_space: &DockSpaceId,
        old_root: DockNodeId,
        target_left: DockNodeId,
        target_right: DockNodeId,
        zone: DropZone,
        expected_items: &[DockItemId],
    ) {
        match zone {
            DropZone::Left | DropZone::Right => {
                assert_eq!(
                    workspace.graph().root(target_space),
                    Some(old_root),
                    "{zone:?}"
                );
                let DockNode::Split { axis, children, .. } = workspace
                    .graph()
                    .node(old_root)
                    .expect("root split should still exist")
                else {
                    panic!("{zone:?}: root should remain a split");
                };
                assert_eq!(*axis, SplitAxis::Horizontal, "{zone:?}");
                assert_eq!(children.len(), 3, "{zone:?}");
                let moved_index = if zone == DropZone::Left { 0 } else { 2 };
                assert_tabs_items(workspace, children[moved_index], expected_items, zone);
                assert_eq!(
                    children
                        .iter()
                        .copied()
                        .filter(|child| *child == target_left)
                        .count(),
                    1,
                    "{zone:?}: left target should stay in the root split"
                );
                assert_eq!(
                    children
                        .iter()
                        .copied()
                        .filter(|child| *child == target_right)
                        .count(),
                    1,
                    "{zone:?}: right target should stay in the root split"
                );
            }
            DropZone::Top | DropZone::Bottom => {
                let new_root = workspace
                    .graph()
                    .root(target_space)
                    .expect("target space should keep a root");
                assert_ne!(new_root, old_root, "{zone:?}");
                let DockNode::Split { axis, children, .. } = workspace
                    .graph()
                    .node(new_root)
                    .expect("new root split should exist")
                else {
                    panic!("{zone:?}: target space should be wrapped in a new root split");
                };
                assert_eq!(*axis, SplitAxis::Vertical, "{zone:?}");
                assert_eq!(children.len(), 2, "{zone:?}");
                let (moved_index, old_root_index) = if zone == DropZone::Top {
                    (0, 1)
                } else {
                    (1, 0)
                };
                assert_tabs_items(workspace, children[moved_index], expected_items, zone);
                assert_eq!(children[old_root_index], old_root, "{zone:?}");
            }
            DropZone::Center => unreachable!(),
        }
    }

    fn assert_tabs_items(
        workspace: &DockWorkspace,
        tabs: DockNodeId,
        expected_items: &[DockItemId],
        zone: DropZone,
    ) {
        let DockNode::Tabs { items, .. } = workspace
            .graph()
            .node(tabs)
            .expect("moved tabs should exist")
        else {
            panic!("{zone:?}: moved child should be tabs");
        };
        assert_eq!(items, expected_items, "{zone:?}");
    }
}
