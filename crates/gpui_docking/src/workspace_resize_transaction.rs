use crate::{
    DockActionApplyError, DockActionOutcome, DockNodeId, DockOp, DockSplitResize, DockWorkspace,
};

impl DockWorkspace {
    pub(crate) fn commit_resize_split(
        &mut self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_splitter_resize()?;
        self.commit_graph_op(DockOp::SetSplitFractions {
            split,
            fractions: fractions.to_vec(),
        })
    }

    pub(crate) fn commit_resize_splits(
        &mut self,
        updates: &[DockSplitResize],
    ) -> Result<DockActionOutcome, DockActionApplyError> {
        self.policy().validate_splitter_resize()?;
        self.commit_graph_op(DockOp::SetSplitFractionsMany {
            updates: updates.to_vec(),
        })
    }
}
