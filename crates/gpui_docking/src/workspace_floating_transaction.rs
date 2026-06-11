use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockOp, DockSpaceId,
    DockWorkspace,
};
use open_gpui::{Bounds, Pixels};

impl DockWorkspace {
    pub(crate) fn commit_float_item_in_window(
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

    pub(crate) fn commit_float_tabs_in_window(
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

    pub(crate) fn commit_set_floating_bounds(
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

    pub(crate) fn commit_raise_floating(
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

    pub(crate) fn commit_merge_floating_into(
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

    pub(crate) fn commit_floating_move(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target: DockNodeId,
        zone: crate::DropZone,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_drop_zone(zone)?;
        self.commit_graph_op(DockOp::MoveFloating {
            source_space: source_space.clone(),
            floating,
            target_space: target_space.clone(),
            target,
            zone,
        })
    }

    pub(crate) fn commit_floating_to_empty_dock_space(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_platform_viewports()?;
        self.commit_graph_op(DockOp::MoveFloatingToEmptyDockSpace {
            source_space: source_space.clone(),
            floating,
            target_space: target_space.clone(),
        })
    }
}
