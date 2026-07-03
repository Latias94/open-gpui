#![warn(missing_docs)]

//! Renderer-neutral foundation primitives for the Open GPUI component ecosystem.
//!
//! This crate intentionally stays below the styled component layer. It provides stable vocabulary
//! for sizing, adaptive layout, tokens, accessibility, focus, and overlay helpers that are useful
//! across future component crates without depending on the GPUI runtime or renderer types.

pub mod a11y;
pub mod active_descendant;
pub mod adaptive;
pub mod collection;
pub mod controllable_state;
pub mod focus;
pub mod geometry;
pub mod grid_viewport;
pub mod motion;
pub mod motion_controller;
pub mod motion_policy;
pub mod motion_projection;
pub mod motion_runtime;
pub mod motion_spring;
pub mod motion_value;
pub mod overlay;
pub mod prelude;
pub mod sizing;
pub mod split;
pub mod table;
pub mod tokens;
pub mod virtualizer;

pub use a11y::{AccessibleAction, Orientation, Role, Toggled};
pub use active_descendant::ActiveDescendant;
pub use adaptive::{
    AdaptiveQuerySource, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceAdaptiveSnapshot,
    DeviceShellMode, DeviceShellSwitchPolicy, PanelAdaptiveClass, PanelAdaptivePolicy,
    device_adaptive_class, device_adaptive_snapshot, device_shell_mode, panel_adaptive_class,
};
pub use collection::CollectionPosition;
pub use controllable_state::ControllableState;
pub use focus::FocusTargetId;
pub use geometry::{
    UiEdges, UiPoint, UiPx, UiRect, UiSize, ui_edges, ui_point, ui_px, ui_rect, ui_size,
};
pub use grid_viewport::{GridViewport2D, RowWindow, RowWindowItem, resolve_grid_viewport_2d};
pub use motion::{MotionDuration, MotionEasing, MotionPreference, MotionSpec};
pub use motion_controller::{
    MotionFrameDemand, MotionFrameReason, MotionScalarController, MotionScalarControllerSample,
    MotionScalarTrack, MotionScalarTrackSample,
};
pub use motion_policy::{
    MOTION_POLICY_MAX_UI_DURATION, MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue,
    MotionPolicyReport, MotionPreviewTargetPolicy, validate_motion_policy,
};
pub use motion_projection::{MotionProjection, MotionProjectionSample, MotionProjectionScale};
pub use motion_runtime::{
    MotionEdge, MotionRetargetItem, MotionRetargetSet, MotionRunState, MotionSnapshot,
    MotionTimeline, MotionTimelineSample, MotionTimelineState, lerp_rect, motion_source_rect,
    preferred_motion_edge, retarget_motion_snapshots, reveal_rect_from_edge,
};
pub use motion_spring::{
    MotionModel, MotionPreset, MotionSpring, MotionSpringPhysics, MotionSpringPreset,
    MotionSpringSample, MotionSpringSpec,
};
pub use overlay::{
    DismissReason, EscapeKeyPolicy, EscapeKeyResolution, FocusRestoreIntent,
    FocusRestoreResolution, InitialFocusIntent, OutsidePressOutcome, OutsidePressPolicy,
    OutsidePressResolution, OverlayAnchorInput, OverlayEdges, OverlayFocusTarget, OverlayLayer,
    OverlayLayerId, OverlayLayerKind, OverlayLayerPolicy, OverlayLayerState,
    OverlayPlacementAlignment, OverlayPlacementFit, OverlayPlacementInput,
    OverlayPlacementResolution, OverlayPlacementSide, OverlayPlacementTrace,
    OverlayPlacementTraceStep, OverlayPresence, OverlayResolvedState, OverlaySize, Rect,
    anchor_rect_from_point, inset_rect, outer_bounds_with_window_margin, prefer_visual_bounds,
    rect, resolve_escape_key, resolve_focus_restore, resolve_outside_press,
    resolve_overlay_placement,
};
pub use sizing::{Density, Sizable, Size};
pub use split::{
    SplitTreeChild, SplitTreeNode, SplitterHandleLayout, SplitterHandlePlacement,
    SplitterHandleState, SplitterHandleTransition, SplitterHandleTransitionKind, SplitterHitMap,
    SplitterHitTarget, SplitterJunctionHitRegion, SplitterLayoutScene, SplitterLayoutTransition,
    SplitterMetrics, SplitterPanelDescriptor, SplitterPanelLayout, SplitterPanelState,
    SplitterPanelTransition, SplitterPanelTransitionKind, SplitterResizeOutcome,
    SplitterResizeResult, SplitterState, SplitterTransitionIntent, normalize_split_fractions,
    resize_split_fractions_by_pixels, resolve_split_fractions,
    resolve_split_fractions_with_fill_child,
};
pub use table::{
    TABLE_DEFAULT_COLUMN_WIDTH, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH,
    TABLE_ROW_MODEL_PIPELINE, TABLE_ROW_MODEL_V0_PIPELINE, TableAggregateKind, TableAggregation,
    TableCellEditor, TableCellValue, TableColumn, TableColumnFacets, TableColumnGroup,
    TableColumnGroupId, TableColumnId, TableColumnNode, TableColumnPinning, TableColumnRegion,
    TableColumnRegions, TableColumnResizeDirection, TableColumnResizeMode, TableColumnResizeState,
    TableColumnResizeUpdate, TableColumnSizing, TableColumnVisibilityOverrides,
    TableColumnWidthPolicy, TableExpansionMode, TableExpansionState, TableFacetRange,
    TableFacetValueCount, TableFilter, TableFilterKind, TableGlobalFacetSummary, TableGroupRow,
    TableNumericFilterBound, TableNumericFilterOperator, TablePagination,
    TableResolvedColumnSizing, TableResolvedColumnSizingRegions, TableResolvedHeaderCell,
    TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions, TableResolvedHeaderKind,
    TableResolvedRow, TableResolvedRowKind, TableResolvedState, TableRow,
    TableRowChildrenLoadState, TableRowId, TableRowModel, TableRowModelStage, TableRowPinning,
    TableRowPinningPolicy, TableRowRegion, TableRowRegions, TableSelectOption,
    TableSelectionActivationMode, TableSelectionMode, TableSelectionPolicy, TableSelectionSummary,
    TableSelectionSummaryState, TableSort, TableSortDirection, TableStageMode, TableState,
    TableStateCacheKey, TableSubRowSelectionPolicy, TableTextFilterOperator, TableTreeRow,
    drag_table_column_resize, end_table_column_resize,
};
pub use tokens::{ThemeTokens, TokenKey, semantic};
pub use virtualizer::{
    VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange, VirtualizerResolvedState,
    VirtualizerSnapshot, VirtualizerSnapshotItem, VirtualizerState,
};
