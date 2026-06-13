use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraphMutationError, DockItemId,
    DockNode, DockNodeId, DockOp, DockSpaceId, DockWorkspace,
};
use open_gpui::{Bounds, Pixels};

impl DockWorkspace {
    /// Selects a tab within one tabs node.
    pub fn select_tab(
        &mut self,
        tabs: DockNodeId,
        item: impl Into<DockItemId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let item = item.into();
        self.commit_select_tab(tabs, &item)
    }

    /// Closes one registered dock item through panel lifecycle policy.
    pub fn close_item(
        &mut self,
        space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = space.into();
        let item = item.into();
        self.commit_close_item(&space, &item)
    }

    /// Opens one registered dock item into an existing tabs node or empty dock space.
    pub fn open_item(
        &mut self,
        space: impl Into<DockSpaceId>,
        target_tabs: Option<DockNodeId>,
        item: impl Into<DockItemId>,
        insert_index: Option<usize>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = space.into();
        let item = item.into();
        self.commit_open_item(&space, target_tabs, &item, insert_index)
    }

    /// Floats one item inside a dock space without creating a platform window.
    pub fn float_item_in_window(
        &mut self,
        source_space: impl Into<DockSpaceId>,
        item: impl Into<DockItemId>,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let source_space = source_space.into();
        let item = item.into();
        let target_space = target_space.into();
        self.commit_float_item_in_window(&source_space, &item, &target_space, bounds)
    }

    /// Floats an entire tabs node inside a dock space without creating a platform window.
    pub fn float_tabs_in_window(
        &mut self,
        source_space: impl Into<DockSpaceId>,
        source_tabs: DockNodeId,
        target_space: impl Into<DockSpaceId>,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let source_space = source_space.into();
        let target_space = target_space.into();
        self.commit_float_tabs_in_window(&source_space, source_tabs, &target_space, bounds)
    }

    /// Updates the bounds of an in-window floating container.
    pub fn set_floating_bounds(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = space.into();
        self.commit_set_floating_bounds(&space, floating, bounds)
    }

    /// Raises an in-window floating container above other floating containers.
    pub fn raise_floating(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = space.into();
        self.commit_raise_floating(&space, floating)
    }

    /// Merges an in-window floating container into an existing tabs node.
    pub fn merge_floating_into(
        &mut self,
        space: impl Into<DockSpaceId>,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let space = space.into();
        self.commit_merge_floating_into(&space, floating, target_tabs)
    }

    /// Resizes one split node by replacing its normalized fractions.
    pub fn resize_split(
        &mut self,
        split: DockNodeId,
        fractions: impl AsRef<[f32]>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.commit_resize_split(split, fractions.as_ref())
    }

    /// Applies a docking action command object.
    pub fn apply_action(
        &mut self,
        action: &DockAction,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        match action {
            DockAction::SelectTab { tabs, item } => self.commit_select_tab(*tabs, item),
            DockAction::CloseItem { space, item } => self.commit_close_item(space, item),
            DockAction::OpenItem {
                space,
                target_tabs,
                item,
                insert_index,
            } => self.commit_open_item(space, *target_tabs, item, *insert_index),
            DockAction::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.commit_float_item_in_window(source_space, item, target_space, *bounds),
            DockAction::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => {
                self.commit_float_tabs_in_window(source_space, *source_tabs, target_space, *bounds)
            }
            DockAction::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.commit_set_floating_bounds(space, *floating, *bounds),
            DockAction::RaiseFloating { space, floating } => {
                self.commit_raise_floating(space, *floating)
            }
            DockAction::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.commit_merge_floating_into(space, *floating, *target_tabs),
            DockAction::ResizeSplit { split, fractions } => {
                self.commit_resize_split(*split, fractions)
            }
        }
    }

    pub(crate) fn commit_graph_op(
        &mut self,
        op: DockOp,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.apply_op_checked(&op)
            .map(DockActionOutcome::from_changed)
            .map_err(Into::into)
    }

    pub(crate) fn commit_select_tab(
        &mut self,
        tabs: DockNodeId,
        item: &DockItemId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        let Some(node) = self.graph().node(tabs) else {
            return Err(DockGraphMutationError::TabsNodeNotFound { tabs }.into());
        };
        let DockNode::Tabs { items, selected } = node else {
            return Err(DockGraphMutationError::NodeIsNotTabs { node: tabs }.into());
        };
        if !items.contains(item) {
            return Err(DockActionApplyError::ItemNotInTabs {
                tabs,
                item: item.clone(),
            });
        }
        if selected.as_ref() == Some(item) {
            return Ok(DockActionOutcome::Unchanged);
        }

        self.commit_graph_op(DockOp::SelectTab {
            tabs,
            item: item.clone(),
        })
    }
}
