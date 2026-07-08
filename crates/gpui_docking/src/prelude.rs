//! Common docking APIs for ordinary Open GPUI applications.

pub use crate::{
    DOCK_LAYOUT_VERSION, DOCK_VIEWPORT_PLACEMENT_VERSION, DockClassId, DockController,
    DockControllerBuilder, DockDropGuideStyle, DockItemId, DockLayout, DockLayoutValidationError,
    DockPanel, DockPanelAttachError, DockPanelCatalog, DockPanelCloseOutcome, DockPanelDescriptor,
    DockPanelOpenOutcome, DockPanelOpenPlacementSource, DockPanelPlacement,
    DockPanelPlacementTarget, DockPanelRegistration, DockPanelRegistry, DockPanelReopenPolicy,
    DockPolicy, DockPolicyError, DockSpaceId, DockSurface, DockSurfaceBuildError,
    DockSurfaceBuilder, DockSurfaceChange, DockSurfacePanelError, DockSurfacePanelOutcome,
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportShouldCloseOutcome,
    DockSurfaceViewportShouldCloseStatus, DockSurfaceViewportSpec, DockSurfaceViewportSpecError,
    DockSurfaceViewportUnavailable, DockViewportClosePolicy, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportWindowBounds, DockViewportWindowState,
};
