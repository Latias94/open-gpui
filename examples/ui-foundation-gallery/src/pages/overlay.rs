//! Overlay foundation page metadata.

use open_gpui::{point, px};
use std::time::Duration;

use open_gpui_ui_components::{
    AlertDialog, AlertDialogIntent, AlertDialogState, ContextMenu, ContextMenuState, Dialog,
    DialogState, HoverCard, HoverCardDelayPolicy, HoverCardOpenIntent, HoverCardState, Menu,
    MenuItem, MenuState, Popover, PopoverState, Sheet, SheetCloseAffordance, SheetModalMode,
    SheetSide, SheetState, Tooltip, TooltipDelayPolicy, TooltipOpenIntent, TooltipState,
};
use open_gpui_ui_core::{
    OutsidePressPolicy, OverlayLayerKind, OverlayLayerPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, OverlayPresence, Rect, Sizable, Size, ThemeTokens,
    anchor_rect_from_point, outer_bounds_with_window_margin, prefer_visual_bounds, rect, ui_point,
    ui_px, ui_size,
};

/// Page title.
pub const TITLE: &str = "Overlay";
/// Page summary.
pub const SUMMARY: &str =
    "Anchor geometry plus renderer-neutral overlay presence, dismissal, and focus policy.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_foundation_gallery::pages::overlay::OVERLAY_CATALOG",
    "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogEntry",
    "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogStatus",
    "open_gpui_ui_foundation_gallery::pages::overlay::overlay_sample_selector_pairs",
    "open_gpui_ui_components::Tooltip",
    "open_gpui_ui_components::TooltipState",
    "open_gpui_ui_components::HoverCard",
    "open_gpui_ui_components::HoverCardState",
    "open_gpui_ui_components::Popover",
    "open_gpui_ui_components::PopoverState",
    "open_gpui_ui_components::Dialog",
    "open_gpui_ui_components::DialogState",
    "open_gpui_ui_components::AlertDialog",
    "open_gpui_ui_components::AlertDialogState",
    "open_gpui_ui_components::Sheet",
    "open_gpui_ui_components::SheetState",
    "open_gpui_ui_components::Menu",
    "open_gpui_ui_components::MenuState",
    "open_gpui_ui_components::ContextMenu",
    "open_gpui_ui_components::ContextMenuState",
    "anchor_rect_from_point()",
    "prefer_visual_bounds()",
    "outer_bounds_with_window_margin()",
    "OverlayLayerPolicy",
    "OverlayPresence",
    "OutsidePressPolicy",
    "FocusRestoreIntent",
    "TooltipState",
    "HoverCardState",
    "PopoverState",
    "DialogState",
    "AlertDialogState",
    "SheetState",
    "MenuState",
    "ContextMenuState",
    "OverlayEdges",
    "OverlaySize",
];

/// Overlay catalog status shown by the Overlay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayCatalogStatus {
    /// Official overlay component with resolved state, gallery sample, and runtime smoke coverage.
    Official,
}

impl OverlayCatalogStatus {
    /// Stable status label used by tests and the gallery.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
        }
    }

    /// Pill colors used by the gallery to render the catalog status badge.
    pub const fn badge_colors(self) -> (u32, u32, u32) {
        match self {
            Self::Official => (0xe8f3ef, 0x9ccdbd, 0x1f5f4d),
        }
    }
}

/// One overlay catalog entry shown by the Overlay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayCatalogEntry {
    /// Public overlay component name.
    pub name: &'static str,
    /// Current overlay catalog status.
    pub status: OverlayCatalogStatus,
    /// Overlay family or behavior area.
    pub family: &'static str,
    /// Resolved state contract type.
    pub state: &'static str,
    /// Gallery and verification coverage note.
    pub coverage: &'static str,
    /// Stable rendered sample selector for this official overlay.
    pub sample_selector: &'static str,
    /// Stable selector used for the visible overlay catalog card.
    pub catalog_selector: &'static str,
    /// Focused tests or gates that protect this overlay family.
    pub behavior_gates: &'static [&'static str],
}

impl OverlayCatalogEntry {
    /// Creates an official overlay catalog entry with a stable sample selector.
    pub const fn official(
        name: &'static str,
        family: &'static str,
        state: &'static str,
        coverage: &'static str,
        sample_selector: &'static str,
        catalog_selector: &'static str,
        behavior_gates: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            status: OverlayCatalogStatus::Official,
            family,
            state,
            coverage,
            sample_selector,
            catalog_selector,
            behavior_gates,
        }
    }

    /// Returns the stable selector used for the visible overlay catalog card.
    pub const fn catalog_selector(self) -> &'static str {
        self.catalog_selector
    }
}

