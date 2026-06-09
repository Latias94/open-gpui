use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockOp, DockSpaceId,
    DockWorkspace,
};
use open_gpui::{Bounds, Pixels};

impl DockWorkspace {
    pub(crate) fn float_item_in_window_action(
        &mut self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::FloatItemInWindow {
            source_space: source_space.clone(),
            item: item.clone(),
            target_space: target_space.clone(),
            bounds,
        })
    }

    pub(crate) fn float_tabs_in_window_action(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::FloatTabsInWindow {
            source_space: source_space.clone(),
            source_tabs,
            target_space: target_space.clone(),
            bounds,
        })
    }

    pub(crate) fn set_floating_bounds_action(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::SetFloatingBounds {
            space: space.clone(),
            floating,
            bounds,
        })
    }

    pub(crate) fn raise_floating_action(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::RaiseFloating {
            space: space.clone(),
            floating,
        })
    }

    pub(crate) fn merge_floating_into_action(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_floating()?;
        self.commit_graph_op(DockOp::MergeFloatingInto {
            space: space.clone(),
            floating,
            target_tabs,
        })
    }
}
