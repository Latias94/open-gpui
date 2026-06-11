use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockSpaceId, DockWorkspace,
    DropZone,
    drop_target::{DockResolvedDropTarget, DockResolvedDropTargetKind},
    workspace_move_transaction::{DockWorkspaceMoveTabRequest, DockWorkspaceMoveTabsRequest},
};

#[cfg(test)]
pub(crate) struct DockWorkspaceDropRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) item: &'a DockItemId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target: DockResolvedDropTarget,
}

pub(crate) struct DockWorkspacePayloadDropRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) payload: DockWorkspaceDropPayload<'a>,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target: DockResolvedDropTarget,
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
}

impl DockWorkspace {
    #[cfg(test)]
    pub(crate) fn commit_resolved_drop(
        &mut self,
        request: DockWorkspaceDropRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let DockWorkspaceDropRequest {
            source_space,
            source_tabs,
            item,
            target_space,
            target,
        } = request;

        self.commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
            source_space,
            payload: DockWorkspaceDropPayload::Item { source_tabs, item },
            target_space,
            target,
        })
    }

    pub(crate) fn commit_resolved_payload_drop(
        &mut self,
        request: DockWorkspacePayloadDropRequest<'_>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let DockWorkspacePayloadDropRequest {
            source_space,
            payload,
            target_space,
            target,
        } = request;

        match target.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => self.commit_resolved_payload_tabs_target_drop(
                source_space,
                payload,
                target_space,
                target_tabs,
                DropZone::Center,
                Some(insert_index),
            ),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => self
                .commit_resolved_payload_tabs_target_drop(
                    source_space,
                    payload,
                    target_space,
                    target_tabs,
                    DropZone::Center,
                    None,
                ),
            DockResolvedDropTargetKind::InnerEdge {
                target_tabs, zone, ..
            } => self.commit_resolved_payload_tabs_target_drop(
                source_space,
                payload,
                target_space,
                target_tabs,
                zone,
                None,
            ),
            DockResolvedDropTargetKind::RootEdge { root, zone, .. } => self
                .commit_resolved_payload_tabs_target_drop(
                    source_space,
                    payload,
                    target_space,
                    root,
                    zone,
                    None,
                ),
            DockResolvedDropTargetKind::EmptyDockSpace { space } => match payload {
                DockWorkspaceDropPayload::Item { item, .. } => {
                    self.commit_item_to_empty_dock_space(source_space, item, &space)
                }
                DockWorkspaceDropPayload::Tabs { source_tabs } => {
                    self.commit_tabs_to_empty_dock_space(source_space, source_tabs, &space)
                }
            },
            DockResolvedDropTargetKind::KnownViewport { .. }
            | DockResolvedDropTargetKind::TearOffCandidate { .. } => {
                Err(DockActionApplyError::DropTargetUnavailable)
            }
        }
    }

    fn commit_resolved_payload_tabs_target_drop(
        &mut self,
        source_space: &DockSpaceId,
        payload: DockWorkspaceDropPayload<'_>,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
        insert_index: Option<usize>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match payload {
            DockWorkspaceDropPayload::Item { source_tabs, item } => {
                self.commit_tab_move(DockWorkspaceMoveTabRequest {
                    source_space,
                    source_tabs,
                    item,
                    target_space,
                    target_tabs,
                    zone,
                    insert_index,
                })
            }
            DockWorkspaceDropPayload::Tabs { source_tabs } => {
                self.commit_tabs_move(DockWorkspaceMoveTabsRequest {
                    source_space,
                    source_tabs,
                    target_space,
                    target_tabs,
                    zone,
                    insert_index,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockItemId, DockNode, DockPolicyError, DockSpaceId, DockViewportHit, SplitAxis,
        drop_target::{
            DockDropResolveSource, DockKnownViewportDropTarget, DockResolvedDropTargetKind,
        },
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

    fn split_workspace() -> (DockWorkspace, DockNodeId, DockNodeId, DockNodeId) {
        let mut graph = DockGraph::new();
        let left = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            active: 0,
        });
        let right = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            active: 0,
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
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: source_items,
            active: 0,
        });
        let target_left = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            active: 0,
        });
        let target_right = graph.insert_node(DockNode::Tabs {
            items: vec![item("d")],
            active: 0,
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
        DockResolvedDropTarget {
            kind,
            source: DockDropResolveSource::LeafBody,
            preview_bounds: Some(bounds()),
            is_central_region: false,
        }
    }

    #[test]
    fn resolved_center_target_moves_item_without_graph_shaped_action() {
        let (mut workspace, _root, left, right) = split_workspace();

        let outcome = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: left,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::LeafCenter {
                    root: right,
                    target_tabs: right,
                }),
            })
            .expect("resolved center drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        let DockNode::Tabs { items, active } =
            workspace.graph().node(right).expect("target should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a")]);
        assert_eq!(*active, 1);
    }

    #[test]
    fn resolved_tab_bar_target_reorders_same_stack() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b"), item("c")],
            active: 0,
        });
        graph.set_root(space(), tabs);
        let mut workspace = DockWorkspace::new(space(), graph);

        let outcome = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: tabs,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::TabBar {
                    target_tabs: tabs,
                    insert_index: 3,
                }),
            })
            .expect("same-stack reorder should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        let DockNode::Tabs { items, active } =
            workspace.graph().node(tabs).expect("tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(*active, 2);
    }

    #[test]
    fn resolved_edge_target_respects_policy_before_mutation() {
        let (mut workspace, _root, left, right) = split_workspace();
        workspace.policy_mut().set_allow_edge_split(false);

        let err = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: left,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::InnerEdge {
                    root: right,
                    target_tabs: right,
                    zone: DropZone::Right,
                }),
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
    fn resolved_empty_space_target_creates_detached_root_when_policy_allows() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        workspace.policy_mut().set_allow_platform_viewports(true);

        let outcome = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: left,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: detached.clone(),
                }),
            })
            .expect("empty-space target should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        assert_eq!(
            workspace.graph().collect_items_in_space(&detached),
            vec![item("a")]
        );
    }

    #[test]
    fn resolved_center_target_moves_tabs_stack_preserving_order_and_active_item() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            active: 1,
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            active: 0,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::LeafCenter {
                    root: target_tabs,
                    target_tabs,
                }),
            })
            .expect("resolved center stack drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        let DockNode::Tabs { items, active } = workspace
            .graph()
            .node(target_tabs)
            .expect("target tabs should still exist")
        else {
            panic!("target should remain tabs");
        };
        assert_eq!(items, &vec![item("b"), item("a"), item("c")]);
        assert_eq!(*active, 2);
    }

    #[test]
    fn resolved_empty_space_target_moves_tabs_stack_when_policy_allows() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("c")],
            active: 1,
        });
        graph.set_root(space(), source_tabs);
        let mut workspace = DockWorkspace::new(space(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        let detached = DockSpaceId::from("detached");

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: detached.clone(),
                }),
            })
            .expect("resolved empty-space stack drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        let detached_root = workspace
            .graph()
            .root(&detached)
            .expect("detached space should have a root");
        let DockNode::Tabs { items, active } = workspace
            .graph()
            .node(detached_root)
            .expect("detached root should exist")
        else {
            panic!("detached root should be tabs");
        };
        assert_eq!(items, &vec![item("a"), item("c")]);
        assert_eq!(*active, 1);
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

                let outcome = workspace
                    .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                        source_space: &source_space,
                        payload,
                        target_space: &target_space,
                        target: resolved_target(DockResolvedDropTargetKind::RootEdge {
                            root: target_root,
                            leaf_tabs: Some(leaf_tabs),
                            zone,
                        }),
                    })
                    .unwrap_or_else(|error| {
                        panic!("{zone:?} root-edge payload drop should commit: {error}")
                    });

                assert_eq!(outcome, DockActionOutcome::Changed, "{zone:?}");
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

    #[test]
    fn runtime_only_targets_return_drop_target_unavailable_without_mutation() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let secondary = DockSpaceId::from("secondary");

        let viewport_err = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: left,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::KnownViewport {
                    target: DockKnownViewportDropTarget::from_hit(DockViewportHit::new(
                        secondary.clone(),
                        point(px(5.0), px(5.0)),
                    )),
                }),
            })
            .expect_err("known viewport requires local target resolution");

        assert_eq!(viewport_err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }
}