/// Official overlay catalog entries and their conformance gates.
pub const OVERLAY_CATALOG: &[OverlayCatalogEntry] = &[
    OverlayCatalogEntry::official(
        "Tooltip",
        "descriptive",
        "TooltipState",
        "state samples / hover-focus smoke / manual-open smoke",
        "gallery:overlay-tooltip-sample:hover-focus",
        "overlay-catalog:Tooltip",
        &[
            "overlay_page_tooltip_samples_expose_focus_hover_and_disabled_contracts",
            "overlay_gallery_smoke_opens_tooltip_from_hover_focus_and_ignores_disabled",
            "overlay_gallery_smoke_renders_manual_tooltip_from_state",
        ],
    ),
    OverlayCatalogEntry::official(
        "HoverCard",
        "interactive-hover",
        "HoverCardState",
        "state samples / trigger smoke / controlled-toggle smoke",
        "gallery:overlay-hover-card-sample:profile-preview",
        "overlay-catalog:HoverCard",
        &[
            "overlay_page_hover_card_samples_expose_interactive_hover_contracts",
            "overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses",
            "overlay_gallery_smoke_toggles_hover_card_from_control_surface",
        ],
    ),
    OverlayCatalogEntry::official(
        "Popover",
        "non-modal",
        "PopoverState",
        "state samples / outside-press smoke / focus restore",
        "gallery:overlay-popover-sample:default-open",
        "overlay-catalog:Popover",
        &[
            "overlay_page_popover_samples_expose_controlled_and_dismissal_contracts",
            "overlay_gallery_smoke_dismisses_popover_from_outside_press",
        ],
    ),
    OverlayCatalogEntry::official(
        "Dialog",
        "modal",
        "DialogState",
        "state samples / barrier smoke / escape smoke",
        "gallery:overlay-dialog-sample:controlled-modal",
        "overlay-catalog:Dialog",
        &[
            "overlay_page_dialog_samples_expose_modal_and_close_contracts",
            "overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape",
        ],
    ),
    OverlayCatalogEntry::official(
        "AlertDialog",
        "modal-action",
        "AlertDialogState",
        "state samples / action smoke / cancel focus",
        "gallery:overlay-alert-dialog-sample:destructive-confirm",
        "overlay-catalog:AlertDialog",
        &[
            "overlay_page_alert_dialog_samples_expose_critical_action_contracts",
            "overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape",
        ],
    ),
    OverlayCatalogEntry::official(
        "Sheet",
        "edge-modal",
        "SheetState",
        "state samples / non-modal outside-press smoke",
        "gallery:overlay-sheet-sample:left-modal",
        "overlay-catalog:Sheet",
        &[
            "overlay_page_sheet_samples_expose_edge_and_policy_contracts",
            "overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press",
        ],
    ),
    OverlayCatalogEntry::official(
        "Menu",
        "menu",
        "MenuState",
        "state samples / roving-focus smoke / outside-press smoke",
        "gallery:overlay-menu-sample:default-open",
        "overlay-catalog:Menu",
        &[
            "overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts",
            "overlay_gallery_smoke_closes_menu_from_escape_and_outside_press",
        ],
    ),
    OverlayCatalogEntry::official(
        "ContextMenu",
        "point-menu",
        "ContextMenuState",
        "state samples / point-anchor smoke / escape smoke",
        "gallery:overlay-context-menu-sample:point-anchor",
        "overlay-catalog:ContextMenu",
        &[
            "overlay_page_context_menu_samples_expose_point_anchor_contracts",
            "overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses",
        ],
    ),
];

/// Returns the official overlay entries that own rendered sample selectors.
pub fn overlay_sample_selector_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    OVERLAY_CATALOG
        .iter()
        .map(|entry| (entry.name, entry.sample_selector))
}

const OVERLAY_CONTROLLED_SAMPLE_COUNT: usize = 7;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayControlledSample {
    HoverCard,
    Popover,
    Dialog,
    AlertDialog,
    Sheet,
    Menu,
    ContextMenu,
}

/// Mutable shell state for the overlay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct OverlayPageState {
    open: bool,
    hovered_tooltip_sample: Option<&'static str>,
    overlay_controlled_open: OverlayControlledOpenState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct OverlayControlledOpenState {
    open: [bool; OVERLAY_CONTROLLED_SAMPLE_COUNT],
}

