//! Convenient re-exports for the Open GPUI UI foundation layer.

pub use crate::{
    a11y::{
        AccessibleAction, AccessibleTextPosition, AccessibleTextSelection, LivePoliteness,
        Orientation, Role, SemanticDescriptor, SortDirection, Toggled,
    },
    active_descendant::ActiveDescendant,
    adaptive::{
        AdaptiveQuerySource, DeviceAdaptiveClass, DeviceAdaptivePolicy, DeviceAdaptiveSnapshot,
        DeviceShellMode, DeviceShellSwitchPolicy, PanelAdaptiveClass, PanelAdaptivePolicy,
        device_adaptive_class, device_adaptive_snapshot, device_shell_mode, panel_adaptive_class,
    },
    collection::CollectionPosition,
    controllable_state::ControllableState,
    focus::{
        FocusResolution, FocusRestoreInput, FocusRestoreIntent, FocusScopeId, FocusScopeMode,
        FocusScopePolicy, FocusTargetAvailability, FocusTargetCandidate, FocusTargetId,
        InitialFocusIntent, resolve_focus_restore as resolve_focus_scope_restore,
    },
    geometry::{
        UiEdges, UiPoint, UiPx, UiRect, UiSize, ui_edges, ui_point, ui_px, ui_rect, ui_size,
    },
    overlay::{
        DismissReason, EscapeKeyPolicy, EscapeKeyResolution, OutsidePressOutcome,
        OutsidePressParticipation, OutsidePressPolicy, OutsidePressResolution, OverlayAnchorInput,
        OverlayEdges, OverlayLayer, OverlayLayerId, OverlayLayerKind, OverlayLayerPolicy,
        OverlayLayerState, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
        OverlayPresence, OverlayResolvedState, OverlaySize, Rect, anchor_rect_from_point,
        inset_rect, outer_bounds_with_window_margin, prefer_visual_bounds, rect,
        resolve_outside_press,
    },
    sizing::{Density, Sizable, Size, SizeScale},
    tokens::{
        ThemeDesignScales, ThemeElevationLayer, ThemeElevationScale, ThemeRadiusScale,
        ThemeSpacingScale, ThemeTokens, ThemeTypographyScale, TokenKey, semantic,
    },
};
