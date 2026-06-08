#![doc = "Retained docking graph and layout primitives for Open GPUI."]
//!
//! `open-gpui-docking` separates durable layout state from GPUI runtime state:
//!
//! - [`DockGraph`] stores logical dock spaces, tab stacks, splits, and in-window floating layout.
//! - [`DockLayout`] serializes that graph state without views or platform-window handles.
//! - [`DockController`] owns a mutable [`DockWorkspace`] and is the preferred shared owner for
//!   rendered hosts.
//! - [`DockHost`] renders one logical [`DockSpaceId`] and forwards UI actions to either its owned
//!   workspace or a shared controller.
//! - [`DockViewportAdapter`] stores runtime window mappings and placement snapshots outside
//!   [`DockLayout`].
//!
//! Common GPUI applications should start with [`DockController::builder`], register lazy panel
//! factories, and mount a controller-backed [`DockHost`]. Advanced callers can keep using
//! [`DockGraph`], [`DockLayoutBuilder`], [`DockWorkspace`], and [`DockAction`] directly.
//! In-window floating and platform viewport tear-off are separate [`DockPolicy`] capabilities so
//! applications can enable platform windows without changing graph-backed floating behavior.
//!
//! ```rust,no_run
//! use open_gpui::{AnyView, Context};
//! use open_gpui_docking::{
//!     DockController, DockHost, DockPolicy, EditorDockLayoutSpec,
//! };
//!
//! fn panel_factory(_cx: &mut Context<DockHost>) -> AnyView {
//!     unreachable!("create and return a GPUI view for the panel")
//! }
//!
//! let mut policy = DockPolicy::default();
//! policy.set_allow_floating(true);
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
//!     .policy(policy)
//!     .build();
//! # let _ = controller;
//! ```
#![warn(missing_docs)]

mod action;
mod builder;
mod controller;
mod debug;
mod drag;
mod drop_target;
mod geometry;
mod graph;
mod host;
mod ids;
mod layout;
mod op;
mod panel;
mod policy;
mod render;
mod splitter;
mod viewport;
mod workspace;

#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod tests;

pub use action::*;
pub use builder::*;
pub use controller::*;
pub use graph::*;
pub use host::*;
pub use ids::*;
pub use layout::*;
pub use op::*;
pub use panel::*;
pub use policy::*;
pub use viewport::*;
pub use workspace::*;
