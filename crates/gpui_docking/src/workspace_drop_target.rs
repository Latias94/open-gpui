use crate::{
    DockActionApplyError, DockGraphDropTarget, DockNode, DockNodeId, DockSpaceId, DockWorkspace,
    drop_target::{
        DockDropResolution, DockDropTargetValidator, DockResolvedDropTarget,
        DockResolvedDropTargetKind, validate_resolved_drop_target,
    },
    geometry::DockDropBoxKind,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockWorkspaceResolvedDropTarget {
    target_space: DockSpaceId,
    target: DockResolvedDropTarget,
}

pub(crate) struct DockWorkspaceResolvedDropTargetParts {
    pub(crate) target_space: DockSpaceId,
    pub(crate) target: DockResolvedDropTarget,
}

impl DockWorkspaceResolvedDropTarget {
    pub(crate) fn new(target_space: DockSpaceId, target: DockResolvedDropTarget) -> Self {
        Self {
            target_space,
            target,
        }
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    #[cfg(test)]
    pub(crate) fn target(&self) -> &DockResolvedDropTarget {
        &self.target
    }

    pub(crate) fn into_parts(self) -> DockWorkspaceResolvedDropTargetParts {
        DockWorkspaceResolvedDropTargetParts {
            target_space: self.target_space,
            target: self.target,
        }
    }
}

pub(crate) struct DockWorkspaceDropCommitTarget {
    target_space: DockSpaceId,
    kind: DockWorkspaceDropCommitTargetKind,
}

impl DockWorkspaceDropCommitTarget {
    pub(crate) fn target_space(&self) -> &DockSpaceId {
        &self.target_space
    }

    pub(crate) fn into_parts(self) -> DockWorkspaceDropCommitTargetParts {
        DockWorkspaceDropCommitTargetParts {
            target_space: self.target_space,
            kind: self.kind,
        }
    }
}

pub(crate) struct DockWorkspaceDropCommitTargetParts {
    pub(crate) target_space: DockSpaceId,
    pub(crate) kind: DockWorkspaceDropCommitTargetKind,
}

pub(crate) enum DockWorkspaceDropCommitTargetKind {
    Graph(DockGraphDropTarget),
    EmptyDockSpace(DockSpaceId),
}

pub(crate) fn resolve_workspace_drop_commit_target(
    workspace: &DockWorkspace,
    target: DockWorkspaceResolvedDropTarget,
    target_validator: Option<&DockDropTargetValidator<'_>>,
) -> Result<DockWorkspaceDropCommitTarget, DockActionApplyError> {
    let drop_target = target.into_parts();
    let target = match validate_resolved_drop_target(
        drop_target.target,
        workspace.policy(),
        target_validator,
    ) {
        DockDropResolution::Valid(target) => target,
        DockDropResolution::Rejected(rejection) => {
            return Err(DockActionApplyError::Policy(rejection.reason));
        }
    };
    validate_resolved_target_drop_box(&target)?;
    validate_resolved_target_graph_identity(workspace, &drop_target.target_space, &target)?;

    let kind = match target.kind {
        DockResolvedDropTargetKind::TabBar {
            target_tabs,
            insert_index,
        } => DockWorkspaceDropCommitTargetKind::Graph(DockGraphDropTarget::tab_bar(
            target_tabs,
            insert_index,
        )),
        DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
            DockWorkspaceDropCommitTargetKind::Graph(DockGraphDropTarget::center(target_tabs))
        }
        DockResolvedDropTargetKind::InnerEdge {
            root: _,
            target_tabs,
            zone,
        } => DockWorkspaceDropCommitTargetKind::Graph(resolve_edge_graph_drop_target(
            workspace,
            &drop_target.target_space,
            target_tabs,
            zone,
            &target,
        )?),
        DockResolvedDropTargetKind::RootEdge { root, zone, .. } => {
            DockWorkspaceDropCommitTargetKind::Graph(resolve_edge_graph_drop_target(
                workspace,
                &drop_target.target_space,
                root,
                zone,
                &target,
            )?)
        }
        DockResolvedDropTargetKind::EmptyDockSpace { space } => {
            if target.is_central_region {
                workspace.policy().validate_central_region_dock_over()?;
            }
            DockWorkspaceDropCommitTargetKind::EmptyDockSpace(space)
        }
    };

    Ok(DockWorkspaceDropCommitTarget {
        target_space: drop_target.target_space,
        kind,
    })
}

fn resolve_edge_graph_drop_target(
    workspace: &DockWorkspace,
    target_space: &DockSpaceId,
    target_node: DockNodeId,
    zone: crate::DropZone,
    target: &DockResolvedDropTarget,
) -> Result<DockGraphDropTarget, DockActionApplyError> {
    let sizing = target
        .edge_sizing
        .ok_or(DockActionApplyError::DropTargetUnavailable)?;
    let plan = target
        .edge_plan
        .ok_or(DockActionApplyError::DropTargetUnavailable)?;
    if plan.drop_zone() != zone {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    if workspace
        .graph()
        .edge_dock_plan_with_sizing(target_space, target_node, zone, sizing)
        != Some(plan)
    {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    if !workspace
        .graph()
        .edge_dock_plan_is_current(target_space, plan)
    {
        return Err(DockActionApplyError::DropTargetUnavailable);
    }
    Ok(DockGraphDropTarget::edge(plan))
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

pub(crate) fn expected_drop_box_kind(kind: &DockResolvedDropTargetKind) -> Option<DockDropBoxKind> {
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
