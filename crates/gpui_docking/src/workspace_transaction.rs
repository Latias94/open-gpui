use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockSpaceId,
    DockTransactionError, DockViewportHit, DockWorkspace, DropZone,
    drop_target::{DockResolvedDropTarget, DockResolvedDropTargetKind},
    workspace_move_action::DockWorkspaceMoveTabRequest,
};

pub(crate) struct DockWorkspaceDropRequest<'a> {
    pub(crate) source_space: &'a DockSpaceId,
    pub(crate) source_tabs: DockNodeId,
    pub(crate) item: &'a DockItemId,
    pub(crate) target_space: &'a DockSpaceId,
    pub(crate) target: DockResolvedDropTarget,
}

impl DockWorkspace {
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

        match target.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => self.commit_resolved_tab_drop(
                source_space,
                source_tabs,
                item,
                target_space,
                target_tabs,
                DropZone::Center,
                Some(insert_index),
            ),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => self
                .commit_resolved_tab_drop(
                    source_space,
                    source_tabs,
                    item,
                    target_space,
                    target_tabs,
                    DropZone::Center,
                    None,
                ),
            DockResolvedDropTargetKind::InnerEdge {
                target_tabs, zone, ..
            } => self.commit_resolved_tab_drop(
                source_space,
                source_tabs,
                item,
                target_space,
                target_tabs,
                zone,
                None,
            ),
            DockResolvedDropTargetKind::RootEdge { root, zone, .. } => self
                .commit_resolved_tab_drop(
                    source_space,
                    source_tabs,
                    item,
                    target_space,
                    root,
                    zone,
                    None,
                ),
            DockResolvedDropTargetKind::EmptyDockSpace { space } => {
                self.move_item_to_empty_dock_space_action(source_space, item, &space)
            }
            DockResolvedDropTargetKind::KnownViewport { hit } => {
                Err(viewport_target_error(hit).into())
            }
            DockResolvedDropTargetKind::TearOffCandidate { .. } => {
                Err(DockTransactionError::TearOffRequiresViewportRuntime.into())
            }
        }
    }

    fn commit_resolved_tab_drop(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
        insert_index: Option<usize>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.move_tab_action(DockWorkspaceMoveTabRequest {
            source_space,
            source_tabs,
            item,
            target_space,
            target_tabs,
            zone,
            insert_index,
        })
    }
}

fn viewport_target_error(hit: DockViewportHit) -> DockTransactionError {
    DockTransactionError::ViewportTargetRequiresLocalResolution { space: hit.space }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockItemId, DockNode, DockPolicyError, DockSpaceId, DockTransactionError,
        DockViewportHit, SplitAxis,
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

    fn resolved_target(kind: DockResolvedDropTargetKind) -> DockResolvedDropTarget {
        DockResolvedDropTarget {
            kind,
            source: DockDropResolveSource::LeafBody,
            preview_bounds: Some(bounds()),
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
    fn runtime_only_targets_return_transaction_errors_without_mutation() {
        let (mut workspace, _root, left, _right) = split_workspace();
        let secondary = DockSpaceId::from("secondary");

        let viewport_err = workspace
            .commit_resolved_drop(DockWorkspaceDropRequest {
                source_space: &space(),
                source_tabs: left,
                item: &item("a"),
                target_space: &space(),
                target: resolved_target(DockResolvedDropTargetKind::KnownViewport {
                    hit: DockViewportHit {
                        space: secondary.clone(),
                        host_position: point(px(5.0), px(5.0)),
                    },
                }),
            })
            .expect_err("known viewport requires local target resolution");

        assert_eq!(
            viewport_err,
            DockActionApplyError::Transaction(
                DockTransactionError::ViewportTargetRequiresLocalResolution { space: secondary }
            )
        );
        assert_eq!(
            workspace.graph().collect_items_in_space(&space()),
            vec![item("a"), item("b")]
        );
    }
}
