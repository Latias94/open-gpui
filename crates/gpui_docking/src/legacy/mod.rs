//! Compatibility engine for pre-session docking integrations.
//!
//! Available only with `legacy-docking`. New workspaces use the crate-root
//! [`crate::DockSurface`]. This engine retains native qualification scenarios
//! until their shared-session counterparts have been validated on each platform.

mod accessibility_scene;
mod action;
pub mod advanced;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod benchmark_support;
mod binary_layout_policy;
#[cfg(test)]
mod binary_layout_policy_tests;
mod builder;
mod canonical_runtime;
#[cfg(test)]
mod canonical_runtime_tests;
mod chrome_geometry;
mod controller;
#[cfg(test)]
mod controller_builder_tests;
mod debug;
mod divider_hit_map;
#[cfg(test)]
mod dock_op_fixture_tests;
#[cfg(test)]
mod dockspace_scenario_fixture_tests;
#[cfg(any(test, feature = "test-support"))]
mod dockspace_testkit;
#[cfg(test)]
mod dockspace_testkit_tests;
mod drag;
mod drag_visual;
mod drop_preview;
mod drop_runtime;
mod drop_scene_fact;
mod drop_target;
mod geometry;
mod graph;
mod graph_assembly;
#[cfg(test)]
mod graph_floating_tests;
mod graph_index;
#[cfg(test)]
mod graph_kernel_tests;
#[cfg(test)]
mod graph_move_tests;
mod graph_patch;
#[cfg(test)]
mod graph_patch_tests;
#[cfg(test)]
mod graph_split_tests;
#[cfg(test)]
mod graph_test_support;
mod graph_transaction;
#[cfg(test)]
mod graph_validation_tests;
mod host;
mod host_debug;
mod host_interaction_outcome;
mod host_interactions;
mod host_render_actions;
mod host_render_session;
mod host_viewport_drop;
mod ids;
mod interaction;
mod layout;
mod layout_migration;
mod locked_drop_identity;
pub mod model;
mod native_captured_drag;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod native_scenario_support;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use native_captured_drag::{
    NativeCapturedReleaseObservationForTest, native_captured_drag_router_is_idle_for_test,
    native_captured_release_observation_for_test,
};
mod detached_tree;
mod op;
mod panel;
mod panel_catalog;
mod panel_registry;
mod panel_view;
mod payload_ref;
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
mod surface_activation_tests;
#[cfg(test)]
mod surface_owner_tests;
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
mod viewport_host_geometry;
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
mod viewport_runtime_lineage;
mod viewport_runtime_status;
mod viewport_target_context;
mod viewport_target_resolver;
mod viewport_tear_off;
mod viewport_tear_off_move;
mod viewport_tear_off_placement;
mod viewport_window_lifecycle;
mod viewport_window_ownership;
mod visual_affordance_scene;
pub(crate) use crate::visual_style;
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
mod host_multiview_scenario_tests;
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
mod host_viewport_preview_cleanup_tests;
#[cfg(test)]
mod host_viewport_preview_tests;
#[cfg(test)]
mod host_viewport_preview_visual_tests;
#[cfg(test)]
mod host_viewport_route_adapter_tests;
#[cfg(test)]
mod host_viewport_route_tests;
#[cfg(test)]
mod host_viewport_runtime_test_support;
#[cfg(test)]
mod host_viewport_tests;
#[cfg(test)]
mod host_zoom_focus_tests;
#[cfg(test)]
mod layout_migration_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod public_surface_tests;
#[cfg(test)]
mod viewport_runtime_native_owner_tests;
#[cfg(test)]
mod viewport_test_support;
#[cfg(test)]
mod visual_style_runtime_tests;
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
pub(crate) use controller::{DockController, DockControllerBuilder};
pub(crate) use debug::DockVisualAffordanceDebugSummary;
pub use geometry::DockDropGuideMetrics;
pub(crate) use graph::{
    DockCentralRegion, DockEdgeDockPlan, DockEdgeDockSizing, DockEdgeDockSizingScope,
    DockFloatingContainer, DockGraph, DockGraphValidationError, DockNode, DropZone, SplitAxis,
    dock_bounds,
};
#[cfg(test)]
pub(crate) use host::DockHostOptions;
pub(crate) use host::{DockHost, DockHostWindowBinding};
pub(crate) use ids::{
    CanonicalGraphAuthority, CanonicalGraphRevisionChange, CanonicalGraphVersion, CanonicalNodeId,
    DockFloatingId, DockGroupId, DockNodeId,
};
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
    DockSurface, DockSurfaceActivationOutcome, DockSurfaceActivationRequestId,
    DockSurfaceBuildError, DockSurfaceBuilder, DockSurfaceChange, DockSurfaceChangeCategory,
    DockSurfaceChangeEvent, DockSurfaceFloatingPanelSnapshot, DockSurfacePanelError,
    DockSurfacePanelLocation, DockSurfacePanelLocationKind, DockSurfacePanelOutcome,
    DockSurfacePanelSnapshot, DockSurfacePrimaryWindowOpenConflict,
    DockSurfacePrimaryWindowOpenOutcome, DockSurfacePrimaryWindowOpened,
    DockSurfacePrimaryWindowUnavailable, DockSurfaceSnapshot, DockSurfaceTransition,
    DockSurfaceViewportCloseOutcome, DockSurfaceViewportCloseStatus,
    DockSurfaceViewportFlagWarning, DockSurfaceViewportInputStatus,
    DockSurfaceViewportLifecycleReadiness, DockSurfaceViewportOpenOutcome,
    DockSurfaceViewportOpenReport, DockSurfaceViewportOpenStatus, DockSurfaceViewportOpened,
    DockSurfaceViewportPlatformCapabilities, DockSurfaceViewportPlatformReadiness,
    DockSurfaceViewportReadiness, DockSurfaceViewportReadinessReport,
    DockSurfaceViewportReadinessStatus, DockSurfaceViewportRestoreOutcome,
    DockSurfaceViewportRestoreReport, DockSurfaceViewportRouteStatus,
    DockSurfaceViewportShouldCloseOutcome, DockSurfaceViewportShouldCloseStatus,
    DockSurfaceViewportSpec, DockSurfaceViewportSpecError, DockSurfaceViewportStaleReason,
    DockSurfaceViewportUnavailable, DockSurfaceViewportUnsupportedFlag, DockSurfaceViewports,
    DockSurfaceWindowSessionOpeningRollbackReason, DockSurfaceWindowSessionPhase,
    DockSurfaceWindowSessionReason, DockSurfaceWindowSessionShutdownReason,
    DockSurfaceWindowSessionStatus,
};
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use surface::{
    DockSurfaceLiveUndockTestEvent, DockSurfaceLiveUndockTestEventKind,
    DockSurfaceLiveUndockTestObservation, DockSurfaceShutdownCaptureReleaseEvidence,
    DockSurfaceShutdownTestEvent, DockSurfaceShutdownTestEventKind,
    DockSurfaceShutdownTestObservation,
};
pub(crate) use viewport::*;
#[cfg(test)]
pub(crate) use viewport_activation::DockViewportWindowActivation;
pub(crate) use viewport_activation::{
    DockViewportActivationBackendFocusApply, DockViewportActivationBackendFocusObservation,
    DockViewportActivationBackendFocusRecordEffect, DockViewportActivationTransaction,
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
    DockViewportExactFocusTarget, DockViewportFocusCommand, DockViewportFocusCommandSource,
    DockViewportFocusCoordinator,
};
pub(crate) use viewport_frame_coordinator::*;
pub(crate) use viewport_host_geometry::*;
pub(crate) use viewport_identity::*;
pub(crate) use viewport_open::{DockViewportOpenOutcome, DockViewportOpenStatus};
pub(crate) use viewport_payload_drag::*;
pub use viewport_placement::{
    DOCK_VIEWPORT_PLACEMENT_VERSION, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportWindowBounds, DockViewportWindowState,
};
pub use viewport_placement_adapter::DockViewportRestoreReadiness;
pub use viewport_placement_validation::DockViewportPlacementValidationError;
pub(crate) use viewport_platform_signals::*;
#[cfg(test)]
pub(crate) use viewport_platform_sync::{
    sync_reused_viewport_window, unavailable_reused_viewport_window_sync,
};
pub(crate) use viewport_registration::*;
pub(crate) use viewport_registry::{DockViewportSnapshot, DockViewportWindowFacts};
pub(crate) use viewport_routed_preview::*;
pub(crate) use viewport_runtime::*;
pub(crate) use viewport_runtime_drop_resolution::*;
pub(crate) use viewport_runtime_effects::*;
pub(crate) use viewport_runtime_handle::{
    DockViewportLiveUndockProvisionalRetirementOutcome, DockViewportLockedDropRoute,
    DockViewportPreflightedLiveUndockHostDrop, DockViewportPreparedLiveUndockHostDrop,
    DockViewportRuntimeHandle, DockViewportRuntimeIdentity,
};
pub(crate) use viewport_runtime_lineage::*;
pub(crate) use viewport_runtime_status::*;
pub(crate) use viewport_target_context::*;
pub(crate) use viewport_target_resolver::*;
pub(crate) use viewport_tear_off::DockViewportTearOffCancelReason;
pub(crate) use viewport_tear_off::{
    DockViewportCommittedTearOffMove, DockViewportDropActionOutcome, DockViewportDropPayload,
    DockViewportDropRouteOutcome, DockViewportTearOffBeginOutcome, DockViewportTearOffCancelled,
    DockViewportTearOffCompleted, DockViewportTearOffMachine, DockViewportTearOffOpenOutcome,
    DockViewportTearOffPending, DockViewportTearOffRequest,
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
pub use visual_style::{
    DockDragVisualStyle, DockDropGuidePalette, DockDropGuideVisualState, DockFloatingVisualStyle,
    DockHostVisualStyle, DockPreviewVisualStyle, DockRoutePreviewPalette,
    DockRoutePreviewVisualState, DockSplitterVisualState, DockSplitterVisualStyle, DockTabPalette,
    DockTabVisualState, DockTabsVisualStyle, DockTargetPreviewPalette,
    DockTargetPreviewVisualState, DockVisualPalette, DockVisualStyle, DockVisualStyleResolver,
};
pub(crate) use workspace::DockWorkspace;