impl OverlayControlledOpenState {
    #[cfg(test)]
    const ALL: [OverlayControlledSample; OVERLAY_CONTROLLED_SAMPLE_COUNT] = [
        OverlayControlledSample::HoverCard,
        OverlayControlledSample::Popover,
        OverlayControlledSample::Dialog,
        OverlayControlledSample::AlertDialog,
        OverlayControlledSample::Sheet,
        OverlayControlledSample::Menu,
        OverlayControlledSample::ContextMenu,
    ];

    const fn is_open(self, sample: OverlayControlledSample) -> bool {
        self.open[sample as usize]
    }

    fn set_open(&mut self, sample: OverlayControlledSample, open: bool) -> bool {
        let index = sample as usize;
        let current = self.open[index];
        if current == open {
            return false;
        }

        self.open[index] = open;
        true
    }

    fn reset(&mut self) -> bool {
        if self.open.iter().all(|open| !*open) {
            return false;
        }

        self.open = [false; OVERLAY_CONTROLLED_SAMPLE_COUNT];
        true
    }
}

impl OverlayPageState {
    /// Returns whether the demo overlay is open.
    pub(crate) fn overlay_open(self) -> bool {
        self.open
    }

    /// Returns the currently hovered tooltip sample id.
    pub(crate) fn hovered_tooltip_sample(self) -> Option<&'static str> {
        self.hovered_tooltip_sample
    }

    /// Returns whether a controlled sample overlay is open.
    pub(crate) fn is_controlled_open(self, sample: OverlayControlledSample) -> bool {
        self.overlay_controlled_open.is_open(sample)
    }

    /// Opens or closes the demo overlay and returns whether the state changed.
    pub(crate) fn set_overlay_open(&mut self, open: bool) -> bool {
        if self.open == open {
            return false;
        }

        self.open = open;
        true
    }

    /// Updates the hovered tooltip sample and returns whether the state changed.
    pub(crate) fn set_hovered_tooltip_sample(&mut self, sample: Option<&'static str>) -> bool {
        if self.hovered_tooltip_sample == sample {
            return false;
        }

        self.hovered_tooltip_sample = sample;
        true
    }

    /// Updates a controlled sample open flag and returns whether the state changed.
    pub(crate) fn set_controlled_open(
        &mut self,
        sample: OverlayControlledSample,
        open: bool,
    ) -> bool {
        self.overlay_controlled_open.set_open(sample, open)
    }

    /// Clears the controlled sample open flags and returns whether the state changed.
    pub(crate) fn close_controlled_overlays(&mut self) -> bool {
        self.overlay_controlled_open.reset()
    }

    /// Clears page-local state when navigating away.
    pub(crate) fn reset_on_page_change(&mut self) -> bool {
        let mut changed = false;
        if self.hovered_tooltip_sample.take().is_some() {
            changed = true;
        }
        changed |= self.overlay_controlled_open.reset();
        changed
    }
}

/// Geometry used by the overlay demo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDemoGeometry {
    /// Point selected as the trigger anchor.
    pub trigger_point: open_gpui_ui_core::UiPoint,
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
    let trigger_point = ui_point(
        ui_px(trigger_point.x.as_f32()),
        ui_px(trigger_point.y.as_f32()),
    );
    let anchor_rect = anchor_rect_from_point(trigger_point);
    let layout_rect = rect(
        ui_point(ui_px(288.0), ui_px(144.0)),
        ui_size(ui_px(176.0), ui_px(40.0)),
    );
    let visual_rect = rect(
        ui_point(ui_px(284.0), ui_px(140.0)),
        ui_size(ui_px(184.0), ui_px(48.0)),
    );
    let preferred_rect = prefer_visual_bounds(Some(visual_rect), Some(layout_rect))
        .expect("visual or layout rect should be present");
    let safe_window_rect = outer_bounds_with_window_margin(
        rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(640.0), ui_px(360.0)),
        ),
        ui_px(12.0),
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
}

