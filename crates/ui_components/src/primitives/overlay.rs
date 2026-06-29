//! GPUI-facing overlay primitives and adapter helpers.

pub use crate::overlay::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState,
    OverlayOpenChange, OverlayResolvedState, default_deferred_priority, escape_open_change,
    focus_restore_requests_trigger, gpui_anchor, gpui_overlay_state, outside_press_open_change,
    point_anchor_placement,
};
