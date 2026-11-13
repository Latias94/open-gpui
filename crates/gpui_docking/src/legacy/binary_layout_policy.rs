#![allow(dead_code)]

use crate::legacy::CanonicalNodeId;
use crate::legacy::graph_assembly::CanonicalSizeRef;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryChild {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinarySplitClass {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedAxisSizes {
    pub(crate) available: f32,
    pub(crate) first: f32,
    pub(crate) second: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StructuralIdentityRemap {
    pub(crate) from: CanonicalNodeId,
    pub(crate) to: CanonicalNodeId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SiblingPromotionPlan {
    pub(crate) parent: CanonicalNodeId,
    pub(crate) survivor: CanonicalNodeId,
    pub(crate) retired: [CanonicalNodeId; 2],
    pub(crate) remap: StructuralIdentityRemap,
    pub(crate) preserved_size_ref: Option<CanonicalSizeRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub(crate) enum BinaryLayoutPolicyError {
    #[error("binary layout dimensions must be finite and non-negative")]
    Dimension,
    #[error("binary split ratio must be finite and between zero and one inclusive")]
    Ratio,
    #[error("sibling promotion survivor is not a child of the promoted parent")]
    PromotionSurvivor,
    #[error("sibling promotion requires distinct parent and child identities")]
    PromotionIdentity,
}

/// Applies Dear ImGui's split-creation rule: truncate the first size and assign the remainder.
pub(crate) fn initial_split_sizes(
    parent_size: f32,
    separator_size: f32,
    minimum_size: f32,
    ratio: f32,
) -> Result<ResolvedAxisSizes, BinaryLayoutPolicyError> {
    validate_inputs(parent_size, separator_size, minimum_size, ratio)?;
    let minimum_pair = minimum_pair(minimum_size)?;
    let available = (parent_size - separator_size).max(minimum_pair);
    let first = (available * ratio).trunc();
    let second = (available - first).trunc();
    Ok(ResolvedAxisSizes {
        available,
        first,
        second,
    })
}

/// Resolves one binary branch using Dear ImGui's minimum, rounding, and central-remainder rules.
pub(crate) fn resolve_split_sizes(
    parent_size: f32,
    separator_size: f32,
    minimum_size: f32,
    ratio: f32,
    first_size_ref: Option<f32>,
    second_size_ref: Option<f32>,
    central_descendant: Option<BinaryChild>,
) -> Result<ResolvedAxisSizes, BinaryLayoutPolicyError> {
    validate_inputs(parent_size, separator_size, minimum_size, ratio)?;
    validate_size_ref(first_size_ref)?;
    validate_size_ref(second_size_ref)?;

    let available = (parent_size - separator_size).max(0.0);
    let minimum_each = (available.min(minimum_pair(minimum_size)?) * 0.5).trunc();
    let first = match (central_descendant, first_size_ref, second_size_ref) {
        (Some(BinaryChild::Second), Some(size_ref), _) if size_ref != 0.0 => {
            size_ref.min((available - minimum_each).max(0.0))
        }
        (Some(BinaryChild::First), _, Some(size_ref)) if size_ref != 0.0 => {
            available - size_ref.min((available - minimum_each).max(0.0))
        }
        _ => {
            let effective_ratio = match (first_size_ref, second_size_ref) {
                (Some(first_ref), Some(second_ref)) => {
                    let total = first_ref + second_ref;
                    if total.is_finite() && total > 0.0 {
                        first_ref / total
                    } else {
                        ratio
                    }
                }
                _ => ratio,
            };
            minimum_each.max((available * effective_ratio + 0.5).trunc())
        }
    };
    let second = available - first;
    Ok(ResolvedAxisSizes {
        available,
        first,
        second,
    })
}

/// Describes ImGui-style sibling promotion without mutating the graph.
pub(crate) fn plan_sibling_promotion(
    parent: CanonicalNodeId,
    first: CanonicalNodeId,
    second: CanonicalNodeId,
    survivor: CanonicalNodeId,
    parent_size_ref: Option<CanonicalSizeRef>,
) -> Result<SiblingPromotionPlan, BinaryLayoutPolicyError> {
    if parent == first || parent == second || first == second {
        return Err(BinaryLayoutPolicyError::PromotionIdentity);
    }
    if survivor != first && survivor != second {
        return Err(BinaryLayoutPolicyError::PromotionSurvivor);
    }
    Ok(SiblingPromotionPlan {
        parent,
        survivor,
        retired: [first, second],
        remap: StructuralIdentityRemap {
            from: survivor,
            to: parent,
        },
        preserved_size_ref: parent_size_ref,
    })
}

/// Inner splits transfer central identity to the node inheriting the target's contents.
pub(crate) fn transfer_central_after_split(
    current_central: Option<CanonicalNodeId>,
    target: CanonicalNodeId,
    inheritor: CanonicalNodeId,
    split_class: BinarySplitClass,
) -> Option<CanonicalNodeId> {
    match (split_class, current_central) {
        (BinarySplitClass::Inner, Some(central)) if central == target => Some(inheritor),
        _ => current_central,
    }
}

fn validate_inputs(
    parent_size: f32,
    separator_size: f32,
    minimum_size: f32,
    ratio: f32,
) -> Result<(), BinaryLayoutPolicyError> {
    if !parent_size.is_finite()
        || !separator_size.is_finite()
        || !minimum_size.is_finite()
        || parent_size < 0.0
        || separator_size < 0.0
        || minimum_size < 0.0
    {
        return Err(BinaryLayoutPolicyError::Dimension);
    }
    if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
        return Err(BinaryLayoutPolicyError::Ratio);
    }
    Ok(())
}

fn validate_size_ref(size_ref: Option<f32>) -> Result<(), BinaryLayoutPolicyError> {
    if size_ref.is_some_and(|value| !value.is_finite() || value < 0.0) {
        Err(BinaryLayoutPolicyError::Dimension)
    } else {
        Ok(())
    }
}

fn minimum_pair(minimum_size: f32) -> Result<f32, BinaryLayoutPolicyError> {
    if minimum_size > f32::MAX * 0.5 {
        return Err(BinaryLayoutPolicyError::Dimension);
    }
    Ok(minimum_size * 2.0)
}
