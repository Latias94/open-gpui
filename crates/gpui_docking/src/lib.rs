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
//! - Tab drag/drop resolves pointer facts into a crate-internal full-layout target first; preview
//!   and commit both consume that resolved target so render callbacks do not assemble graph-shaped
//!   move commands.
//! - Split layout, splitter hit testing, drop preview rectangles, and central-region remaining
//!   space allocation are computed by shared geometry helpers.
//! - [`DockPanelRegistry`] maps item ids to [`DockPanelDescriptor`] metadata and GPUI view
//!   lifecycle state without storing either in the graph.
//! - [`DockPanelCatalog`] exposes descriptor-only metadata for policy, restore, and tab chrome
//!   paths that should not touch live GPUI view state.
//! - [`DockViewportAdapter`] stores runtime window mappings and placement snapshots outside
//!   [`DockLayout`].
//! - [`DockViewportRuntime`] owns the controller-backed viewport lifecycle while
//!   [`DockViewportRuntimeHandle`] keeps GPUI application callbacks ergonomic.
//!
//! Common GPUI applications should start with [`DockController::builder`], register lazy panel
//! factories, and mount a controller-backed [`DockHost`]. Rendered tab movement, splitter resize,
//! floating drag, and viewport tear-off flow through the crate's interaction, transaction, geometry,
//! and viewport-runtime modules. Advanced callers can still use [`DockGraph`],
//! [`DockLayoutBuilder`], [`DockWorkspace`], and [`DockAction`] directly for explicit programmatic
//! layout operations.
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
//! Panel close/reopen flows should use [`DockAction::CloseItem`] and [`DockAction::OpenItem`]:
//! close removes the item from the graph while the panel catalog remains available, and reopen
//! inserts that registered item back into a target tab stack or empty dock space. Ordinary tab
//! drag/drop uses resolved drop transactions internally rather than asking render code or app code
//! to construct graph-shaped move commands.
//! Descriptor-only restored panels can bind GPUI content later through
//! [`DockPanelRegistry::attach_factory`], [`DockWorkspace::attach_panel_factory`], or
//! [`DockController::attach_panel_factory`] without rewriting restored titles or close policy.
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
//!     .try_build()
//!     .expect("dock controller setup should validate");
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
mod drop_runtime;
mod drop_target;
mod geometry;
mod graph;
#[cfg(test)]
mod graph_floating_tests;
#[cfg(test)]
mod graph_move_tests;
#[cfg(test)]
mod graph_split_tests;
#[cfg(test)]
mod graph_test_support;
#[cfg(test)]
mod graph_validation_tests;
mod host;
mod host_debug;
mod host_interactions;
mod host_render_actions;
mod host_render_session;
mod ids;
mod interaction;
mod layout;
mod op;
mod panel;
mod panel_catalog;
mod panel_registry;
mod panel_view;
mod policy;
mod render;
mod render_floating;
mod render_split;
mod render_tabs;
mod split_fraction;
mod viewport;
mod viewport_close;
mod viewport_close_gate;
mod viewport_coordinates;
mod viewport_open;
mod viewport_placement;
mod viewport_placement_adapter;
mod viewport_placement_options;
mod viewport_placement_validation;
mod viewport_registration;
mod viewport_registry;
mod viewport_runtime;
mod viewport_runtime_handle;
mod viewport_target;
mod viewport_target_context;
mod viewport_target_resolver;
mod viewport_tear_off;
mod workspace;
mod workspace_action;
mod workspace_floating_transaction;
mod workspace_move_transaction;
mod workspace_move_validation;
mod workspace_panel_lifecycle;
mod workspace_panel_transaction;
mod workspace_resize_transaction;
mod workspace_transaction;

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
mod host_viewport_runtime_handle_tests;
#[cfg(test)]
mod host_viewport_runtime_tests;
#[cfg(test)]
mod host_viewport_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod viewport_test_support;
#[cfg(test)]
mod workspace_move_tests;
#[cfg(test)]
mod workspace_panel_lifecycle_tests;
#[cfg(test)]
mod workspace_resize_policy_tests;
#[cfg(test)]
mod workspace_selection_tests;
pub use action::*;
pub use builder::*;
pub use controller::*;
pub use graph::*;
pub use host::*;
pub use ids::*;
pub use layout::*;
pub(crate) use op::DockOp;
pub use op::DockOpApplyError;
#[cfg(test)]
pub(crate) use op::SplitFractionsUpdate;
pub use panel::*;
pub use panel_catalog::*;
pub use panel_registry::*;
pub use policy::*;
pub use viewport::*;
pub use viewport_close::*;
pub use viewport_open::*;
pub use viewport_placement::*;
pub use viewport_placement_adapter::*;
pub use viewport_placement_validation::*;
pub use viewport_registration::*;
pub use viewport_registry::DockViewportSnapshot;
pub use viewport_runtime::*;
pub use viewport_runtime_handle::*;
pub use viewport_target_context::*;
pub use viewport_target_resolver::*;
pub use viewport_tear_off::*;
pub use workspace::*;
