use crate::{
    DockActionApplyError, DockClassId, DockGraphMutationError, DockItemId, DockNode, DockNodeId,
    DockPolicy, DockPolicyError, DockSpaceId, DockViewportDropPayload, DockWorkspace,
    drag::{DockDragPayload, DockDragPayloadKind},
    drop_target::DockResolvedDropTarget,
};

pub(crate) struct DockWorkspaceMoveValidation<'a> {
    workspace: &'a DockWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockPayloadDockClasses {
    items: Vec<DockPayloadDockClassItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockPayloadDockClassItem {
    item: DockItemId,
    dock_class: Option<DockClassId>,
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

    pub(crate) fn validate_item_target_space(
        &self,
        target_space: &DockSpaceId,
        item: &DockItemId,
    ) -> Result<(), DockActionApplyError> {
        self.workspace
            .payload_dock_classes_for_item(item)
            .validate_target_space(target_space, self.workspace.policy())
            .map_err(Into::into)
    }

    pub(crate) fn validate_tabs_target_space(
        &self,
        target_space: &DockSpaceId,
        tabs: DockNodeId,
    ) -> Result<(), DockActionApplyError> {
        self.workspace
            .payload_dock_classes_for_tabs(tabs)
            .validate_target_space(target_space, self.workspace.policy())
            .map_err(Into::into)
    }

    pub(crate) fn validate_floating_target_space(
        &self,
        target_space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Result<(), DockActionApplyError> {
        self.workspace
            .payload_dock_classes_for_floating(floating)
            .validate_target_space(target_space, self.workspace.policy())
            .map_err(Into::into)
    }
}

impl DockWorkspace {
    pub(crate) fn move_validation(&self) -> DockWorkspaceMoveValidation<'_> {
        DockWorkspaceMoveValidation::new(self)
    }

    pub(crate) fn payload_dock_classes_for_item(
        &self,
        item: &DockItemId,
    ) -> DockPayloadDockClasses {
        DockPayloadDockClasses::from_items([payload_dock_class_item(self, item)])
    }

    pub(crate) fn payload_dock_classes_for_tabs(&self, tabs: DockNodeId) -> DockPayloadDockClasses {
        let items = match self.graph().node(tabs) {
            Some(DockNode::Tabs { items, .. }) => items.as_slice(),
            Some(_) | None => &[],
        };
        self.payload_dock_classes_for_items(items)
    }

    pub(crate) fn payload_dock_classes_for_floating(
        &self,
        floating: DockNodeId,
    ) -> DockPayloadDockClasses {
        let items = self.graph().collect_items_in_subtree(floating);
        self.payload_dock_classes_for_items(&items)
    }

    pub(crate) fn payload_dock_classes_for_drag_payload(
        &self,
        payload: &DockDragPayload,
    ) -> DockPayloadDockClasses {
        match &payload.kind {
            DockDragPayloadKind::Item { item } => self.payload_dock_classes_for_item(item),
            DockDragPayloadKind::Tabs => self.payload_dock_classes_for_tabs(payload.source_node),
            DockDragPayloadKind::Floating { floating } => {
                self.payload_dock_classes_for_floating(*floating)
            }
        }
    }

    pub(crate) fn payload_dock_classes_for_viewport_payload(
        &self,
        payload: &DockViewportDropPayload,
        source_tabs: DockNodeId,
    ) -> DockPayloadDockClasses {
        match payload {
            DockViewportDropPayload::Item(item) => self.payload_dock_classes_for_item(item),
            DockViewportDropPayload::Tabs => self.payload_dock_classes_for_tabs(source_tabs),
            DockViewportDropPayload::Floating(floating) => {
                self.payload_dock_classes_for_floating(*floating)
            }
        }
    }

    fn payload_dock_classes_for_items<'a>(
        &self,
        items: impl IntoIterator<Item = &'a DockItemId>,
    ) -> DockPayloadDockClasses {
        DockPayloadDockClasses::from_items(
            items
                .into_iter()
                .map(|item| payload_dock_class_item(self, item)),
        )
    }
}

impl DockPayloadDockClasses {
    fn from_items(items: impl IntoIterator<Item = DockPayloadDockClassItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    pub(crate) fn validate_target_space(
        &self,
        target_space: &DockSpaceId,
        policy: &DockPolicy,
    ) -> Result<(), DockPolicyError> {
        for item in &self.items {
            policy.validate_dock_class_for_item(
                target_space,
                &item.item,
                item.dock_class.as_ref(),
            )?;
        }
        Ok(())
    }
}

pub(crate) fn dock_target_validator<'a>(
    default_space: &'a DockSpaceId,
    payload_classes: &'a DockPayloadDockClasses,
    policy: &'a DockPolicy,
) -> impl Fn(&DockResolvedDropTarget) -> Result<(), DockPolicyError> + 'a {
    move |target| payload_classes.validate_target_space(target.target_space(default_space), policy)
}

fn payload_dock_class_item(
    workspace: &DockWorkspace,
    item: &DockItemId,
) -> DockPayloadDockClassItem {
    DockPayloadDockClassItem {
        item: item.clone(),
        dock_class: workspace
            .panels()
            .descriptor(item)
            .and_then(|descriptor| descriptor.dock_class().cloned()),
    }
}
