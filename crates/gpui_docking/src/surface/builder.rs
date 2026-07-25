use super::{DockSurface, DockSurfaceSnapshot};
use crate::{
    DockClassId, DockController, DockControllerBuilder, DockDropGuideMetrics, DockItemId,
    DockLayout, DockLayoutValidationError, DockPanel, DockPanelDescriptor, DockPanelPlacement,
    DockPolicy, DockSpaceId, DockViewportClosePolicy, DockVisualStyleResolver,
    EditorDockLayoutSpec,
};
use open_gpui::{AnyView, App, AppContext as _, Pixels};
use open_gpui_motion::MotionPreference;
use thiserror::Error;

/// Builder for [`DockSurface`].
#[derive(Debug)]
pub struct DockSurfaceBuilder {
    controller: DockControllerBuilder,
    close_policy: DockViewportClosePolicy,
    visual_style_resolver: Option<DockVisualStyleResolver>,
}

/// Error returned when a facade docking surface cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockSurfaceBuildError {
    /// The configured layout graph did not validate.
    #[error("dock surface layout is invalid: {message}")]
    InvalidLayout {
        /// Validation message from the low-level model validator.
        message: String,
    },
}

impl DockSurfaceBuilder {
    /// Creates a builder from the existing controller builder.
    pub fn new(space: impl Into<DockSpaceId>) -> Self {
        Self {
            controller: DockController::builder(space),
            close_policy: DockViewportClosePolicy::default(),
            visual_style_resolver: None,
        }
    }

    /// Restores the durable layout graph from serialized dock layout data.
    pub fn try_layout(
        mut self,
        layout: &DockLayout,
    ) -> std::result::Result<Self, DockLayoutValidationError> {
        self.controller = self.controller.try_layout(layout)?;
        Ok(self)
    }

    /// Restores the durable layout graph from an app-level surface snapshot.
    ///
    /// Viewport placement in the snapshot is restored after building through
    /// [`DockSurface::viewports`].
    pub fn try_snapshot(
        self,
        snapshot: &DockSurfaceSnapshot,
    ) -> std::result::Result<Self, DockLayoutValidationError> {
        self.try_layout(snapshot.layout())
    }

    /// Replaces the durable layout graph with the common editor-style layout.
    pub fn default_editor_layout(mut self, spec: EditorDockLayoutSpec) -> Self {
        self.controller = self.controller.default_editor_layout(spec);
        self
    }

    /// Replaces the durable layout graph with product-level panel placements.
    pub fn panel_placements(
        mut self,
        placements: impl IntoIterator<Item = DockPanelPlacement>,
    ) -> Self {
        self.controller = self.controller.panel_placements(placements);
        self
    }

    /// Registers descriptor-only panel metadata.
    pub fn panel_descriptor(
        mut self,
        item: impl Into<DockItemId>,
        descriptor: DockPanelDescriptor,
    ) -> Self {
        self.controller = self.controller.panel_descriptor(item, descriptor);
        self
    }

    /// Registers a prepared panel.
    pub fn panel(mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Self {
        self.controller = self.controller.panel(item, panel);
        self
    }

    /// Registers a lazy GPUI view factory as panel content.
    pub fn panel_factory(
        mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut App) -> AnyView + 'static,
    ) -> Self {
        self.controller = self.controller.panel_factory(item, title, factory);
        self
    }

    /// Replaces the message rendered when the selected dock space has no root node.
    pub fn empty_message(mut self, message: impl Into<String>) -> Self {
        self.controller = self.controller.empty_message(message);
        self
    }

    /// Replaces the message prefix rendered when a selected panel is missing from the registry.
    pub fn missing_panel_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.controller = self.controller.missing_panel_prefix(prefix);
        self
    }

    /// Replaces the minimum rendered size for split panes during splitter resizing.
    pub fn split_min_size(mut self, size: Pixels) -> Self {
        self.controller = self.controller.split_min_size(size);
        self
    }

    /// Replaces the hit target and visual thickness for rendered splitter handles.
    pub fn splitter_handle_size(mut self, size: Pixels) -> Self {
        self.controller = self.controller.splitter_handle_size(size);
        self
    }

    /// Replaces the structural metrics used to size and hit-test dock drop guides.
    pub fn drop_guide_metrics(mut self, metrics: DockDropGuideMetrics) -> Self {
        self.controller = self.controller.drop_guide_metrics(metrics);
        self
    }

    /// Replaces the host-owned motion preference for docking transitions.
    pub fn motion_preference(mut self, preference: MotionPreference) -> Self {
        self.controller = self.controller.motion_preference(preference);
        self
    }

    /// Replaces the docking interaction policy.
    pub fn policy(mut self, policy: DockPolicy) -> Self {
        self.controller = self.controller.policy(policy);
        self
    }

    /// Enables or disables in-window floating interactions.
    pub fn allow_floating(mut self, allowed: bool) -> Self {
        self.controller = self.controller.allow_floating(allowed);
        self
    }

    /// Enables or disables platform viewport interactions.
    pub fn allow_platform_viewports(mut self, allowed: bool) -> Self {
        self.controller = self.controller.allow_platform_viewports(allowed);
        self
    }

    /// Enables or disables restoring dock-panel focus when a platform window gains focus.
    pub fn platform_focus_sets_dock_focus(mut self, enabled: bool) -> Self {
        self.controller = self.controller.platform_focus_sets_dock_focus(enabled);
        self
    }

    /// Allows one dock class to be dropped into the given dock space.
    pub fn allow_dock_class_in_space(
        mut self,
        space: impl Into<DockSpaceId>,
        dock_class: impl Into<DockClassId>,
    ) -> Self {
        self.controller = self.controller.allow_dock_class_in_space(space, dock_class);
        self
    }

    /// Replaces the close policy for runtime-opened viewport windows.
    pub fn close_policy(mut self, close_policy: DockViewportClosePolicy) -> Self {
        self.close_policy = close_policy;
        self
    }

    /// Installs the immutable render-time visual-style resolver for every surface host.
    pub fn visual_style_resolver(mut self, resolver: DockVisualStyleResolver) -> Self {
        self.visual_style_resolver = Some(resolver);
        self
    }

    /// Builds the surface after validating controller graph state.
    pub fn build(self, cx: &mut App) -> Result<DockSurface, DockSurfaceBuildError> {
        let controller =
            self.controller
                .try_build()
                .map_err(|error| DockSurfaceBuildError::InvalidLayout {
                    message: error.to_string(),
                })?;
        let controller = cx.new(|_| controller);
        Ok(
            DockSurface::from_controller_with_close_policy_and_visual_style_resolver(
                controller,
                self.close_policy,
                self.visual_style_resolver,
                cx,
            ),
        )
    }
}
