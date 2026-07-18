//! Overlay foundation page metadata.

use open_gpui::{point, px};
use std::time::Duration;

use crate::story::{StoryContract, StoryProbeContract, StoryProbeOperation::*};
use open_gpui_ui_components::component_contract::{
    ComponentContractMetadata, component_contract_entry,
};
use open_gpui_ui_components::{
    AlertDialog, AlertDialogIntent, AlertDialogState, ContextMenu, ContextMenuState, Dialog,
    DialogState, HoverCard, HoverCardDelayPolicy, HoverCardOpenIntent, HoverCardState, Menu,
    MenuItem, MenuState, Popover, PopoverState, Sheet, SheetCloseAffordance, SheetModalMode,
    SheetSide, SheetState, Tooltip, TooltipDelayPolicy, TooltipOpenIntent, TooltipState,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, OutsidePressPolicy, OverlayLayerKind, OverlayLayerPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, OverlayPresence, Rect, Sizable, Size,
    ThemeTokens, anchor_rect_from_point, outer_bounds_with_window_margin, prefer_visual_bounds,
    rect, ui_point, ui_px, ui_size,
};

/// Page title.
pub const TITLE: &str = "Overlay";
/// Page summary.
pub const SUMMARY: &str =
    "Anchor geometry, renderer-neutral policy, and per-window overlay runtime behavior.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_ui_foundation_gallery::pages::overlay::OVERLAY_CATALOG",
    "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogEntry",
    "open_gpui_ui_foundation_gallery::pages::overlay::OverlayCatalogStatus",
    "open_gpui_ui_foundation_gallery::pages::overlay::overlay_story_contracts",
    "open_gpui_ui_foundation_gallery::pages::overlay::overlay_sample_selector_pairs",
    "open_gpui_ui_foundation_gallery::StoryContract",
    "open_gpui_ui_foundation_gallery::StoryProbeContract",
    "open_gpui_ui_foundation_gallery::StoryProbeOperation",
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
    "open_gpui_ui_components::gpui_adapter::WindowOverlayRuntime",
    "open_gpui_ui_components::gpui_adapter::WindowOverlaySnapshot",
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
    contract: ComponentContractMetadata,
    /// Public overlay component name.
    pub name: &'static str,
    /// Current overlay catalog status.
    pub status: OverlayCatalogStatus,
    /// Canonical component product family.
    pub family: &'static str,
    /// Gallery-local behavior group used only for presentation.
    pub gallery_group: &'static str,
    /// Resolved state contract type.
    pub state: &'static str,
    /// Gallery and verification coverage note.
    pub coverage: &'static str,
    /// Stable rendered sample selector for this official overlay.
    pub sample_selector: &'static str,
    /// Stable selector used for the visible overlay catalog card.
    pub catalog_selector: &'static str,
}

impl OverlayCatalogEntry {
    /// Creates an official overlay catalog entry with a stable sample selector.
    pub const fn official(
        contract_id: &'static str,
        gallery_group: &'static str,
        state: &'static str,
        coverage: &'static str,
        sample_selector: &'static str,
        catalog_selector: &'static str,
    ) -> Self {
        let Some(contract) = component_contract_entry(contract_id) else {
            panic!("official Overlay Gallery entry requires a component contract row");
        };
        let metadata = contract.metadata();
        Self {
            contract: metadata,
            name: metadata.id().as_str(),
            status: OverlayCatalogStatus::Official,
            family: metadata.family().as_str(),
            gallery_group,
            state,
            coverage,
            sample_selector,
            catalog_selector,
        }
    }

    /// Returns the stable selector used for the visible overlay catalog card.
    pub const fn catalog_selector(self) -> &'static str {
        self.catalog_selector
    }

    /// Returns canonical product metadata for this overlay component.
    pub const fn component_contract(self) -> ComponentContractMetadata {
        self.contract
    }
}

