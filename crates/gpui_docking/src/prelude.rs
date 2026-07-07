//! Common docking APIs for ordinary Open GPUI applications.

pub use crate::{
    DOCK_LAYOUT_VERSION, DOCK_VIEWPORT_PLACEMENT_VERSION, DockAction, DockActionApplyError,
    DockActionOutcome, DockCentralRegion, DockClassId, DockController, DockControllerBuilder,
    DockDropGuideStyle, DockEdgeDockPlan, DockEdgeDockSizing, DockEdgeDockSizingScope,
    DockFloatingContainer, DockGraph, DockGraphMutationError, DockGraphValidationError, DockHost,
    DockHostOptions, DockItemId, DockLayout, DockLayoutBuilder, DockLayoutCentralRegion,
    DockLayoutFloatingContainer, DockLayoutNode, DockLayoutRect, DockLayoutSpace,
    DockLayoutValidationError, DockNode, DockNodeId, DockPanel, DockPanelAttachError,
    DockPanelCatalog, DockPanelDescriptor, DockPanelRegistration, DockPanelRegistry, DockPolicy,
    DockPolicyError, DockSpaceId, DockSpatialDirection, DockSplitResize, DockViewportCloseOutcome,
    DockViewportClosePolicy, DockViewportCloseStatus, DockViewportFocusRequest,
    DockViewportOpenOutcome, DockViewportOpenStatus, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreReadiness, DockViewportRuntimeHandle, DockViewportShouldCloseOutcome,
    DockViewportShouldCloseStatus, DockViewportUnregisterOutcome, DockViewportUnregisterReason,
    DockViewportWindowBounds, DockViewportWindowState, DockWorkspace, DropZone,
    EditorDockLayoutSpec, SplitAxis, dock_bounds,
};
