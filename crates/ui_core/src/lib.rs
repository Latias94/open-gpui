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
pub mod overlay;
pub mod prelude;
pub mod sizing;
pub mod split;
pub mod table;
pub mod tokens;
pub mod virtualizer;

pub use a11y::{
    AccessibleAction, AccessibleTextPosition, AccessibleTextSelection, Orientation, Role,
    SemanticDescriptor, SortDirection, Toggled,
};
pub use active_descendant::ActiveDescendant;
pub use adaptive::{
    AdaptiveQuerySource, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceAdaptiveSnapshot,
    DeviceShellMode, DeviceShellSwitchPolicy, PanelAdaptiveClass, PanelAdaptivePolicy,
    device_adaptive_class, device_adaptive_snapshot, device_shell_mode, panel_adaptive_class,
};
pub use collection::CollectionPosition;
pub use controllable_state::ControllableState;
pub use focus::{
    FocusResolution, FocusRestoreInput, FocusRestoreIntent, FocusScopeId, FocusScopeMode,
    FocusScopePolicy, FocusTargetAvailability, FocusTargetCandidate, FocusTargetId,
    InitialFocusIntent, resolve_focus_restore as resolve_focus_scope_restore,
};
pub use geometry::{
    UiEdges, UiPoint, UiPx, UiRect, UiSize, ui_edges, ui_point, ui_px, ui_rect, ui_size,
};
pub use grid_viewport::{GridViewport2D, RowWindow, RowWindowItem, resolve_grid_viewport_2d};
pub use overlay::{
    DismissReason, EscapeKeyPolicy, EscapeKeyResolution, OutsidePressOutcome,
    OutsidePressParticipation, OutsidePressPolicy, OutsidePressResolution, OverlayAnchorInput,
    OverlayEdges, OverlayLayer, OverlayLayerId, OverlayLayerKind, OverlayLayerPolicy,
    OverlayLayerState, OverlayPlacementAlignment, OverlayPlacementFit, OverlayPlacementInput,
    OverlayPlacementResolution, OverlayPlacementSide, OverlayPlacementTrace,
    OverlayPlacementTraceStep, OverlayPresence, OverlayResolvedState, OverlaySize, Rect,
    anchor_rect_from_point, inset_rect, outer_bounds_with_window_margin, prefer_visual_bounds,
    rect, resolve_escape_key, resolve_outside_press, resolve_overlay_placement,
};
pub use sizing::{Density, Sizable, Size, SizeScale};
pub use split::{
    SplitTreeChild, SplitTreeNode, SplitterHandleLayout, SplitterHandlePlacement,
    SplitterHandleState, SplitterHitMap, SplitterHitTarget, SplitterJunctionHitRegion,
    SplitterLayoutScene, SplitterMetrics, SplitterPanelDescriptor, SplitterPanelLayout,
    SplitterPanelState, SplitterResizeOutcome, SplitterResizeResult, SplitterState,
    normalize_split_fractions, resize_split_fractions_by_pixels, resolve_split_fractions,
    resolve_split_fractions_with_fill_child,
};
pub use table::{
    TABLE_DEFAULT_COLUMN_WIDTH, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH, TableAggregateKind,
    TableAggregation, TableCellEditor, TableCellValue, TableColumn, TableColumnFacets,
    TableColumnGroup, TableColumnGroupId, TableColumnId, TableColumnNode, TableColumnPinning,
    TableColumnRegion, TableColumnRegions, TableColumnResizeDirection, TableColumnResizeMode,
    TableColumnResizeState, TableColumnResizeUpdate, TableColumnSizing,
    TableColumnVisibilityOverrides, TableColumnWidthPolicy, TableExpansionMode,
    TableExpansionState, TableFacetRange, TableFacetValueCount, TableFilter, TableFilterKind,
    TableGlobalFacetSummary, TableGroupRow, TableGroupRowIdentity, TableGroupRowSegment,
    TableGroupValueIdentity, TableHeaderIdentity, TableHeaderRowIdentity, TableNumericFilterBound,
    TableNumericFilterOperator, TablePagination, TableResolvedColumnSizing,
    TableResolvedColumnSizingRegions, TableResolvedHeaderCell, TableResolvedHeaderGroup,
    TableResolvedHeaderGroupRegions, TableResolvedHeaderIdentity, TableResolvedHeaderKind,
    TableResolvedRow, TableResolvedRowKind, TableResolvedState, TableRow,
    TableRowChildrenLoadState, TableRowId, TableRowIdentity, TableRowIdentityDiagnostic,
    TableRowIdentityKey, TableRowInstanceId, TableRowModel, TableRowModelStage, TableRowPinTarget,
    TableRowPinning, TableRowPinningPolicy, TableRowRegion, TableRowRegions, TableSelectOption,
    TableSelectionActivationMode, TableSelectionMode, TableSelectionPolicy, TableSelectionSummary,
    TableSelectionSummaryState, TableSort, TableSortDirection, TableSourceInstanceIdentity,
    TableSourceRowIdentity, TableSourceRowLookup, TableStageMode, TableState,
    TableSubRowSelectionPolicy, TableTextFilterOperator, TableTreeRow, drag_table_column_resize,
    end_table_column_resize,
};
pub use tokens::{
    ThemeDesignScales, ThemeElevationLayer, ThemeElevationScale, ThemeRadiusScale,
    ThemeSpacingScale, ThemeTokens, ThemeTypographyScale, TokenKey, semantic,
};
pub use virtualizer::{
    VirtualizerItemGeometry, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange,
    VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerSnapshotItem, VirtualizerState,
};
