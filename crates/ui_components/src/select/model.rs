use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Size, ThemeTokens,
};

use crate::focus::FocusRing;
use crate::listbox::{
    ListboxGroupDescriptor, ListboxOptionDescriptor, ListboxSelection, ListboxState,
};
use crate::overlay::{OverlayDisclosureConfig, OverlayDisclosureOpenMode, OverlayResolvedState};
use crate::scroll_area::{ScrollAreaAxis, ScrollAreaState, ScrollResetPolicy};
use crate::theme::ThemeResolver;

use super::style::{SelectColors, SelectMetrics};

/// Select open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

const fn select_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> SelectOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => SelectOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => SelectOpenMode::Controlled,
    }
}

/// Selection payload emitted by a select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectSelection {
    index: usize,
    value: String,
    label: String,
}

impl SelectSelection {
    /// Creates a select selection payload.
    pub fn new(index: usize, value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            index,
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns the flattened option index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns selected option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected option label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl From<ListboxSelection> for SelectSelection {
    fn from(selection: ListboxSelection) -> Self {
        Self {
            index: selection.index(),
            value: selection.value().to_owned(),
            label: selection.label().to_owned(),
        }
    }
}

/// Resolved select state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: SelectOpenMode,
    label: String,
    placeholder: String,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    metrics: SelectMetrics,
    colors: SelectColors,
    focus_ring: FocusRing,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    overlay: OverlayResolvedState,
}

impl SelectState {
    /// Resolves public state for a select.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        groups: impl IntoIterator<Item = ListboxGroupDescriptor>,
        options: impl IntoIterator<Item = ListboxOptionDescriptor>,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let placeholder = placeholder.into();
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = select_open_mode_from_disclosure(disclosure.open_mode());
        let group_descriptors = groups.into_iter().collect::<Vec<_>>();
        let option_descriptors = options.into_iter().collect::<Vec<_>>();
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            selected_value,
            active_value,
            None,
            "No options",
            group_descriptors.clone(),
            option_descriptors.clone(),
            tokens,
        );
        let overlay = disclosure.overlay().clone();
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:select-content-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::select_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            label,
            placeholder,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            metrics: SelectMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            listbox,
            scroll_area,
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the popup is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> SelectOpenMode {
        self.open_mode
    }

    /// Returns accessible select label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns visible trigger label.
    pub fn trigger_label(&self) -> &str {
        self.listbox
            .selected_option()
            .filter(|option| option.focusable())
            .map(|option| option.label())
            .unwrap_or(self.placeholder.as_str())
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.listbox.selected_value()
    }

    /// Returns active option value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
    }

    /// Returns preferred placement side.
    pub const fn placement_side(&self) -> OverlayPlacementSide {
        self.placement_side
    }

    /// Returns preferred placement alignment.
    pub const fn placement_alignment(&self) -> OverlayPlacementAlignment {
        self.placement_alignment
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns initial focus intent.
    pub fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns trigger accessibility role.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns content accessibility role.
    pub const fn content_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns whether the trigger is visually selected.
    pub const fn trigger_selected(&self) -> bool {
        self.open
    }

    /// Returns whether content should use a scroll viewport.
    pub const fn scrollable_content(&self) -> bool {
        self.listbox.scrollable_content()
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SelectMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> SelectColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns nested scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}
