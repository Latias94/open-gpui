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
    allow_platform_viewports: bool,
    allow_central_region_dock_over: bool,
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

    /// Returns whether platform viewport tear-off interactions are allowed.
    pub fn allows_platform_viewports(&self) -> bool {
        self.allow_platform_viewports
    }

    /// Enables or disables platform viewport tear-off interactions.
    pub fn set_allow_platform_viewports(&mut self, allowed: bool) {
        self.allow_platform_viewports = allowed;
    }

    /// Returns whether center/tab-bar docking over a central region is allowed.
    pub fn allows_central_region_dock_over(&self) -> bool {
        self.allow_central_region_dock_over
    }

    /// Enables or disables center/tab-bar docking over a central region.
    pub fn set_allow_central_region_dock_over(&mut self, allowed: bool) {
        self.allow_central_region_dock_over = allowed;
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

    /// Returns a typed error when platform viewport tear-off interactions are disabled.
    pub fn validate_platform_viewports(&self) -> Result<(), DockPolicyError> {
        if self.allow_platform_viewports {
            Ok(())
        } else {
            Err(DockPolicyError::PlatformViewportsDisabled)
        }
    }

    pub(crate) fn validate_central_region_dock_over(&self) -> Result<(), DockPolicyError> {
        if self.allow_central_region_dock_over {
            Ok(())
        } else {
            Err(DockPolicyError::CentralRegionDockOverDisabled)
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
            allow_platform_viewports: false,
            allow_central_region_dock_over: true,
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
    /// Platform viewport tear-off interactions are disabled.
    #[error("platform viewports are disabled by docking policy")]
    PlatformViewportsDisabled,
    /// Docking over the central region is disabled.
    #[error("central region dock-over is disabled by docking policy")]
    CentralRegionDockOverDisabled,
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
        assert_eq!(
            policy.validate_platform_viewports(),
            Err(DockPolicyError::PlatformViewportsDisabled)
        );
        assert!(policy.validate_central_region_dock_over().is_ok());
    }

    #[test]
    fn disabled_capabilities_return_typed_errors() {
        let mut policy = DockPolicy::default();
        policy.set_allow_center_merge(false);
        policy.set_allow_edge_split(false);
        policy.set_allow_same_stack_center_drop(false);
        policy.set_allow_splitter_resize(false);
        policy.set_allow_platform_viewports(false);
        policy.set_allow_central_region_dock_over(false);

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
        assert_eq!(
            policy.validate_platform_viewports(),
            Err(DockPolicyError::PlatformViewportsDisabled)
        );
        assert_eq!(
            policy.validate_central_region_dock_over(),
            Err(DockPolicyError::CentralRegionDockOverDisabled)
        );
    }

    #[test]
    fn platform_viewports_are_independent_from_in_window_floating() {
        let mut policy = DockPolicy::default();

        policy.set_allow_platform_viewports(true);
        assert!(policy.allows_platform_viewports());
        assert!(policy.validate_platform_viewports().is_ok());
        assert!(!policy.allows_floating());
        assert_eq!(
            policy.validate_floating(),
            Err(DockPolicyError::FloatingDisabled)
        );

        policy.set_allow_floating(true);
        policy.set_allow_platform_viewports(false);
        assert!(policy.allows_floating());
        assert_eq!(
            policy.validate_platform_viewports(),
            Err(DockPolicyError::PlatformViewportsDisabled)
        );
    }
}
