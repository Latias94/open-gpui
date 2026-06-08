use crate::DropZone;
use thiserror::Error;

/// Workspace-level policy for docking interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockPolicy {
    allow_center_merge: bool,
    allow_edge_split: bool,
    allow_same_stack_center_drop: bool,
    allow_splitter_resize: bool,
    allow_floating: bool,
}

impl DockPolicy {
    /// Creates the default policy used by docking workspaces.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether center tab merges are allowed.
    pub fn allows_center_merge(&self) -> bool {
        self.allow_center_merge
    }

    /// Enables or disables center tab merges.
    pub fn set_allow_center_merge(&mut self, allowed: bool) {
        self.allow_center_merge = allowed;
    }

    /// Returns whether edge splits are allowed.
    pub fn allows_edge_split(&self) -> bool {
        self.allow_edge_split
    }

    /// Enables or disables edge splits.
    pub fn set_allow_edge_split(&mut self, allowed: bool) {
        self.allow_edge_split = allowed;
    }

    /// Returns whether same-stack center drops are accepted as no-ops.
    pub fn allows_same_stack_center_drop(&self) -> bool {
        self.allow_same_stack_center_drop
    }

    /// Enables or disables same-stack center drop no-ops.
    pub fn set_allow_same_stack_center_drop(&mut self, allowed: bool) {
        self.allow_same_stack_center_drop = allowed;
    }

    /// Returns whether splitter resize commits are allowed.
    pub fn allows_splitter_resize(&self) -> bool {
        self.allow_splitter_resize
    }

    /// Enables or disables splitter resize commits.
    pub fn set_allow_splitter_resize(&mut self, allowed: bool) {
        self.allow_splitter_resize = allowed;
    }

    /// Returns whether floating interactions are allowed.
    pub fn allows_floating(&self) -> bool {
        self.allow_floating
    }

    /// Enables or disables floating interactions.
    pub fn set_allow_floating(&mut self, allowed: bool) {
        self.allow_floating = allowed;
    }

    pub(crate) fn validate_drop_zone(&self, zone: DropZone) -> Result<(), DockPolicyError> {
        match zone {
            DropZone::Center if !self.allow_center_merge => {
                Err(DockPolicyError::CenterMergeDisabled)
            }
            DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom
                if !self.allow_edge_split =>
            {
                Err(DockPolicyError::EdgeSplitDisabled)
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn validate_same_stack_center_drop(&self) -> Result<(), DockPolicyError> {
        if self.allow_same_stack_center_drop {
            Ok(())
        } else {
            Err(DockPolicyError::SameStackCenterDropDisabled)
        }
    }

    pub(crate) fn validate_splitter_resize(&self) -> Result<(), DockPolicyError> {
        if self.allow_splitter_resize {
            Ok(())
        } else {
            Err(DockPolicyError::SplitterResizeDisabled)
        }
    }

    /// Returns a typed error when floating interactions are disabled.
    pub fn validate_floating(&self) -> Result<(), DockPolicyError> {
        if self.allow_floating {
            Ok(())
        } else {
            Err(DockPolicyError::FloatingDisabled)
        }
    }
}

impl Default for DockPolicy {
    fn default() -> Self {
        Self {
            allow_center_merge: true,
            allow_edge_split: true,
            allow_same_stack_center_drop: true,
            allow_splitter_resize: true,
            allow_floating: false,
        }
    }
}

/// Reason a docking interaction is rejected by workspace policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DockPolicyError {
    /// Center tab merging is disabled.
    #[error("center tab merge is disabled by docking policy")]
    CenterMergeDisabled,
    /// Edge splitting is disabled.
    #[error("edge split is disabled by docking policy")]
    EdgeSplitDisabled,
    /// Same-stack center drop no-ops are disabled.
    #[error("same-stack center drop is disabled by docking policy")]
    SameStackCenterDropDisabled,
    /// Splitter resizing is disabled.
    #[error("splitter resize is disabled by docking policy")]
    SplitterResizeDisabled,
    /// Floating interactions are disabled.
    #[error("floating is disabled by docking policy")]
    FloatingDisabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_allows_current_interactions() {
        let policy = DockPolicy::default();

        assert!(policy.validate_drop_zone(DropZone::Center).is_ok());
        assert!(policy.validate_drop_zone(DropZone::Right).is_ok());
        assert!(policy.validate_same_stack_center_drop().is_ok());
        assert!(policy.validate_splitter_resize().is_ok());
        assert_eq!(
            policy.validate_floating(),
            Err(DockPolicyError::FloatingDisabled)
        );
    }

    #[test]
    fn disabled_capabilities_return_typed_errors() {
        let mut policy = DockPolicy::default();
        policy.set_allow_center_merge(false);
        policy.set_allow_edge_split(false);
        policy.set_allow_same_stack_center_drop(false);
        policy.set_allow_splitter_resize(false);

        assert_eq!(
            policy.validate_drop_zone(DropZone::Center),
            Err(DockPolicyError::CenterMergeDisabled)
        );
        assert_eq!(
            policy.validate_drop_zone(DropZone::Left),
            Err(DockPolicyError::EdgeSplitDisabled)
        );
        assert_eq!(
            policy.validate_same_stack_center_drop(),
            Err(DockPolicyError::SameStackCenterDropDisabled)
        );
        assert_eq!(
            policy.validate_splitter_resize(),
            Err(DockPolicyError::SplitterResizeDisabled)
        );
    }
}