/// Official overlay catalog entries and their Gallery-owned coverage summaries.
pub const OVERLAY_CATALOG: &[OverlayCatalogEntry] = &[
    OverlayCatalogEntry::official(
        "Tooltip",
        "descriptive",
        "TooltipState",
        "state samples / hover-focus smoke / manual-open smoke",
        "gallery:overlay-tooltip-sample:hover-focus",
        "overlay-catalog:Tooltip",
    ),
    OverlayCatalogEntry::official(
        "HoverCard",
        "interactive-hover",
        "HoverCardState",
        "state samples / trigger smoke / controlled-toggle smoke",
        "gallery:overlay-hover-card-sample:profile-preview",
        "overlay-catalog:HoverCard",
    ),
    OverlayCatalogEntry::official(
        "Popover",
        "non-modal",
        "PopoverState",
        "state samples / runtime layer / outside press / focus restore",
        "gallery:overlay-popover-sample:default-open",
        "overlay-catalog:Popover",
    ),
    OverlayCatalogEntry::official(
        "Dialog",
        "modal",
        "DialogState",
        "state samples / modal runtime / controlled refusal / focus restore",
        "gallery:overlay-dialog-sample:controlled-modal",
        "overlay-catalog:Dialog",
    ),
    OverlayCatalogEntry::official(
        "AlertDialog",
        "modal-action",
        "AlertDialogState",
        "state samples / action smoke / cancel focus",
        "gallery:overlay-alert-dialog-sample:destructive-confirm",
        "overlay-catalog:AlertDialog",
    ),
    OverlayCatalogEntry::official(
        "Sheet",
        "edge-modal",
        "SheetState",
        "state samples / non-modal outside-press smoke",
        "gallery:overlay-sheet-sample:left-modal",
        "overlay-catalog:Sheet",
    ),
    OverlayCatalogEntry::official(
        "Menu",
        "menu",
        "MenuState",
        "state samples / runtime layer / submenu parentage / focus restore",
        "gallery:overlay-menu-sample:default-open",
        "overlay-catalog:Menu",
    ),
    OverlayCatalogEntry::official(
        "ContextMenu",
        "point-menu",
        "ContextMenuState",
        "state samples / point-anchor smoke / escape smoke",
        "gallery:overlay-context-menu-sample:point-anchor",
        "overlay-catalog:ContextMenu",
    ),
];

/// Returns the official overlay entries that own rendered sample selectors.
pub fn overlay_sample_selector_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    OVERLAY_CATALOG
        .iter()
        .map(|entry| (entry.name, entry.sample_selector))
}

const OVERLAY_TRIGGER_OPEN_DISMISS_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "trigger", "opened surface"),
    StoryProbeContract::new(Dismiss, "outside or escape", "closed surface"),
    StoryProbeContract::new(Focus, "trigger", "focus restore"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved overlay policy"),
];

const OVERLAY_CONTROL_OPEN_DISMISS_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "control", "opened controlled surface"),
    StoryProbeContract::new(Dismiss, "escape or outside", "closed controlled surface"),
    StoryProbeContract::new(Focus, "trigger", "focus restore"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved overlay policy"),
];

const TOOLTIP_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "hover or focus trigger", "tooltip content"),
    StoryProbeContract::new(Dismiss, "focus or pointer leave", "closed tooltip content"),
    StoryProbeContract::new(Focus, "trigger", "focus-driven tooltip content"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved tooltip policy"),
];

const ALERT_DIALOG_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "trigger", "opened alert dialog surface"),
    StoryProbeContract::new(
        Dismiss,
        "primary action or escape",
        "closed alert dialog surface",
    ),
    StoryProbeContract::new(Activate, "primary action", "action dismissal"),
    StoryProbeContract::new(Focus, "cancel action", "initial focus and restore"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved alert dialog policy"),
];

const CONTEXT_MENU_STORY_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(Open, "right-click hotspot", "opened menu surface"),
    StoryProbeContract::new(Dismiss, "escape or outside", "closed menu surface"),
    StoryProbeContract::new(Focus, "hotspot", "context anchor focus"),
    StoryProbeContract::new(ReadPublicPayload, "state", "resolved context-menu policy"),
];

