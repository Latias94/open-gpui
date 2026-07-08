//! Common docking APIs for ordinary Open GPUI applications.

pub use crate::{
    DOCK_LAYOUT_VERSION, DOCK_VIEWPORT_PLACEMENT_VERSION, DockClassId, DockDropGuideStyle,
    DockItemId, DockLayout, DockLayoutValidationError, DockPanel, DockPanelAttachError,
    DockPanelCatalog, DockPanelCloseOutcome, DockPanelDescriptor, DockPanelOpenOutcome,
    DockPanelOpenPlacementSource, DockPanelPlacement, DockPanelPlacementTarget,
    DockPanelRegistration, DockPanelRegistry, DockPanelReopenPolicy, DockPolicy, DockPolicyError,
    DockSpaceId, DockSurface, DockSurfaceBuildError, DockSurfaceBuilder, DockSurfaceChange,
    DockSurfaceFloatingPanelSnapshot, DockSurfacePanelError, DockSurfacePanelLocation,
    DockSurfacePanelLocationKind, DockSurfacePanelOutcome, DockSurfacePanelSnapshot,
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportOpenOutcome, DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus,
    DockSurfaceViewportOpened, DockSurfaceViewportRestoreOutcome, DockSurfaceViewportRestoreReport,
    DockSurfaceViewportSession, DockSurfaceViewportShouldCloseOutcome,
    DockSurfaceViewportShouldCloseStatus, DockSurfaceViewportSpec, DockSurfaceViewportSpecError,
    DockSurfaceViewportUnavailable, DockViewportClosePolicy, DockViewportPlacement,
    DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportRestoreReadiness, DockViewportWindowBounds, DockViewportWindowState,
};
