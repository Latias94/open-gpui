//! GPUI content and native transport for one renderer-neutral docking session.
//!
//! Dockspace owns topology, selection and gestures. This adapter owns GPUI
//! entities, physical mounts and exact frame submission. Applications retain
//! their document and history owners and create a separate entity for each View.

mod config;
mod continuation;
mod element;
mod host;
mod host_native;
mod input;
pub mod native;
pub mod persistence;
mod presentation;
#[cfg(test)]
mod tests;

pub use config::{DockspaceColors, DockspaceInputBindings, GpuiDockspaceConfig};
pub use dockspace;
pub use host::{DockPane, DockRegion, DockSurface};

use host::{HostState, SharedHost};
