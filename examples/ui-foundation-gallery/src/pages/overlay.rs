//! Overlay foundation page metadata.

use open_gpui::{Pixels, point, px, size};
use std::time::Duration;

use open_gpui_ui_components::{
    GpuiOverlayAdapterConfig, GpuiOverlayState, Popover, PopoverState, Tooltip, TooltipDelayPolicy,
    TooltipOpenIntent, TooltipState,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayLayerPolicy, OverlayPlacementAlignment, OverlayPlacementSide, OverlayPresence, Rect,
    Sizable, Size, ThemeTokens, anchor_rect_from_point, outer_bounds_with_window_margin,
    prefer_visual_bounds, rect,
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
    "TooltipState",
    "PopoverState",
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

/// Tooltip sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Text shown by the tooltip surface.
    pub tooltip_text: &'static str,
    /// Resolved tooltip state.
    pub state: TooltipState,
}

/// Returns deterministic tooltip samples for gallery dogfood.
pub fn tooltip_samples(tokens: ThemeTokens) -> [TooltipSample; 4] {
    [
        TooltipSample {
            id: "hover-focus",
            label: "Hover or focus",
            tooltip_text: "Visible from pointer hover or keyboard focus.",
            state: Tooltip::new(
                "overlay-tooltip:hover-focus",
                "Visible from pointer hover or keyboard focus.",
            )
            .placement_side(OverlayPlacementSide::Top)
            .placement_alignment(OverlayPlacementAlignment::Center)
            .tokens(tokens)
            .state(),
        },
        TooltipSample {
            id: "focus-only",
            label: "Focus only",
            tooltip_text: "Keyboard focus can reveal this tooltip without pointer input.",
            state: Tooltip::new(
                "overlay-tooltip:focus-only",
                "Keyboard focus can reveal this tooltip without pointer input.",
            )
            .open_intent(TooltipOpenIntent::Focus)
            .placement_side(OverlayPlacementSide::Bottom)
            .tokens(tokens)
            .state(),
        },
        TooltipSample {
            id: "delayed-manual",
            label: "Manual delayed",
            tooltip_text: "Resolved state keeps explicit delay policy and controlled open.",
            state: Tooltip::new(
                "overlay-tooltip:delayed-manual",
                "Resolved state keeps explicit delay policy and controlled open.",
            )
            .open(true)
            .open_intent(TooltipOpenIntent::Manual)
            .delay(TooltipDelayPolicy::new(
                Duration::from_millis(120),
                Duration::from_millis(40),
                Duration::from_millis(250),
            ))
            .placement_side(OverlayPlacementSide::Right)
            .with_size(Size::Small)
            .tokens(tokens)
            .state(),
        },
        TooltipSample {
            id: "disabled",
            label: "Disabled",
            tooltip_text: "Disabled triggers do not expose a focusable tooltip target.",
            state: Tooltip::new(
                "overlay-tooltip:disabled",
                "Disabled triggers do not expose a focusable tooltip target.",
            )
            .open(true)
            .disabled(true)
            .placement_side(OverlayPlacementSide::Left)
            .tokens(tokens)
            .state(),
        },
    ]
}

/// Popover sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct PopoverSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Text shown by the popover surface.
    pub content_text: &'static str,
    /// Resolved popover state.
    pub state: PopoverState,
}

/// Returns deterministic popover samples for gallery dogfood.
pub fn popover_samples(tokens: ThemeTokens) -> [PopoverSample; 4] {
    [
        PopoverSample {
            id: "default-open",
            label: "Default open",
            content_text: "Uncontrolled popover initialized open.",
            state: Popover::new(
                "overlay-popover:default-open",
                "Default open",
                "Uncontrolled popover initialized open.",
            )
            .default_open(true)
            .tokens(tokens)
            .state(),
        },
        PopoverSample {
            id: "controlled",
            label: "Controlled",
            content_text: "The gallery shell owns this open state.",
            state: Popover::new(
                "overlay-popover:controlled",
                "Controlled",
                "The gallery shell owns this open state.",
            )
            .open(false)
            .placement_side(OverlayPlacementSide::Right)
            .tokens(tokens)
            .state(),
        },
        PopoverSample {
            id: "consume-outside",
            label: "Consume outside",
            content_text: "Outside press dismisses and consumes the event.",
            state: Popover::new(
                "overlay-popover:consume-outside",
                "Consume outside",
                "Outside press dismisses and consumes the event.",
            )
            .open(true)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .placement_alignment(OverlayPlacementAlignment::End)
            .tokens(tokens)
            .state(),
        },
        PopoverSample {
            id: "disabled",
            label: "Disabled",
            content_text: "Disabled triggers stay closed and unfocusable.",
            state: Popover::new(
                "overlay-popover:disabled",
                "Disabled",
                "Disabled triggers stay closed and unfocusable.",
            )
            .default_open(true)
            .disabled(true)
            .tokens(tokens)
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
