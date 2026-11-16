//! GPUI content and native transport for one renderer-neutral docking session.
//!
//! Dockspace owns topology, selection and gestures. This adapter owns GPUI
//! entities, physical mounts and exact frame submission. Applications retain
//! their document and history owners and create a separate entity for each View.

mod close;
mod config;
mod continuation;
mod element;
mod failure;
mod host;
mod host_native;
mod input;
mod inspection;
pub mod native;
pub mod native_restore;
pub mod persistence;
mod presentation;
#[cfg(test)]
mod tests;

pub use config::{DockspaceColors, DockspaceInputBindings, GpuiDockspaceConfig};
pub use dockspace;
pub use failure::{DockHostFailure, DockNativeTermination};
pub use host::{DockHostAdmissionError, DockPane, DockPaneError, DockRegion, DockSurface};
pub use inspection::{
    DockInspectionError, DockModelSurfaceInspection, DockMountInspection, DockNativeInspection,
    DockRegionInspection, DockSurfaceInspection,
};
pub use native_restore::DockNativeRestoreError;

use host::{HostState, SharedHost};
