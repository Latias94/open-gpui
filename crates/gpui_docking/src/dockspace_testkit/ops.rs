use crate::{
    DockGraph, DockGraphDropTarget, DockItemId, DockNodeId, DockOp, DockSpaceId, DropZone,
    dock_bounds,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceScenarioZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum DockspaceScenarioStep {
    MoveItemToItem {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        source_space: DockSpaceId,
        item: DockItemId,
        target_space: DockSpaceId,
        target_item: DockItemId,
        zone: DockspaceScenarioZone,
    },
    MoveItemToEmptySpace {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        source_space: DockSpaceId,
        item: DockItemId,
        target_space: DockSpaceId,
    },
    FloatItemInWindow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        source_space: DockSpaceId,
        item: DockItemId,
        target_space: DockSpaceId,
        bounds: [f32; 4],
    },
    MergeFirstFloating {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        source_space: DockSpaceId,
        target_space: DockSpaceId,
        target_item: DockItemId,
    },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum DockspaceStepResolveError {
    #[error("scenario item {item} was not found in source space {space}")]
    MissingSourceItem {
        space: DockSpaceId,
        item: DockItemId,
    },
    #[error("scenario target item {item} was not found in target space {space}")]
    MissingTargetItem {
        space: DockSpaceId,
        item: DockItemId,
    },
    #[error("scenario target item {item} in space {space} has no edge drop target for {zone:?}")]
    MissingEdgeTarget {
        space: DockSpaceId,
        item: DockItemId,
        zone: DockspaceScenarioZone,
    },
    #[error("scenario space {space} has no floating container")]
    MissingFloating { space: DockSpaceId },
}

impl DockspaceScenarioStep {
    pub(crate) fn move_item_to_item(
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        target_item: impl Into<DockItemId>,
        zone: DockspaceScenarioZone,
    ) -> Self {
        Self::MoveItemToItem {
            id: None,
            source_space: source_space.into(),
            item: item.into(),
            target_space: target_space.into(),
            target_item: target_item.into(),
            zone,
        }
    }

    pub(crate) fn move_item_to_empty_space(
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
    ) -> Self {
        Self::MoveItemToEmptySpace {
            id: None,
            source_space: source_space.into(),
            item: item.into(),
            target_space: target_space.into(),
        }
    }

    pub(crate) fn id(&self) -> Option<&str> {
        match self {
            Self::MoveItemToItem { id, .. }
            | Self::MoveItemToEmptySpace { id, .. }
            | Self::FloatItemInWindow { id, .. }
            | Self::MergeFirstFloating { id, .. } => id.as_deref(),
        }
    }

    pub(crate) fn to_op(&self, graph: &DockGraph) -> Result<DockOp, DockspaceStepResolveError> {
        match self {
            Self::MoveItemToItem {
                id: _,
                source_space,
                item,
                target_space,
                target_item,
                zone,
            } => {
                graph
                    .find_item_in_space(source_space, item)
                    .ok_or_else(|| DockspaceStepResolveError::MissingSourceItem {
                        space: source_space.clone(),
                        item: item.clone(),
                    })?;
                let target_tabs = graph
                    .find_item_in_space(target_space, target_item)
                    .ok_or_else(|| DockspaceStepResolveError::MissingTargetItem {
                        space: target_space.clone(),
                        item: target_item.clone(),
                    })?
                    .0;
                let target = zone.resolve_target(graph, target_space, target_item, target_tabs)?;
                Ok(DockOp::MoveItem {
                    source_space: source_space.clone(),
                    item: item.clone(),
                    target_space: target_space.clone(),
                    target,
                })
            }
            Self::MoveItemToEmptySpace {
                id: _,
                source_space,
                item,
                target_space,
            } => {
                graph
                    .find_item_in_space(source_space, item)
                    .ok_or_else(|| DockspaceStepResolveError::MissingSourceItem {
                        space: source_space.clone(),
                        item: item.clone(),
                    })?;
                Ok(DockOp::MoveItem {
                    source_space: source_space.clone(),
                    item: item.clone(),
                    target_space: target_space.clone(),
                    target: DockGraphDropTarget::empty_space(),
                })
            }
            Self::FloatItemInWindow {
                id: _,
                source_space,
                item,
                target_space,
                bounds,
            } => {
                graph
                    .find_item_in_space(source_space, item)
                    .ok_or_else(|| DockspaceStepResolveError::MissingSourceItem {
                        space: source_space.clone(),
                        item: item.clone(),
                    })?;
                Ok(DockOp::FloatItemInWindow {
                    source_space: source_space.clone(),
                    item: item.clone(),
                    target_space: target_space.clone(),
                    bounds: dock_bounds(bounds[0], bounds[1], bounds[2], bounds[3]),
                })
            }
            Self::MergeFirstFloating {
                id: _,
                source_space,
                target_space,
                target_item,
            } => {
                let floating = graph
                    .floating_containers(source_space)
                    .first()
                    .ok_or_else(|| DockspaceStepResolveError::MissingFloating {
                        space: source_space.clone(),
                    })?
                    .node;
                let target_tabs = graph
                    .find_item_in_space(target_space, target_item)
                    .ok_or_else(|| DockspaceStepResolveError::MissingTargetItem {
                        space: target_space.clone(),
                        item: target_item.clone(),
                    })?
                    .0;
                Ok(DockOp::MoveFloating {
                    source_space: source_space.clone(),
                    floating,
                    target_space: target_space.clone(),
                    target: DockGraphDropTarget::center(target_tabs),
                })
            }
        }
    }
}

impl DockspaceScenarioZone {
    fn resolve_target(
        self,
        graph: &DockGraph,
        target_space: &DockSpaceId,
        target_item: &DockItemId,
        target_tabs: DockNodeId,
    ) -> Result<DockGraphDropTarget, DockspaceStepResolveError> {
        match self {
            Self::Center => Ok(DockGraphDropTarget::center(target_tabs)),
            Self::Left | Self::Right | Self::Top | Self::Bottom => {
                let zone: DropZone = self.into();
                graph
                    .edge_dock_plan(target_space, target_tabs, zone)
                    .map(DockGraphDropTarget::edge)
                    .ok_or_else(|| DockspaceStepResolveError::MissingEdgeTarget {
                        space: target_space.clone(),
                        item: target_item.clone(),
                        zone: self,
                    })
            }
        }
    }
}

impl From<DockspaceScenarioZone> for DropZone {
    fn from(zone: DockspaceScenarioZone) -> Self {
        match zone {
            DockspaceScenarioZone::Center => DropZone::Center,
            DockspaceScenarioZone::Left => DropZone::Left,
            DockspaceScenarioZone::Right => DropZone::Right,
            DockspaceScenarioZone::Top => DropZone::Top,
            DockspaceScenarioZone::Bottom => DropZone::Bottom,
        }
    }
}
