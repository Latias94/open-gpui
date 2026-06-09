#[cfg(test)]
use crate::SplitFractionsUpdate;
use crate::{DockNodeId, DockOpApplyError};
#[cfg(test)]
use std::collections::HashSet;

use super::{DockGraph, DockNode};

impl DockGraph {
    pub(in crate::graph) fn validate_split_fractions(
        &self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<(), DockOpApplyError> {
        let Some(node) = self.node(split) else {
            return Err(DockOpApplyError::SplitNodeNotFound { split });
        };
        let DockNode::Split { children, .. } = node else {
            return Err(DockOpApplyError::NodeIsNotSplit { node: split });
        };
        if children.len() < 2 {
            return Err(DockOpApplyError::SplitTooFewChildren {
                split,
                children_len: children.len(),
            });
        }
        if fractions.len() != children.len() {
            return Err(DockOpApplyError::SplitFractionsLenMismatch {
                split,
                children_len: children.len(),
                fractions_len: fractions.len(),
            });
        }
        for (index, fraction) in fractions.iter().copied().enumerate() {
            if !fraction.is_finite() || fraction < 0.0 {
                return Err(DockOpApplyError::SplitFractionInvalid { split, index });
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::graph) fn validate_split_fraction_updates(
        &self,
        updates: &[SplitFractionsUpdate],
    ) -> Result<(), DockOpApplyError> {
        let mut seen = HashSet::new();
        for update in updates {
            if !seen.insert(update.split) {
                return Err(DockOpApplyError::DuplicateSplitFractionUpdate {
                    split: update.split,
                });
            }
            self.validate_split_fractions(update.split, &update.fractions)?;
        }
        Ok(())
    }
}
