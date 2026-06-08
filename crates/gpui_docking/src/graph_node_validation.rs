use crate::{DockItemId, DockNodeId, DockOpApplyError, DockSpaceId};

use super::{DockGraph, DockNode};

impl DockGraph {
    pub(in crate::graph) fn require_tabs_node(
        &self,
        tabs: DockNodeId,
    ) -> Result<&[DockItemId], DockOpApplyError> {
        match self.node(tabs) {
            Some(DockNode::Tabs { items, .. }) => Ok(items),
            Some(_) => Err(DockOpApplyError::NodeIsNotTabs { node: tabs }),
            None => Err(DockOpApplyError::TabsNodeNotFound { tabs }),
        }
    }

    pub(in crate::graph) fn require_non_empty_tabs_node(
        &self,
        tabs: DockNodeId,
    ) -> Result<(), DockOpApplyError> {
        let items = self.require_tabs_node(tabs)?;
        if items.is_empty() {
            return Err(DockOpApplyError::TabsNodeEmpty { tabs });
        }
        Ok(())
    }

    pub(in crate::graph) fn require_source_node_in_space(
        &self,
        space: &DockSpaceId,
        node: DockNodeId,
    ) -> Result<DockNodeId, DockOpApplyError> {
        self.root_for_node_in_space(space, node).ok_or_else(|| {
            DockOpApplyError::SourceNodeNotInSpace {
                space: space.clone(),
                node,
            }
        })
    }

    pub(in crate::graph) fn require_target_node_in_space(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> Result<DockNodeId, DockOpApplyError> {
        self.root_for_node_in_space(space, target).ok_or_else(|| {
            DockOpApplyError::TargetNodeNotInSpace {
                space: space.clone(),
                target,
            }
        })
    }
}
