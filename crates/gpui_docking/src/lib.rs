#![doc = "Retained docking graph and layout primitives for Open GPUI."]
//!
//! `open-gpui-docking` separates durable layout state from GPUI runtime state:
//!
//! - [`DockGraph`] stores logical dock spaces, tab stacks, splits, and in-window floating layout.
//! - [`DockLayout`] serializes that graph state without views or platform-window handles.
//! - [`DockSurface`] is the preferred application facade. It owns controller wiring, host-window
//!   creation, panel commands, and typed platform-viewport capability outcomes.
//! - [`DockController`] owns a mutable [`DockWorkspace`] for lower-level integrations and tests.
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
//! - An internal viewport adapter stores runtime window mappings and placement snapshots outside
//!   [`DockLayout`].
//! - [`DockViewportRuntimeHandle`] is the explicit runtime-tier entry point for callers that need
//!   lower-level window control; the controller-backed viewport runtime core stays internal so
//!   applications cannot bypass the handle's window hooks and transaction surface.
//!
//! Common GPUI applications should start with [`DockSurface::builder`], register lazy panel
//! factories, and open the primary host window through [`DockSurface::open_primary_window`].
//! Rendered tab movement, splitter resize, floating drag, and viewport tear-off flow through the
//! crate's interaction, transaction, geometry, and viewport-runtime modules. Advanced callers can
//! still use [`DockGraph`], [`DockLayoutBuilder`], [`DockWorkspace`], and explicit [`DockAction`]
//! command objects for programmatic layout operations, but applications should prefer
//! [`DockSurface`] panel commands or the named [`DockController`] and [`DockWorkspace`] methods for
//! common panel and floating flows.
//! In-window floating and platform viewport tear-off are separate [`DockPolicy`] capabilities so
//! applications can enable platform windows without changing graph-backed floating behavior.
//! Multi-window applications should keep one [`DockSurface`] as the graph, panel, and host-window
//! owner, open controller-backed viewport windows through [`DockSurface::open_viewport`], and let
//! the surface/runtime install post-close cleanup for those windows. Runtime-opened windows install
//! a should-close hook so [`DockViewportClosePolicy::Prevent`] can veto platform closes before
//! cleanup runs. Persist [`DockLayout`] and
//! [`DockViewportPlacementLayout`] separately: layout restores logical dock spaces, while placement
//! restores platform-window hints for the runtime adapter. Cross-window drops derive hovered-window
//! and front-to-back window-stack arbitration from GPUI runtime signals inside the crate.
//! Panel close/reopen flows should use [`DockController::close_item`],
//! [`DockController::open_item`], [`DockWorkspace::close_item`], or [`DockWorkspace::open_item`]:
//! close removes the item from the graph while the panel catalog remains available, and reopen
//! inserts that registered item back into a target tab stack or empty dock space. Ordinary tab
//! drag/drop uses resolved drop transactions internally rather than asking render code or app code
//! to construct graph-shaped move commands.
//! Descriptor-only restored panels can bind GPUI content later through
//! [`DockPanelRegistry::attach_view_handle`] or [`DockController::attach_panel_view`] without
//! rewriting restored titles or close policy.
//! Lazy panels should be registered up front with [`DockControllerBuilder::panel_factory`] or
//! [`DockWorkspace::register_panel_factory`].
//!
//! ```rust,no_run
//! use open_gpui::{AnyView, App};
//! use open_gpui_docking::prelude::{DockPanelPlacement, DockSurface};
//!
//! fn panel_factory(_cx: &mut App) -> AnyView {
//!     unreachable!("create and return a GPUI view for the panel")
//! }
//!
//! # fn configure(cx: &mut App) {
//! let surface = DockSurface::builder("main")
//!     .panel_placements([
//!         DockPanelPlacement::left_rail("explorer").fraction(0.24),
//!         DockPanelPlacement::center("editor").selected(),
//!         DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
//!     ])
//!     .panel_factory("explorer", "Explorer", panel_factory)
//!     .panel_factory("editor", "Editor", panel_factory)
//!     .panel_factory("terminal", "Terminal", panel_factory)
//!     .allow_floating(true)
//!     .allow_platform_viewports(true)
//!     .build(cx)
//!     .expect("dock surface setup should validate");
//! # let _ = surface;
//! # }
//! ```
#![warn(missing_docs)]

