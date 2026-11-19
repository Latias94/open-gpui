//! Common retained docking APIs for Open GPUI applications.
//!
//! Layout and policy types belong to the shared semantic core. More specialized
//! native transport and persistence APIs live in [`crate::native`] and
//! [`crate::persistence`].

pub use crate::{
    DockCommitNotification, DockPane, DockRegion, DockSurface, DockTabContentContext,
    DockVisualPalette, DockVisualStyle, DockspaceColors, DockspaceInputBindings,
    GpuiDockspaceConfig, SplitterDoubleClick,
};
pub use dockspace::{
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::DockspaceSession,
};