/// Returns overlay story contracts used by gallery runtime probes.
pub fn overlay_story_contracts() -> Vec<StoryContract> {
    OVERLAY_CATALOG
        .iter()
        .map(|entry| match entry.name {
            "Tooltip" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("gallery:overlay-tooltip-trigger:hover-focus"),
                Some("tooltip:overlay-tooltip-content:hover-focus:content"),
                None,
                TOOLTIP_STORY_PROBES,
            ),
            "HoverCard" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("hover-card:overlay-hover-card-demo:manual-controlled:trigger"),
                Some("hover-card:overlay-hover-card-demo:manual-controlled:content"),
                Some("gallery:overlay-hover-card-controlled-toggle"),
                OVERLAY_CONTROL_OPEN_DISMISS_PROBES,
            ),
            "Popover" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("popover:overlay-popover-demo:controlled:trigger"),
                Some("popover:overlay-popover-demo:controlled:content"),
                Some("gallery:overlay-popover-control:controlled"),
                OVERLAY_TRIGGER_OPEN_DISMISS_PROBES,
            ),
            "Dialog" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("dialog:overlay-dialog-demo:controlled-modal:trigger"),
                Some("dialog:overlay-dialog-demo:controlled-modal:surface"),
                Some("gallery:overlay-dialog-control:controlled-modal"),
                OVERLAY_TRIGGER_OPEN_DISMISS_PROBES,
            ),
            "AlertDialog" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("alert-dialog:overlay-alert-dialog-demo:destructive-confirm:trigger"),
                Some("alert-dialog:overlay-alert-dialog-demo:destructive-confirm:surface"),
                Some("gallery:overlay-alert-dialog-control:destructive-confirm"),
                ALERT_DIALOG_STORY_PROBES,
            ),
            "Sheet" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("gallery:overlay-sheet-control:right-non-modal"),
                Some("sheet:overlay-sheet-demo:right-non-modal:surface"),
                Some("gallery:overlay-sheet-control:right-non-modal"),
                OVERLAY_CONTROL_OPEN_DISMISS_PROBES,
            ),
            "Menu" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("menu:overlay-menu-demo:controlled:trigger"),
                Some("menu:overlay-menu-demo:controlled:content"),
                Some("gallery:overlay-menu-control:controlled"),
                OVERLAY_CONTROL_OPEN_DISMISS_PROBES,
            ),
            "ContextMenu" => StoryContract::overlay(
                entry.component_contract(),
                entry.state,
                entry.catalog_selector(),
                entry.sample_selector,
                Some("context-menu:overlay-context-menu-demo:controlled:hotspot"),
                Some("context-menu:overlay-context-menu-demo:controlled:surface"),
                Some("gallery:overlay-context-menu-control:controlled"),
                CONTEXT_MENU_STORY_PROBES,
            ),
            _ => unreachable!("official overlay catalog entry should have a story contract"),
        })
        .collect()
}

const OVERLAY_CONTROLLED_SAMPLE_COUNT: usize = 8;

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
    NestedDialog,
}