mod accessibility_scene;
mod action;
pub mod advanced;
mod builder;
mod chrome_geometry;
mod controller;
#[cfg(test)]
mod controller_builder_tests;
mod debug;
mod divider_hit_map;
#[cfg(test)]
mod dock_op_fixture_tests;
mod drag;
mod drop_preview;
mod drop_runtime;
mod drop_scene_fact;
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
mod host_drop_scene;
mod host_interaction_outcome;
mod host_interactions;
mod host_outside_release;
mod host_render_actions;
mod host_render_session;
mod host_viewport_drop;
mod ids;
mod interaction;
mod layout;
pub mod model;
mod op;
mod panel;
mod panel_catalog;
mod panel_registry;
mod panel_view;
mod policy;
pub mod prelude;
mod presentation_commands;
mod presentation_scene;
mod render;
mod render_floating;
mod render_split;
mod render_tabs;
pub mod runtime;
mod spatial_navigation;
#[cfg(test)]
mod spatial_navigation_tests;
mod split_geometry;
mod surface;
#[cfg(test)]
mod surface_tests;
mod transition_executor;
mod transition_geometry;
mod viewport;
mod viewport_activation;
mod viewport_backend_focus;
mod viewport_close;
mod viewport_coordinates;
mod viewport_drop_delivery;
mod viewport_drop_route;
mod viewport_drop_scene;
mod viewport_focus;
mod viewport_frame_coordinator;
mod viewport_identity;
mod viewport_open;
mod viewport_payload_drag;
mod viewport_placement;
mod viewport_placement_adapter;
mod viewport_placement_options;
mod viewport_placement_validation;
mod viewport_platform_signals;
mod viewport_platform_sync;
mod viewport_registration;
mod viewport_registry;
mod viewport_routed_preview;
mod viewport_runtime;
mod viewport_runtime_drop_resolution;
mod viewport_runtime_effects;
mod viewport_runtime_handle;
mod viewport_runtime_status;
mod viewport_target_context;
mod viewport_target_resolver;
mod viewport_tear_off;
mod viewport_tear_off_move;
mod viewport_tear_off_placement;
mod viewport_window_lifecycle;
mod viewport_window_ownership;
mod visual_affordance_scene;
mod workspace;
mod workspace_action;
mod workspace_drop_target;
mod workspace_drop_transaction;
mod workspace_floating_transaction;
mod workspace_merge_transaction;
mod workspace_move_transaction;
mod workspace_move_validation;
mod workspace_panel_lifecycle;
mod workspace_panel_transaction;
mod workspace_resize_transaction;
mod zoom_state;

