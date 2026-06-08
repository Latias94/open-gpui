#![doc = "Retained docking graph and layout primitives for Open GPUI."]
//!
//! `open-gpui-docking` separates durable layout state from GPUI runtime state:
//!
//! - [`DockGraph`] stores logical dock spaces, tab stacks, splits, and in-window floating layout.
//! - [`DockLayout`] serializes that graph state without views or platform-window handles.
//! - [`DockController`] owns a mutable [`DockWorkspace`] and is the preferred shared owner for
//!   rendered hosts.
//! - [`DockHost`] renders one logical [`DockSpaceId`], with transient splitter, floating, and
//!   drop-preview sessions kept in the crate's interaction runtime.
//! - [`DockPanelRegistry`] maps item ids to [`DockPanelDescriptor`] metadata and GPUI view
//!   lifecycle state without storing either in the graph.
//! - [`DockViewportAdapter`] stores runtime window mappings and placement snapshots outside
//!   [`DockLayout`].
//! - [`DockViewportRuntime`] owns the controller-backed viewport lifecycle while
//!   [`DockViewportRuntimeHandle`] keeps GPUI application callbacks ergonomic.
//!
//! Common GPUI applications should start with [`DockController::builder`], register lazy panel
//! factories, and mount a controller-backed [`DockHost`]. Advanced callers can keep using
//! [`DockGraph`], [`DockLayoutBuilder`], [`DockWorkspace`], and [`DockAction`] directly.
//! In-window floating and platform viewport tear-off are separate [`DockPolicy`] capabilities so
//! applications can enable platform windows without changing graph-backed floating behavior.
//! Multi-window applications should keep one [`DockController`] as the graph and panel owner, wrap
//! it in a [`DockViewportRuntimeHandle`], open controller-backed viewport windows through the
//! runtime, and install [`DockViewportRuntimeHandle::observe_window_closed`] for post-close cleanup.
//! Runtime-opened windows install a should-close hook so [`DockViewportClosePolicy::Prevent`] can
//! veto platform closes before cleanup runs. Persist [`DockLayout`] and
//! [`DockViewportPlacementLayout`] separately: layout restores logical dock spaces, while placement
//! restores platform-window hints for the runtime adapter. Use [`DockViewportTargetContext`] when
//! cross-window drops need active, hovered, or front-to-back window arbitration; pointer-event
//! paths should prefer [`DockViewportTargetContext::from_window`] so the event window participates
//! as the hovered-window signal.
//!
//! ```rust,no_run
//! use open_gpui::{AnyView, App};
//! use open_gpui_docking::{
//!     DockController, EditorDockLayoutSpec,
//! };
//!
//! fn panel_factory(_cx: &mut App) -> AnyView {
//!     unreachable!("create and return a GPUI view for the panel")
//! }
//!
//! let controller = DockController::builder("main")
//!     .default_editor_layout(EditorDockLayoutSpec::new(
//!         ["explorer"],
//!         ["editor"],
//!         ["terminal"],
//!     ))
//!     .panel_factory("explorer", "Explorer", panel_factory)
//!     .panel_factory("editor", "Editor", panel_factory)
//!     .panel_factory("terminal", "Terminal", panel_factory)
//!     .allow_floating(true)
//!     .allow_platform_viewports(true)
//!     .build();
//! # let _ = controller;
//! ```
#![warn(missing_docs)]

mod action;
mod builder;
mod controller;
#[cfg(test)]
mod controller_builder_tests;
mod debug;
#[cfg(test)]
mod dock_op_fixture_tests;
mod drag;
mod drop_target;
mod geometry;
mod graph;
#[cfg(test)]
mod graph_floating_tests;
#[cfg(test)]
mod graph_split_tests;
mod host;
mod host_debug;
mod host_interactions;
mod host_render_actions;
mod host_render_session;
mod host_source;
mod ids;
mod interaction;
mod layout;
mod op;
mod panel;
mod panel_view;
mod policy;
mod render;
mod render_floating;
mod render_split;
mod splitter;
mod viewport;
mod viewport_close;
mod viewport_coordinates;
mod viewport_placement;
mod viewport_registry;
mod viewport_runtime;
mod viewport_target;
mod workspace;
mod workspace_action;

#[cfg(test)]
mod host_floating_tests;
#[cfg(test)]
mod host_interaction_tests;
#[cfg(test)]
mod host_panel_tests;
#[cfg(test)]
mod host_render_tests;
#[cfg(test)]
mod host_test_support;
#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod host_viewport_tests;
#[cfg(test)]
mod host_workspace_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;

pub use action::*;
pub use builder::*;
pub use controller::*;
pub use graph::*;
pub use host::*;
pub use host_source::*;
pub use ids::*;
pub use layout::*;
pub use op::*;
pub use panel::*;
pub use panel_view::*;
pub use policy::*;
pub use viewport::*;
pub use viewport_close::*;
pub use viewport_placement::*;
pub use viewport_registry::DockViewportSnapshot;
pub use viewport_runtime::*;
pub use viewport_target::*;
pub use workspace::*;
