use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockMoveTarget, DockNode, DockNodeId,
    DockSpaceId, DockWorkspace,
    drop_target::{
        DockDropResolution, DockResolvedDropTarget, DockResolvedDropTargetKind,
        validate_resolved_drop_target,
    },
    geometry::DockDropBoxKind,
    workspace_move_validation::dock_target_validator,
};

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
    Floating {
        floating: DockNodeId,
    },
}

impl DockWorkspace {
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

        let target = {
            let payload_classes = self.payload_dock_classes_for_workspace_payload(&payload);
            let target_validator =
                dock_target_validator(target_space, &payload_classes, self.policy());
            match validate_resolved_drop_target(target, self.policy(), Some(&target_validator)) {
                DockDropResolution::Valid(target) => target,
                DockDropResolution::Rejected(rejection) => {
                    return Err(DockActionApplyError::Policy(rejection.reason));
                }
            }
        };
        validate_resolved_target_drop_box(&target)?;
        validate_resolved_target_graph_identity(self, target_space, &target)?;

        match target.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => self.commit_resolved_payload_graph_target_drop(
                source_space,
                payload,
                target_space,
                DockMoveTarget::tab_bar(target_tabs, insert_index),
            ),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => self
                .commit_resolved_payload_graph_target_drop(
                    source_space,
                    payload,
                    target_space,
                    DockMoveTarget::center(target_tabs),
                ),
            DockResolvedDropTargetKind::InnerEdge {
                root,
                target_tabs,
                zone,
            } => self.commit_resolved_payload_graph_target_drop(
                source_space,
                payload,
                target_space,
                DockMoveTarget::inner_edge(root, target_tabs, zone),
            ),
            DockResolvedDropTargetKind::RootEdge { root, zone, .. } => self
                .commit_resolved_payload_graph_target_drop(
                    source_space,
                    payload,
                    target_space,
                    DockMoveTarget::root_edge(root, zone),
                ),
            DockResolvedDropTargetKind::EmptyDockSpace { space, is_central } => {
                if is_central {
                    self.policy().validate_central_region_dock_over()?;
                }
                match payload {
                    DockWorkspaceDropPayload::Item { item, .. } => {
                        self.commit_item_to_empty_dock_space(source_space, item, &space)
                    }
                    DockWorkspaceDropPayload::Tabs { source_tabs } => {
                        self.commit_tabs_to_empty_dock_space(source_space, source_tabs, &space)
                    }
                    DockWorkspaceDropPayload::Floating { floating } => {
                        self.commit_floating_to_empty_dock_space(source_space, floating, &space)
                    }
                }
            }
        }
    }

    fn commit_resolved_payload_graph_target_drop(
        &mut self,
        source_space: &DockSpaceId,
        payload: DockWorkspaceDropPayload<'_>,
        target_space: &DockSpaceId,
        target: DockMoveTarget,
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

fn validate_resolved_target_graph_identity(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    target: &DockResolvedDropTarget,
) -> Result<(), DockActionApplyError> {
    match target.kind {
        DockResolvedDropTargetKind::TabBar { .. } => Ok(()),
        DockResolvedDropTargetKind::EmptyDockSpace { ref space, .. } => {
            if space == target_space {
                Ok(())
            } else {
                Err(DockActionApplyError::DropTargetUnavailable)
            }
        }
        DockResolvedDropTargetKind::LeafCenter { root, target_tabs }
        | DockResolvedDropTargetKind::InnerEdge {
            root, target_tabs, ..
        } => validate_tabs_under_root(workspace, target_space, root, target_tabs),
        DockResolvedDropTargetKind::RootEdge {
            root, leaf_tabs, ..
        } => {
            validate_root_in_space(workspace, target_space, root)?;
            if let Some(leaf_tabs) = leaf_tabs {
                validate_tabs_under_root(workspace, target_space, root, leaf_tabs)?;
            }
            Ok(())
        }
        DockResolvedDropTargetKind::FloatingTitleBar {
            floating,
            target_tabs,
        } => {
            if !matches!(
                workspace.graph().node(floating),
                Some(DockNode::Floating { .. })
            ) {
                return Err(DockActionApplyError::DropTargetUnavailable);
            }
            validate_root_in_space(workspace, target_space, floating)?;
            validate_tabs_under_root(workspace, target_space, floating, target_tabs)
        }
    }
}

fn validate_root_in_space(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    root: DockNodeId,
) -> Result<(), DockActionApplyError> {
    if workspace.graph().root_for_node_in_space(target_space, root) == Some(root) {
        Ok(())
    } else {
        Err(DockActionApplyError::DropTargetUnavailable)
    }
}

fn validate_tabs_under_root(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    root: DockNodeId,
    target_tabs: DockNodeId,
) -> Result<(), DockActionApplyError> {
    if workspace
        .graph()
        .root_for_node_in_space(target_space, target_tabs)
        == Some(root)
    {
        Ok(())
    } else {
        Err(DockActionApplyError::DropTargetUnavailable)
    }
}

fn validate_resolved_target_drop_box(
    target: &DockResolvedDropTarget,
) -> Result<(), DockActionApplyError> {
    let Some(expected) = expected_drop_box_kind(&target.kind) else {
        return Ok(());
    };
    let Some(drop_box) = target.drop_box else {
        return Err(DockActionApplyError::DropTargetUnavailable);
    };
    if drop_box.kind != expected || target.preview_bounds != Some(drop_box.preview_bounds) {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    Ok(())
}

fn expected_drop_box_kind(kind: &DockResolvedDropTargetKind) -> Option<DockDropBoxKind> {
    match *kind {
        DockResolvedDropTargetKind::LeafCenter { .. } => Some(DockDropBoxKind::Center),
        DockResolvedDropTargetKind::InnerEdge { zone, .. } => {
            Some(DockDropBoxKind::InnerEdge(zone))
        }
        DockResolvedDropTargetKind::RootEdge { zone, .. } => Some(DockDropBoxKind::OuterEdge(zone)),
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockFloatingContainer, DockGraph, DockItemId, DockNode, DockPolicyError, DockSpaceId,
        DropZone, SplitAxis,
        drop_target::{DockDropResolveSource, DockResolvedDropTargetKind},
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
        let drop_box =
            super::expected_drop_box_kind(&kind).map(|kind| crate::geometry::DockDropBox {
                kind,
                hit_bounds: bounds(),
                preview_bounds: bounds(),
            });
        DockResolvedDropTarget {
            kind,
            source: DockDropResolveSource::LeafBody,
            drop_box,
            preview_bounds: Some(bounds()),
            is_central_region: false,
        }
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
                is_central: false,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::LeafCenter {
                    root,
                    target_tabs: right,
                }),
            })
            .expect("resolved center drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: tabs,
                    item: &item("a"),
                },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::TabBar {
                    target_tabs: tabs,
                    insert_index: 3,
                }),
            })
            .expect("same-stack reorder should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
        let DockNode::Tabs { items, selected } =
            workspace.graph().node(tabs).expect("tabs should exist")
        else {
            panic!("target should be tabs");
        };
        assert_eq!(items, &vec![item("b"), item("c"), item("a")]);
        assert_eq!(selected.as_ref(), items.get(2));
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: tabs,
                    item: &item("a"),
                },
                target_space: &space(),
                target,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target,
            })
            .expect_err("edge target with mismatched preview bounds should not commit");

        assert_eq!(err, DockActionApplyError::DropTargetUnavailable);
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }

    #[test]
    fn resolved_leaf_target_requires_target_tabs_under_declared_root() {
        let (mut workspace, root, left, right) = split_workspace();

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::LeafCenter {
                    root: right,
                    target_tabs: right,
                }),
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
            preview_bounds: bounds(),
        });

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item_a,
                },
                target_space: &target_space,
                target,
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &detached,
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: detached.clone(),
                    is_central: false,
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
    fn resolved_empty_space_target_requires_route_space_to_match_target_space() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let detached = DockSpaceId::from("detached");
        workspace.policy_mut().set_allow_platform_viewports(true);

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: detached.clone(),
                    is_central: false,
                }),
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

        let err = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs: left,
                    item: &item("a"),
                },
                target_space: &central,
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: central.clone(),
                    is_central: true,
                }),
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::LeafCenter {
                    root,
                    target_tabs,
                }),
            })
            .expect("resolved center stack drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target_space: &detached,
                target: resolved_target(DockResolvedDropTargetKind::EmptyDockSpace {
                    space: detached.clone(),
                    is_central: false,
                }),
            })
            .expect("resolved empty-space stack drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
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

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Tabs { source_tabs },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::RootEdge {
                    root,
                    leaf_tabs: Some(target_tabs),
                    zone: DropZone::Right,
                }),
            })
            .expect("same-space root-edge tabs move should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
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

        let outcome = workspace
            .commit_resolved_payload_drop(DockWorkspacePayloadDropRequest {
                source_space: &source_space,
                payload: DockWorkspaceDropPayload::Floating { floating },
                target_space: &target_space,
                target: resolved_target(DockResolvedDropTargetKind::RootEdge {
                    root: target_root,
                    leaf_tabs: None,
                    zone: DropZone::Right,
                }),
            })
            .expect("floating root-edge drop should commit");

        assert_eq!(outcome, DockActionOutcome::Changed);
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
                source_space: &space(),
                payload: DockWorkspaceDropPayload::Item {
                    source_tabs,
                    item: &item("a"),
                },
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::FloatingTitleBar {
                    floating,
                    target_tabs: unrelated_tabs,
                }),
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