#[cfg(test)]
mod host_accessibility_tests;
#[cfg(test)]
mod host_divider_hit_map_tests;
#[cfg(test)]
mod host_floating_tests;
#[cfg(test)]
mod host_interaction_tests;
#[cfg(test)]
mod host_panel_tests;
#[cfg(test)]
mod host_presentation_scene_tests;
#[cfg(test)]
mod host_render_geometry_parity_tests;
#[cfg(test)]
mod host_render_tests;
#[cfg(test)]
mod host_test_support;
#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod host_transition_tests;
#[cfg(test)]
mod host_viewport_close_tests;
#[cfg(test)]
mod host_viewport_lifecycle_tests;
#[cfg(test)]
mod host_viewport_matrix_tests;
#[cfg(test)]
mod host_viewport_model_tests;
#[cfg(test)]
mod host_viewport_placement_tests;
#[cfg(test)]
mod host_viewport_platform_capability_tests;
#[cfg(test)]
mod host_viewport_preview_tests;
#[cfg(test)]
mod host_viewport_preview_visual_tests;
#[cfg(test)]
mod host_viewport_route_tests;
#[cfg(test)]
mod host_viewport_runtime_test_support;
#[cfg(test)]
mod host_viewport_tests;
#[cfg(test)]
mod host_zoom_focus_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod public_surface_tests;
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
pub(crate) use action::{DockAction, DockActionApplyError, DockActionOutcome, DockSplitResize};
pub(crate) use builder::EditorDockLayoutSpec;
pub use builder::{DockPanelPlacement, DockPanelPlacementTarget};
pub use controller::{DockController, DockControllerBuilder};
pub(crate) use debug::DockVisualAffordanceDebugSummary;
pub use geometry::DockDropGuideStyle;
pub(crate) use graph::{
    DockCentralRegion, DockEdgeDockPlan, DockEdgeDockSizing, DockEdgeDockSizingScope,
    DockFloatingContainer, DockGraph, DockGraphValidationError, DockNode, DropZone, SplitAxis,
    dock_bounds,
};
pub(crate) use host::{DockHost, DockHostOptions};
pub(crate) use ids::DockNodeId;
pub use ids::{DockClassId, DockItemId, DockSpaceId};
pub use layout::{DOCK_LAYOUT_VERSION, DockLayout, DockLayoutRect, DockLayoutValidationError};
pub(crate) use op::DockGraphMutationError;
pub(crate) use op::{DockGraphDropTarget, DockOp};
pub use panel::{
    DockPanel, DockPanelCloseOutcome, DockPanelOpenOutcome, DockPanelOpenPlacementSource,
};
pub use panel_catalog::{DockPanelCatalog, DockPanelDescriptor, DockPanelReopenPolicy};
pub use panel_registry::{DockPanelAttachError, DockPanelRegistration, DockPanelRegistry};
pub use policy::{DockPolicy, DockPolicyError};
pub use surface::{
    DockSurface, DockSurfaceBuildError, DockSurfaceBuilder, DockSurfaceChange,
    DockSurfacePanelError, DockSurfacePanelOutcome, DockSurfaceViewportOpenOutcome,
    DockSurfaceViewportOpenStatus, DockSurfaceViewportOpened, DockSurfaceViewportUnavailable,
};
pub(crate) use viewport::*;
#[cfg(test)]
pub(crate) use viewport_activation::DockViewportWindowActivation;
pub(crate) use viewport_activation::{
    DockViewportActivationBackendFocusApply, DockViewportActivationBackendFocusObservation,
    DockViewportActivationBackendFocusRecordEffect,
    DockViewportActivationPendingBackendFocusEffect, DockViewportActivationTransaction,
};
pub(crate) use viewport_backend_focus::*;
pub use viewport_close::DockViewportClosePolicy;
#[allow(unused_imports)]
pub(crate) use viewport_close::{
    DockMergeBackTarget, DockViewportCloseCoordinator, DockViewportClosePlanEffect,
    DockViewportClosePlanState, DockViewportMergeBackClosePlan,
    commit_prevalidated_merge_back_plan,
};
pub(crate) use viewport_close::{
    DockViewportCloseOutcome, DockViewportCloseStatus, DockViewportShouldCloseOutcome,
    DockViewportShouldCloseStatus, DockViewportUnregisterOutcome, DockViewportUnregisterReason,
};
pub(crate) use viewport_drop_delivery::*;
pub(crate) use viewport_drop_route::*;
pub(crate) use viewport_focus::DockViewportFocusRequest;
pub(crate) use viewport_focus::{
    DockViewportFocusCommand, DockViewportFocusCommandSource, DockViewportFocusCoordinator,
};
pub(crate) use viewport_frame_coordinator::*;
pub(crate) use viewport_identity::*;
pub(crate) use viewport_open::{DockViewportOpenOutcome, DockViewportOpenStatus};
pub(crate) use viewport_payload_drag::*;
pub use viewport_placement::{
    DOCK_VIEWPORT_PLACEMENT_VERSION, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportWindowBounds, DockViewportWindowState,
};
pub(crate) use viewport_placement_adapter::DockViewportRestoreReadiness;
pub(crate) use viewport_placement_validation::DockViewportPlacementValidationError;
pub(crate) use viewport_platform_signals::*;
#[cfg(test)]
pub(crate) use viewport_platform_sync::{
    DockViewportPlatformFlagRequests, sync_reused_viewport_window,
    unavailable_reused_viewport_window_sync, unsupported_viewport_platform_flag_requests,
};
pub(crate) use viewport_registration::*;
pub(crate) use viewport_registry::{DockViewportSnapshot, DockViewportWindowFacts};
pub(crate) use viewport_routed_preview::*;
pub(crate) use viewport_runtime::*;
pub(crate) use viewport_runtime_drop_resolution::*;
pub(crate) use viewport_runtime_effects::*;
pub(crate) use viewport_runtime_handle::DockViewportRuntimeHandle;
pub(crate) use viewport_runtime_status::*;
pub(crate) use viewport_target_context::*;
pub(crate) use viewport_target_resolver::*;
pub(crate) use viewport_tear_off::DockViewportTearOffCancelReason;
pub(crate) use viewport_tear_off::{
    DockViewportCommittedTearOffMove, DockViewportDropActionOutcome, DockViewportDropPayload,
    DockViewportDropRouteOutcome, DockViewportTearOffBeginOutcome, DockViewportTearOffCancelled,
    DockViewportTearOffCompleted, DockViewportTearOffKey, DockViewportTearOffMachine,
    DockViewportTearOffOpenOutcome, DockViewportTearOffPending, DockViewportTearOffRequest,
};
pub(crate) use viewport_tear_off_move::*;
#[cfg(test)]
pub(crate) use viewport_tear_off_placement::DockViewportTearOffPlacementSource;
pub(crate) use viewport_tear_off_placement::{
    DockViewportTearOffPlacement, DockViewportTearOffPlacementPolicy,
};
#[allow(unused_imports)]
pub(crate) use viewport_window_lifecycle::{
    DockViewportReusableWindow, DockViewportReusableWindowOutcome,
};
pub(crate) use viewport_window_ownership::*;
pub(crate) use workspace::DockWorkspace;
