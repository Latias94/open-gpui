use crate::{
    DockActionApplyError, DockGraphMutationError, DockItemId, DockNode, DockNodeId, DockSpaceId,
    DockWorkspace,
};

pub(crate) struct DockWorkspaceMoveValidation<'a> {
    workspace: &'a DockWorkspace,
}

impl<'a> DockWorkspaceMoveValidation<'a> {
    pub(crate) fn new(workspace: &'a DockWorkspace) -> Self {
        Self { workspace }
    }

    pub(crate) fn validate_move_tab_source(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        item: &DockItemId,
    ) -> Result<(), DockActionApplyError> {
        let Some(node) = self.workspace.graph().node(source_tabs) else {
            return Err(DockGraphMutationError::TabsNodeNotFound { tabs: source_tabs }.into());
        };
        let DockNode::Tabs { items, .. } = node else {
            return Err(DockGraphMutationError::NodeIsNotTabs { node: source_tabs }.into());
        };
        if self
            .workspace
            .graph()
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return Err(DockGraphMutationError::SourceNodeNotInSpace {
                space: source_space.clone(),
                node: source_tabs,
            }
            .into());
        }
        if !items.iter().any(|candidate| candidate == item) {
            return Err(DockActionApplyError::ItemNotInTabs {
                tabs: source_tabs,
                item: item.clone(),
            });
        }

        Ok(())
    }
}

impl DockWorkspace {
    pub(crate) fn move_validation(&self) -> DockWorkspaceMoveValidation<'_> {
        DockWorkspaceMoveValidation::new(self)
    }
}