/// Returns deterministic overlay behavior samples for the gallery.
pub fn behavior_samples() -> [OverlayBehaviorSample; 4] {
    [
        OverlayBehaviorSample {
            id: "tooltip",
            label: "Tooltip",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
        },
        OverlayBehaviorSample {
            id: "popover",
            label: "Popover",
            policy: OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        },
        OverlayBehaviorSample {
            id: "dialog",
            label: "Dialog",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open()),
        },
        OverlayBehaviorSample {
            id: "menu",
            label: "Menu",
            policy: OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
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

impl TooltipSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-tooltip-sample:{}", self.id)
    }
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

/// Hover card sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverCardSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Text shown by the hover card surface.
    pub content_text: &'static str,
    /// Resolved hover card state.
    pub state: HoverCardState,
}

impl HoverCardSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-hover-card-sample:{}", self.id)
    }
}

/// Returns deterministic hover card samples for gallery dogfood.
pub fn hover_card_samples(tokens: ThemeTokens) -> [HoverCardSample; 3] {
    [
        HoverCardSample {
            id: "profile-preview",
            label: "Profile preview",
            content_text: "Interactive hover card opened from pointer hover or keyboard focus.",
            state: HoverCard::new(
                "overlay-hover-card:profile-preview",
                "Profile preview",
                "Interactive hover card opened from pointer hover or keyboard focus.",
            )
            .default_open(true)
            .placement_side(OverlayPlacementSide::Bottom)
            .placement_alignment(OverlayPlacementAlignment::Center)
            .tokens(tokens)
            .state(),
        },
        HoverCardSample {
            id: "focus-preview",
            label: "Focus preview",
            content_text: "Focus-only hover card keeps pointer hover from opening it.",
            state: HoverCard::new(
                "overlay-hover-card:focus-preview",
                "Focus preview",
                "Focus-only hover card keeps pointer hover from opening it.",
            )
            .open_intent(HoverCardOpenIntent::Focus)
            .placement_side(OverlayPlacementSide::Right)
            .tokens(tokens)
            .state(),
        },
        HoverCardSample {
            id: "manual-controlled",
            label: "Manual card",
            content_text: "The gallery shell owns this manual hover card open state.",
            state: HoverCard::new(
                "overlay-hover-card:manual-controlled",
                "Manual card",
                "The gallery shell owns this manual hover card open state.",
            )
            .open(false)
            .open_intent(HoverCardOpenIntent::Manual)
            .delay(HoverCardDelayPolicy::new(
                Duration::from_millis(80),
                Duration::from_millis(20),
            ))
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .placement_side(OverlayPlacementSide::Top)
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

impl PopoverSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-popover-sample:{}", self.id)
    }
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
            .default_open(true)
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

/// Dialog sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Text shown by the dialog surface.
    pub content_text: &'static str,
    /// Resolved dialog state.
    pub state: DialogState,
}

impl DialogSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-dialog-sample:{}", self.id)
    }
}

/// Returns deterministic dialog samples for gallery dogfood.
pub fn dialog_samples(tokens: ThemeTokens) -> [DialogSample; 4] {
    [
        DialogSample {
            id: "controlled-modal",
            label: "Controlled modal",
            content_text: "The gallery shell owns this modal open state.",
            state: Dialog::new(
                "overlay-dialog:controlled-modal",
                "Controlled modal",
                "Controlled dialog",
                "The gallery shell owns this modal open state.",
            )
            .description("Escape and the modal barrier can close it.")
            .open(false)
            .outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .tokens(tokens)
            .state(),
        },
        DialogSample {
            id: "default-open",
            label: "Default open",
            content_text: "Uncontrolled modal initialized open.",
            state: Dialog::new(
                "overlay-dialog:default-open",
                "Default open",
                "Default open dialog",
                "Uncontrolled modal initialized open.",
            )
            .default_open(true)
            .tokens(tokens)
            .state(),
        },
        DialogSample {
            id: "outside-ignore",
            label: "Outside ignored",
            content_text: "Outside press does not dismiss this dialog.",
            state: Dialog::new(
                "overlay-dialog:outside-ignore",
                "Outside ignored",
                "Sticky dialog",
                "Outside press does not dismiss this dialog.",
            )
            .default_open(true)
            .outside_press_policy(OutsidePressPolicy::Ignore)
            .tokens(tokens)
            .state(),
        },
        DialogSample {
            id: "disabled",
            label: "Disabled",
            content_text: "Disabled triggers stay closed and unfocusable.",
            state: Dialog::new(
                "overlay-dialog:disabled",
                "Disabled",
                "Disabled dialog",
                "Disabled triggers stay closed and unfocusable.",
            )
            .default_open(true)
            .disabled(true)
            .tokens(tokens)
            .state(),
        },
    ]
}

