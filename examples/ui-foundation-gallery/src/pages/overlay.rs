//! Overlay foundation page metadata.

use open_gpui::{Pixels, point, px, size};
use open_gpui_ui_components::{GpuiOverlayAdapterConfig, GpuiOverlayState};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayLayerPolicy, OverlayPresence, Rect, anchor_rect_from_point,
    outer_bounds_with_window_margin, prefer_visual_bounds, rect,
};

/// Page title.
pub const TITLE: &str = "Overlay";
/// Page summary.
pub const SUMMARY: &str =
    "Anchor geometry plus renderer-neutral overlay presence, dismissal, and focus policy.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "anchor_rect_from_point()",
    "prefer_visual_bounds()",
    "outer_bounds_with_window_margin()",
    "OverlayLayerPolicy",
    "OverlayPresence",
    "OutsidePressPolicy",
    "FocusRestoreIntent",
    "OverlayEdges",
    "OverlaySize",
];

/// Geometry used by the overlay demo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDemoGeometry {
    /// Point selected as the trigger anchor.
    pub trigger_point: open_gpui::Point<Pixels>,
    /// 1x1 anchor rect produced from the trigger point.
    pub anchor_rect: Rect,
    /// Layout rect that approximates the trigger bounds.
    pub layout_rect: Rect,
    /// Visual rect preferred for overlay positioning.
    pub visual_rect: Rect,
    /// Preferred rect resolved from visual/layout candidates.
    pub preferred_rect: Rect,
    /// Window bounds after applying the safe overlay margin.
    pub safe_window_rect: Rect,
}

/// Returns deterministic overlay geometry for the gallery.
pub fn demo_geometry() -> OverlayDemoGeometry {
    let trigger_point = point(px(312.0), px(168.0));
    let anchor_rect = anchor_rect_from_point(trigger_point);
    let layout_rect = rect(point(px(288.0), px(144.0)), size(px(176.0), px(40.0)));
    let visual_rect = rect(point(px(284.0), px(140.0)), size(px(184.0), px(48.0)));
    let preferred_rect = prefer_visual_bounds(Some(visual_rect), Some(layout_rect))
        .expect("visual or layout rect should be present");
    let safe_window_rect = outer_bounds_with_window_margin(
        rect(point(px(0.0), px(0.0)), size(px(640.0), px(360.0))),
        px(12.0),
    );

    OverlayDemoGeometry {
        trigger_point,
        anchor_rect,
        layout_rect,
        visual_rect,
        preferred_rect,
        safe_window_rect,
    }
}

/// Overlay behavior sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayBehaviorSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing sample label.
    pub label: &'static str,
    /// Resolved behavior policy.
    pub policy: OverlayLayerPolicy,
    /// Resolved GPUI adapter state.
    pub adapter: GpuiOverlayState,
}

/// Returns deterministic overlay behavior samples for the gallery.
pub fn behavior_samples() -> [OverlayBehaviorSample; 4] {
    [
        OverlayBehaviorSample {
            id: "tooltip",
            label: "Tooltip",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
            adapter: GpuiOverlayAdapterConfig::new(
                OverlayLayerKind::Tooltip,
                OverlayPresence::open(),
            )
            .state(),
        },
        OverlayBehaviorSample {
            id: "popover",
            label: "Popover",
            policy: OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
            adapter: GpuiOverlayAdapterConfig::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .state(),
        },
        OverlayBehaviorSample {
            id: "dialog",
            label: "Dialog",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
            adapter: GpuiOverlayAdapterConfig::new(
                OverlayLayerKind::Modal,
                OverlayPresence::open(),
            )
            .state(),
        },
        OverlayBehaviorSample {
            id: "menu",
            label: "Menu",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
            adapter: GpuiOverlayAdapterConfig::new(OverlayLayerKind::Menu, OverlayPresence::open())
                .state(),
        },
    ]
}

/// Returns a stable label for overlay layer kind.
pub const fn layer_kind_label(kind: OverlayLayerKind) -> &'static str {
    match kind {
        OverlayLayerKind::Tooltip => "tooltip",
        OverlayLayerKind::NonModalDismissible => "non-modal dismissible",
        OverlayLayerKind::Modal => "modal",
        OverlayLayerKind::Menu => "menu",
    }
}

/// Returns a stable label for outside-press policy.
pub const fn outside_press_label(policy: OutsidePressPolicy) -> &'static str {
    match policy {
        OutsidePressPolicy::Ignore => "ignore",
        OutsidePressPolicy::Consume => "consume",
        OutsidePressPolicy::DismissAndConsume => "dismiss + consume",
        OutsidePressPolicy::DismissAndPassThrough => "dismiss + pass-through",
    }
}

/// Returns a stable label for Escape-key policy.
pub const fn escape_key_label(policy: EscapeKeyPolicy) -> &'static str {
    match policy {
        EscapeKeyPolicy::Ignore => "ignore",
        EscapeKeyPolicy::Dismiss => "dismiss",
    }
}

/// Returns a stable label for focus restoration intent.
pub fn focus_restore_label(intent: &FocusRestoreIntent) -> &'static str {
    match intent {
        FocusRestoreIntent::None => "none",
        FocusRestoreIntent::Trigger => "trigger",
        FocusRestoreIntent::Fallback(_) => "fallback",
        FocusRestoreIntent::TriggerOrFallback(_) => "trigger or fallback",
    }
}

/// Returns a stable label for initial focus intent.
pub fn initial_focus_label(intent: &InitialFocusIntent) -> &'static str {
    match intent {
        InitialFocusIntent::None => "none",
        InitialFocusIntent::FirstFocusable => "first focusable",
        InitialFocusIntent::Target(_) => "target",
        InitialFocusIntent::TargetOrFirstFocusable(_) => "target or first focusable",
    }
}