/// Mutable shell state for the overlay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct OverlayPageState {
    open: bool,
    hovered_tooltip_sample: Option<&'static str>,
    overlay_controlled_open: OverlayControlledOpenState,
    refuse_dialog_close: bool,
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
        OverlayControlledSample::NestedDialog,
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

    /// Returns whether the controlled Dialog sample refuses semantic close requests.
    pub(crate) fn refuses_dialog_close(self) -> bool {
        self.refuse_dialog_close
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

    /// Updates whether the controlled Dialog sample refuses close requests.
    pub(crate) fn set_refuse_dialog_close(&mut self, refuse: bool) -> bool {
        if self.refuse_dialog_close == refuse {
            return false;
        }

        self.refuse_dialog_close = refuse;
        true
    }

    /// Clears page-local state when navigating away.
    pub(crate) fn reset_on_page_change(&mut self) -> bool {
        let mut changed = false;
        if self.hovered_tooltip_sample.take().is_some() {
            changed = true;
        }
        changed |= self.overlay_controlled_open.reset();
        if self.refuse_dialog_close {
            self.refuse_dialog_close = false;
            changed = true;
        }
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
            .escape_key_policy(EscapeKeyPolicy::Dismiss)
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
pub fn menu_samples(tokens: ThemeTokens) -> Vec<MenuSample> {
    vec![
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
        {
            let items = vec![
                MenuItem::checkbox("show-hidden", "Show hidden files", true),
                MenuItem::radio("density-compact", "Compact density", false),
                MenuItem::radio("density-comfortable", "Comfortable density", true),
                MenuItem::submenu(
                    "sort",
                    "Sort by",
                    [
                        MenuItem::action("name", "Name"),
                        MenuItem::action("modified", "Modified"),
                    ],
                ),
                MenuItem::submenu(
                    "group",
                    "Group by",
                    [
                        MenuItem::action("kind", "Kind"),
                        MenuItem::action("owner", "Owner"),
                    ],
                ),
                MenuItem::submenu("empty", "Empty submenu", []),
            ];
            MenuSample {
                id: "rich-items",
                label: "Rich items",
                focused_value: Some("show-hidden"),
                state: Menu::new("overlay-menu:rich-items", "Rich items")
                    .default_open(true)
                    .default_focused_value("show-hidden")
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = vec![
                MenuItem::action("alpha", "Alpha"),
                MenuItem::action("beta", "Beta"),
                MenuItem::action("bravo", "Bravo"),
                MenuItem::action("charlie", "Charlie"),
            ];
            MenuSample {
                id: "typeahead",
                label: "Typeahead",
                focused_value: Some("beta"),
                state: Menu::new("overlay-menu:typeahead", "Typeahead")
                    .default_open(true)
                    .default_focused_value("beta")
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = (0..12).map(|index| {
                MenuItem::action(format!("action-{index:02}"), format!("Action {index:02}"))
            });
            MenuSample {
                id: "long-scroll",
                label: "Long scroll",
                focused_value: Some("action-00"),
                state: Menu::new("overlay-menu:long-scroll", "Long scroll")
                    .default_open(true)
                    .default_focused_value("action-00")
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
        assert!(!state.refuses_dialog_close());
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
        assert!(state.set_controlled_open(OverlayControlledSample::NestedDialog, true));
        assert!(state.is_controlled_open(OverlayControlledSample::NestedDialog));
        assert!(state.set_refuse_dialog_close(true));
        assert!(state.refuses_dialog_close());
        assert!(!state.set_refuse_dialog_close(true));
        assert!(state.set_overlay_open(false));
        assert!(state.reset_on_page_change());
        assert!(!state.overlay_open());
        assert_eq!(state.hovered_tooltip_sample(), None);
        assert!(!state.refuses_dialog_close());
        assert!(!state.is_controlled_open(OverlayControlledSample::HoverCard));
        assert!(!state.is_controlled_open(OverlayControlledSample::NestedDialog));
        assert!(!state.reset_on_page_change());
    }
}

/// Returns deterministic context-menu samples for gallery dogfood.
pub fn context_menu_samples(tokens: ThemeTokens) -> Vec<ContextMenuSample> {
    vec![
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
        {
            let items = vec![
                MenuItem::checkbox("snap-grid", "Snap to grid", true),
                MenuItem::radio("view-icons", "Icon view", false),
                MenuItem::radio("view-list", "List view", true),
                MenuItem::submenu(
                    "arrange",
                    "Arrange by",
                    [
                        MenuItem::action("name", "Name"),
                        MenuItem::action("type", "Type"),
                    ],
                ),
            ];
            ContextMenuSample {
                id: "rich-items",
                label: "Rich context",
                focused_value: Some("snap-grid"),
                state: ContextMenu::new("overlay-context-menu:rich-items", "Rich context area")
                    .default_open(true)
                    .anchor_point(point(px(620.0), px(340.0)))
                    .default_focused_value("snap-grid")
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
        {
            let items = (0..12).map(|index| {
                MenuItem::action(format!("action-{index:02}"), format!("Action {index:02}"))
            });
            ContextMenuSample {
                id: "edge-long",
                label: "Edge long",
                focused_value: Some("action-00"),
                state: ContextMenu::new("overlay-context-menu:edge-long", "Edge long area")
                    .default_open(true)
                    .anchor_point(point(px(960.0), px(560.0)))
                    .default_focused_value("action-00")
                    .items(items)
                    .tokens(tokens)
                    .state(),
            }
        },
    ]
}