/// Alert dialog sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertDialogSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Resolved alert dialog state.
    pub state: AlertDialogState,
}

impl AlertDialogSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-alert-dialog-sample:{}", self.id)
    }
}

/// Returns deterministic alert dialog samples for gallery dogfood.
pub fn alert_dialog_samples(tokens: ThemeTokens) -> [AlertDialogSample; 2] {
    [
        AlertDialogSample {
            id: "destructive-confirm",
            label: "Delete project",
            state: AlertDialog::new(
                "overlay-alert-dialog:destructive-confirm",
                "Delete project",
                "Delete this project?",
                "This permanently removes project data and cannot be undone.",
                "Delete",
            )
            .cancel_label("Keep project")
            .intent(AlertDialogIntent::Destructive)
            .open(false)
            .tokens(tokens)
            .state(),
        },
        AlertDialogSample {
            id: "safe-cancel",
            label: "Archive item",
            state: AlertDialog::new(
                "overlay-alert-dialog:safe-cancel",
                "Archive item",
                "Archive this item?",
                "The item moves out of the active list and can be restored later.",
                "Archive",
            )
            .cancel_label("Cancel")
            .default_open(true)
            .tokens(tokens)
            .state(),
        },
    ]
}

/// Sheet sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Text shown by the sheet body.
    pub content_text: &'static str,
    /// Resolved sheet state.
    pub state: SheetState,
}

impl SheetSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-sheet-sample:{}", self.id)
    }
}

/// Returns deterministic sheet samples for gallery dogfood.
pub fn sheet_samples(tokens: ThemeTokens) -> [SheetSample; 3] {
    [
        SheetSample {
            id: "left-modal",
            label: "Left sheet",
            content_text: "Modal left sheet with an explicit close affordance.",
            state: Sheet::new(
                "overlay-sheet:left-modal",
                "Left sheet",
                "Workspace filters",
                "Modal left sheet with an explicit close affordance.",
            )
            .description("Filter active work without leaving the page.")
            .default_open(true)
            .side(SheetSide::Left)
            .tokens(tokens)
            .state(),
        },
        SheetSample {
            id: "right-non-modal",
            label: "Right sheet",
            content_text: "Non-modal right sheet keeps underlay dispatch explicit.",
            state: Sheet::new(
                "overlay-sheet:right-non-modal",
                "Right sheet",
                "Inspector",
                "Non-modal right sheet keeps underlay dispatch explicit.",
            )
            .description("Outside press dismisses while allowing underlay dispatch.")
            .open(false)
            .side(SheetSide::Right)
            .modal_mode(SheetModalMode::NonModal)
            .tokens(tokens)
            .state(),
        },
        SheetSample {
            id: "bottom-sticky",
            label: "Bottom sheet",
            content_text: "Bottom sheet ignores outside press and hides the close control.",
            state: Sheet::new(
                "overlay-sheet:bottom-sticky",
                "Bottom sheet",
                "Queue details",
                "Bottom sheet ignores outside press and hides the close control.",
            )
            .default_open(true)
            .side(SheetSide::Bottom)
            .close_affordance(SheetCloseAffordance::Hidden)
            .outside_press_policy(OutsidePressPolicy::Ignore)
            .tokens(tokens)
            .state(),
        },
    ]
}

/// Menu sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing trigger label.
    pub label: &'static str,
    /// Requested initial focused item value.
    pub focused_value: Option<&'static str>,
    /// Resolved menu state.
    pub state: MenuState,
}

impl MenuSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-menu-sample:{}", self.id)
    }
}

