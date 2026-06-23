use crate::{DockClassId, DockItemId, DockSpaceId, DropZone};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Workspace-level policy for docking interactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockPolicy {
    allow_center_merge: bool,
    allow_edge_split: bool,
    allow_same_stack_center_drop: bool,
    allow_splitter_resize: bool,
    allow_floating: bool,
    allow_platform_viewports: bool,
    platform_focus_sets_dock_focus: bool,
    allow_central_region_dock_over: bool,
    allowed_dock_classes_by_space: BTreeMap<DockSpaceId, BTreeSet<DockClassId>>,
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

    /// Returns whether platform window focus restores the recorded dock-panel focus.
    pub fn platform_focus_sets_dock_focus(&self) -> bool {
        self.platform_focus_sets_dock_focus
    }

    /// Enables or disables restoring dock-panel focus when a platform window gains focus.
    ///
    /// This mirrors Dear ImGui's `ConfigViewportsPlatformFocusSetsImGuiFocus`: applications can
    /// disable it for platforms or window managers that focus windows eagerly.
    pub fn set_platform_focus_sets_dock_focus(&mut self, enabled: bool) {
        self.platform_focus_sets_dock_focus = enabled;
    }

    /// Returns whether center/tab-bar docking over a central region is allowed.
    pub fn allows_central_region_dock_over(&self) -> bool {
        self.allow_central_region_dock_over
    }

    /// Enables or disables center/tab-bar docking over a central region.
    pub fn set_allow_central_region_dock_over(&mut self, allowed: bool) {
        self.allow_central_region_dock_over = allowed;
    }

    /// Allows one dock class to be dropped into a specific dock space.
    ///
    /// A dock space without an allow-list accepts every class. Once an allow-list exists for a
    /// space, classed panels must match one of its entries. Unclassed panels remain accepted.
    pub fn allow_dock_class_in_space(
        &mut self,
        space: impl Into<DockSpaceId>,
        dock_class: impl Into<DockClassId>,
    ) {
        self.allowed_dock_classes_by_space
            .entry(space.into())
            .or_default()
            .insert(dock_class.into());
    }

    /// Replaces the allowed dock classes for a specific dock space.
    ///
    /// Passing an empty iterator creates a space rule that rejects every classed panel while still
    /// allowing unclassed panels.
    pub fn set_allowed_dock_classes_for_space(
        &mut self,
        space: impl Into<DockSpaceId>,
        dock_classes: impl IntoIterator<Item = impl Into<DockClassId>>,
    ) {
        self.allowed_dock_classes_by_space.insert(
            space.into(),
            dock_classes.into_iter().map(Into::into).collect(),
        );
    }

    /// Clears class restrictions for a specific dock space.
    pub fn clear_allowed_dock_classes_for_space(&mut self, space: &DockSpaceId) {
        self.allowed_dock_classes_by_space.remove(space);
    }

    /// Returns true when the class is accepted by the target dock space.
    pub fn allows_dock_class_in_space(
        &self,
        space: &DockSpaceId,
        dock_class: Option<&DockClassId>,
    ) -> bool {
        let Some(allowed) = self.allowed_dock_classes_by_space.get(space) else {
            return true;
        };
        match dock_class {
            Some(dock_class) => allowed.contains(dock_class),
            None => true,
        }
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

    pub(crate) fn validate_dock_class_for_item(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
        dock_class: Option<&DockClassId>,
    ) -> Result<(), DockPolicyError> {
        if self.allows_dock_class_in_space(space, dock_class) {
            Ok(())
        } else {
            Err(DockPolicyError::DockClassRejected {
                space: space.clone(),
                item: item.clone(),
                dock_class: dock_class.cloned(),
            })
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
            platform_focus_sets_dock_focus: true,
            allow_central_region_dock_over: true,
            allowed_dock_classes_by_space: BTreeMap::new(),
        }
    }
}

/// Reason a docking interaction is rejected by workspace policy.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockPolicyError {
    /// Center tab merging is disabled.
    #[error("center tab merge is disabled by docking policy")]
    CenterMergeDisabled,
    /// A visibly split payload cannot be merged into a non-empty center target.
    #[error("visibly split docking payload cannot be center-merged into a non-empty target")]
    SplitPayloadCenterMergeRejected,
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
    /// A panel class is not accepted by the target dock space.
    #[error("dock item {item} with class {dock_class:?} is not allowed in dock space {space}")]
    DockClassRejected {
        /// The target dock space.
        space: DockSpaceId,
        /// The item that was being docked.
        item: DockItemId,
        /// The rejected dock class, or `None` for an unclassed item.
        dock_class: Option<DockClassId>,
    },
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
        assert!(policy.platform_focus_sets_dock_focus());
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
        policy.set_platform_focus_sets_dock_focus(false);
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
        assert!(!policy.platform_focus_sets_dock_focus());
        assert_eq!(
            policy.validate_central_region_dock_over(),
            Err(DockPolicyError::CentralRegionDockOverDisabled)
        );
    }

    #[test]
    fn platform_focus_restore_policy_is_independent_from_platform_viewports() {
        let mut policy = DockPolicy::default();

        assert!(!policy.allows_platform_viewports());
        assert!(policy.platform_focus_sets_dock_focus());

        policy.set_platform_focus_sets_dock_focus(false);
        policy.set_allow_platform_viewports(true);

        assert!(policy.allows_platform_viewports());
        assert!(
            !policy.platform_focus_sets_dock_focus(),
            "platform focus restoration mirrors ImGui's opt-out and is not the same as tear-off support"
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

    #[test]
    fn dock_class_rules_are_opt_in_per_space() {
        let mut policy = DockPolicy::default();
        let main = DockSpaceId::from("main");
        let inspector = DockClassId::from("inspector");
        let editor = DockClassId::from("editor");

        assert!(policy.allows_dock_class_in_space(&main, Some(&inspector)));
        assert!(
            policy
                .validate_dock_class_for_item(&main, &DockItemId::from("outline"), None)
                .is_ok()
        );

        policy.allow_dock_class_in_space(main.clone(), inspector.clone());
        assert!(policy.allows_dock_class_in_space(&main, Some(&inspector)));
        assert!(!policy.allows_dock_class_in_space(&main, Some(&editor)));
        assert!(
            policy
                .validate_dock_class_for_item(&main, &DockItemId::from("unclassed"), None)
                .is_ok()
        );
        assert_eq!(
            policy.validate_dock_class_for_item(&main, &DockItemId::from("editor"), Some(&editor)),
            Err(DockPolicyError::DockClassRejected {
                space: main,
                item: DockItemId::from("editor"),
                dock_class: Some(editor),
            })
        );
    }
}
