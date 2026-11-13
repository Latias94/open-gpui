//! Common retained docking APIs for Open GPUI applications.
//!
//! Layout and policy types belong to the shared semantic core. More specialized
//! native transport and persistence APIs live in [`crate::native`] and
//! [`crate::persistence`].

pub use crate::{
    DockPane, DockRegion, DockSurface, DockVisualPalette, DockVisualStyle, DockspaceColors,
    DockspaceInputBindings, GpuiDockspaceConfig,
};
pub use dockspace::{
    model::{
        DockAnchor, DockPlacement, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::DockspaceSession,
};