/// Returns deterministic menu samples for gallery dogfood.
pub fn menu_samples(tokens: ThemeTokens) -> [MenuSample; 4] {
    [
        {
            let items = vec![
                MenuItem::action("new", "New"),
                MenuItem::action("save", "Save"),
                MenuItem::separator("separator"),
                MenuItem::action("delete", "Delete").disabled(true),
            ];
            let focused_value = "save";
            MenuSample {
                id: "default-open",
                label: "Default open",
                focused_value: Some("save"),
                state: Menu::new("overlay-menu:default-open", "Default open")
                    .default_open(true)
                    .default_focused_value(focused_value)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![
                MenuItem::action("cut", "Cut"),
                MenuItem::action("copy", "Copy"),
                MenuItem::action("paste", "Paste").disabled(true),
            ];
            let focused_value = "copy";
            MenuSample {
                id: "controlled",
                label: "Controlled",
                focused_value: Some("copy"),
                state: Menu::new("overlay-menu:controlled", "Controlled")
                    .open(false)
                    .default_focused_value(focused_value)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![
                MenuItem::action("rename", "Rename"),
                MenuItem::action("duplicate", "Duplicate"),
            ];
            MenuSample {
                id: "outside-ignore",
                label: "Outside ignored",
                focused_value: None,
                state: Menu::new("overlay-menu:outside-ignore", "Outside ignored")
                    .default_open(true)
                    .outside_press_policy(OutsidePressPolicy::Ignore)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![MenuItem::action("open", "Open")];
            MenuSample {
                id: "disabled",
                label: "Disabled",
                focused_value: None,
                state: Menu::new("overlay-menu:disabled", "Disabled")
                    .default_open(true)
                    .disabled(true)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
    ]
}

/// Context menu sample shown by the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuSample {
    /// Stable sample id.
    pub id: &'static str,
    /// User-facing hotspot label.
    pub label: &'static str,
    /// Requested initial focused item value.
    pub focused_value: Option<&'static str>,
    /// Resolved context-menu state.
    pub state: ContextMenuState,
}

impl ContextMenuSample {
    /// Returns the stable debug selector used by the gallery shell and tests.
    pub fn debug_selector(&self) -> String {
        format!("gallery:overlay-context-menu-sample:{}", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_page_state_tracks_open_hover_and_controlled_flags() {
        let mut state = OverlayPageState::default();

        assert!(!state.overlay_open());
        assert_eq!(state.hovered_tooltip_sample(), None);
        assert!(!state.is_controlled_open(OverlayControlledSample::HoverCard));
        for sample in OverlayControlledOpenState::ALL {
            assert!(!state.is_controlled_open(sample));
        }
        assert!(state.set_overlay_open(true));
        assert!(state.overlay_open());
        assert!(!state.set_hovered_tooltip_sample(None));
        assert!(state.set_hovered_tooltip_sample(Some("tooltip")));
        assert_eq!(state.hovered_tooltip_sample(), Some("tooltip"));
        assert!(state.set_controlled_open(OverlayControlledSample::HoverCard, true));
        assert!(state.is_controlled_open(OverlayControlledSample::HoverCard));
        assert!(!state.is_controlled_open(OverlayControlledSample::Popover));
        assert!(!state.set_controlled_open(OverlayControlledSample::HoverCard, true));
        assert!(state.close_controlled_overlays());
        assert!(!state.is_controlled_open(OverlayControlledSample::HoverCard));
        assert!(state.set_overlay_open(false));
        assert!(state.reset_on_page_change());
        assert!(!state.overlay_open());
        assert_eq!(state.hovered_tooltip_sample(), None);
        assert!(!state.close_controlled_overlays());
        assert!(!state.reset_on_page_change());
    }
}

/// Returns deterministic context-menu samples for gallery dogfood.
pub fn context_menu_samples(tokens: ThemeTokens) -> [ContextMenuSample; 3] {
    [
        {
            let items = vec![
                MenuItem::action("duplicate", "Duplicate"),
                MenuItem::separator("separator"),
                MenuItem::action("delete", "Delete").disabled(true),
            ];
            let focused_value = "duplicate";
            ContextMenuSample {
                id: "point-anchor",
                label: "Point anchor",
                focused_value: Some("duplicate"),
                state: ContextMenu::new("overlay-context-menu:point-anchor", "Right click area")
                    .default_open(true)
                    .anchor_point(point(px(520.0), px(300.0)))
                    .default_focused_value(focused_value)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![
                MenuItem::action("inspect", "Inspect"),
                MenuItem::action("copy-link", "Copy link"),
            ];
            let focused_value = "inspect";
            ContextMenuSample {
                id: "controlled",
                label: "Controlled",
                focused_value: Some("inspect"),
                state: ContextMenu::new("overlay-context-menu:controlled", "Controlled area")
                    .open(false)
                    .anchor_point(point(px(280.0), px(160.0)))
                    .default_focused_value(focused_value)
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![
                MenuItem::action("open", "Open"),
                MenuItem::action("close", "Close"),
            ];
            ContextMenuSample {
                id: "default-open",
                label: "Default open",
                focused_value: None,
                state: ContextMenu::new("overlay-context-menu:default-open", "Default area")
                    .default_open(true)
                    .anchor_point(point(px(96.0), px(96.0)))
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
    ]
}
