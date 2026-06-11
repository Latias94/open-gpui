use crate::{DOCK_VIEWPORT_PLACEMENT_VERSION, DockSpaceId, DockViewportPlacementLayout};
use std::collections::BTreeSet;
use thiserror::Error;

/// Validation error for serialized viewport placement data.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockViewportPlacementValidationError {
    /// The placement version is unsupported.
    #[error("unsupported dock viewport placement version: expected {expected}, found {found}")]
    UnsupportedVersion {
        /// Expected version.
        expected: u32,
        /// Found version.
        found: u32,
    },
    /// A dock space appears more than once in placement data.
    #[error("duplicate dock viewport placement space: {space}")]
    DuplicateSpace {
        /// Duplicate dock space id.
        space: DockSpaceId,
    },
    /// A viewport placement has non-finite platform window coordinates or negative window size.
    #[error("dock viewport placement space {space} has invalid window bounds")]
    InvalidWindowBounds {
        /// Dock space id.
        space: DockSpaceId,
    },
    /// A viewport placement has non-finite host coordinates or negative host size.
    #[error("dock viewport placement space {space} has invalid host bounds")]
    InvalidHostBounds {
        /// Dock space id.
        space: DockSpaceId,
    },
}

impl DockViewportPlacementLayout {
    /// Validates adapter-level placement invariants before applying snapshots.
    pub fn validate(&self) -> Result<(), DockViewportPlacementValidationError> {
        if self.placement_version != DOCK_VIEWPORT_PLACEMENT_VERSION {
            return Err(DockViewportPlacementValidationError::UnsupportedVersion {
                expected: DOCK_VIEWPORT_PLACEMENT_VERSION,
                found: self.placement_version,
            });
        }

        let mut spaces = BTreeSet::new();
        for viewport in &self.viewports {
            if !spaces.insert(viewport.space.clone()) {
                return Err(DockViewportPlacementValidationError::DuplicateSpace {
                    space: viewport.space.clone(),
                });
            }
            if let Some(window_bounds) = viewport.window_bounds
                && !window_bounds.bounds.is_finite_with_non_negative_size()
            {
                return Err(DockViewportPlacementValidationError::InvalidWindowBounds {
                    space: viewport.space.clone(),
                });
            }
            if let Some(host_bounds) = viewport.host_bounds
                && !host_bounds.is_finite_with_non_negative_size()
            {
                return Err(DockViewportPlacementValidationError::InvalidHostBounds {
                    space: viewport.space.clone(),
                });
            }
        }

        Ok(())
    }
}
