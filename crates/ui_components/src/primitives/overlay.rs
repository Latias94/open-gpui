//! Overlay policy primitives shared by concrete component adapters.

pub use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayLayerKind, OverlayLayerPolicy, OverlayLayerState,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence,
    OverlayResolvedState, resolve_escape_key, resolve_focus_restore, resolve_outside_press,
};
