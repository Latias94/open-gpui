//! Retained GPUI Views and native transport for one shared docking session.
//!
//! [`dockspace::runtime::DockspaceSession`] owns layout, selection, gestures and
//! durable pane identities. [`DockSurface`] binds that session to application
//! entities and physical GPUI windows. A View retains its entity when moved;
//! separate Views of one document need distinct entities and pane identities.
//!
//! Start with [`DockSurface::new`] for embedded content or
//! [`DockSurface::new_managed`] for native workspaces. Render one logical surface
//! through [`DockSurface::element`], or combine disjoint logical surfaces in one
//! Window through [`DockSurface::regions`]. The adapter aggregates them under the
//! Window's original exact frame ticket. Documents, history and engine rendering
//! remain application-owned.
//!
//! [`persistence`] stores a versioned session document and stable external View
//! keys without GPUI entities or native handles. Its one-way legacy v2/v3
//! conversion does not require the compatibility engine.
//!
//! The `legacy-docking` feature exposes the displaced graph/controller engine
//! only under `legacy`. Enabling it never changes the types at the crate root
//! or in [`prelude`]. It exists to retain explicit platform qualification paths
//! while their shared-session replacements are validated.
#![warn(missing_docs)]

pub mod adapter;
#[cfg(feature = "legacy-docking")]
pub mod legacy;
pub mod prelude;
mod visual_style;

pub use adapter::{
    DockInspectionError, DockModelSurfaceInspection, DockMountInspection, DockNativeInspection,
    DockPane, DockRegion, DockRegionInspection, DockSurface, DockSurfaceInspection,
    DockspaceColors, DockspaceInputBindings, GpuiDockspaceConfig, dockspace, native, persistence,
};
pub use visual_style::{
    DockDragVisualStyle, DockDropGuidePalette, DockDropGuideVisualState, DockFloatingVisualStyle,
    DockHostVisualStyle, DockPreviewVisualStyle, DockRoutePreviewPalette,
    DockRoutePreviewVisualState, DockSplitterVisualState, DockSplitterVisualStyle, DockTabPalette,
    DockTabVisualState, DockTabsVisualStyle, DockTargetPreviewPalette,
    DockTargetPreviewVisualState, DockVisualPalette, DockVisualStyle,
};
