//! Sidebar component.

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{App, ElementId, IntoElement, SharedString, Window};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::action::{ActionIconDescriptor, ResolvedActionIcon, ResolvedActionState};
use crate::activation::{Activation, ActivationHandle};
use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};
use crate::color::{ColorIntent, ColorState};
use crate::focus::FocusRing;

mod render;

const DEFAULT_SURFACE: u32 = 0xffffff;
const DEFAULT_FLOATING_SURFACE: u32 = 0xf8faf6;
const DEFAULT_BORDER: u32 = 0xcfd5cc;
const DEFAULT_TEXT: u32 = 0x18202a;
const DEFAULT_TEXT_MUTED: u32 = 0x5a6472;
const DEFAULT_SELECTED_SURFACE: u32 = 0xe8f3ef;
const DEFAULT_HOVER_SURFACE: u32 = 0xf1f5ee;
const DEFAULT_ACCENT: u32 = 0x1f7a66;
const DEFAULT_ACCENT_FOREGROUND: u32 = 0xffffff;
const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;

/// Physical side where a sidebar is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarSide {
    /// Attach the sidebar to the left edge.
    #[default]
    Left,
    /// Attach the sidebar to the right edge.
    Right,
}

impl SidebarSide {
    /// Returns the stable side label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// Visual shell treatment for a sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarVariant {
    /// Docked edge rail with a single separating border.
    #[default]
    Docked,
    /// Floating panel with a full border and rounded corners.
    Floating,
    /// Inset panel intended to sit inside a surrounding shell.
    Inset,
}

impl SidebarVariant {
    /// Returns the stable variant label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Docked => "docked",
            Self::Floating => "floating",
            Self::Inset => "inset",
        }
    }
}

/// Collapse behavior for a sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarCollapseMode {
    /// Collapse to an icon rail while keeping navigation items reachable.
    #[default]
    Icon,
    /// Collapse out of layout and remove navigation items from keyboard focus.
    Offcanvas,
    /// Ignore collapsed state and always render expanded content.
    None,
}

impl SidebarCollapseMode {
    /// Returns the stable collapse mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Icon => "icon",
            Self::Offcanvas => "offcanvas",
            Self::None => "none",
        }
    }
}

/// Pure descriptor for one sidebar navigation item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItemDescriptor {
    value: String,
    label: String,
    icon: Option<ResolvedActionIcon>,
    badge: Option<String>,
    action_label: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    shortcut: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
}

impl SidebarItemDescriptor {
    /// Creates an enabled sidebar item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            badge: None,
            action_label: None,
            disabled: false,
            disabled_reason: None,
            shortcut: None,
            tooltip: None,
            accessibility_description: None,
        }
    }

    /// Creates a sidebar item descriptor from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        let mut item = Self::new(action.value(), action.label()).disabled(action.disabled());
        if let Some(icon) = action.icon() {
            item.icon = Some(icon.clone());
        }
        if let Some(shortcut) = action.shortcut() {
            item = item.shortcut(shortcut);
        }
        if let Some(reason) = action.disabled_reason() {
            item = item.disabled_reason(reason);
        }
        if let Some(tooltip) = action.tooltip() {
            item = item.tooltip(tooltip);
        }
        if let Some(description) = action.accessibility_description() {
            item = item.accessibility_description(description);
        }
        item
    }

    /// Applies an icon glyph or symbolic icon label.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        let icon = icon.into();
        self.icon = Some(ResolvedActionIcon::resolved(
            ActionIconDescriptor::new(icon.clone()).fallback_label(icon.clone()),
            icon,
        ));
        self
    }

    /// Applies app-resolved icon metadata.
    pub fn resolved_icon(mut self, icon: ResolvedActionIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Applies display-only badge text.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Applies a display-only trailing action label.
    pub fn action_label(mut self, action_label: impl Into<String>) -> Self {
        self.action_label = Some(action_label.into());
        self
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        if !disabled {
            self.disabled_reason = None;
        }
        self
    }

    /// Marks the item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        if !reason.is_empty() {
            self.disabled = true;
            self.disabled_reason = Some(reason);
        }
        self
    }

    /// Applies a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        let tooltip = tooltip.into();
        if !tooltip.is_empty() {
            self.tooltip = Some(tooltip);
        }
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        if !description.is_empty() {
            self.accessibility_description = Some(description);
        }
        self
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible or accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns app-resolved icon metadata.
    pub const fn icon_ref(&self) -> Option<&ResolvedActionIcon> {
        self.icon.as_ref()
    }

    /// Returns the optional icon glyph or symbolic icon label.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns display-only badge text.
    pub fn badge_label(&self) -> Option<&str> {
        self.badge.as_deref()
    }

    /// Returns display-only trailing action label.
    pub fn action_label_text(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns user-displayable tooltip metadata.
    pub fn tooltip_ref(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }

    /// Returns the optional accessibility description.
    pub fn accessibility_description_ref(&self) -> Option<&str> {
        self.accessibility_description.as_deref()
    }
}

/// Pure descriptor for one sidebar section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSectionDescriptor {
    value: String,
    label: String,
    items: Vec<SidebarItemDescriptor>,
}

impl SidebarSectionDescriptor {
    /// Creates an empty sidebar section descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Adds one navigation item descriptor.
    pub fn item(mut self, item: impl Into<SidebarItemDescriptor>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Replaces section items.
    pub fn items(
        mut self,
        items: impl IntoIterator<Item = impl Into<SidebarItemDescriptor>>,
    ) -> Self {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    /// Returns the stable section value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible section label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns item descriptors in this section.
    pub fn item_descriptors(&self) -> &[SidebarItemDescriptor] {
        &self.items
    }
}

/// Resolved sidebar color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarColors {
    surface: ColorIntent,
    floating_surface: ColorIntent,
    foreground: ColorIntent,
    muted_foreground: ColorIntent,
    border: ColorIntent,
    item_background: ColorIntent,
    item_hover_background: ColorIntent,
    item_selected_background: ColorIntent,
    item_disabled_foreground: ColorIntent,
    badge_background: ColorIntent,
    badge_foreground: ColorIntent,
    focus_ring: ColorIntent,
}

impl SidebarColors {
    /// Resolves sidebar color intents from shared theme tokens.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            surface: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            floating_surface: ColorIntent::new(tokens.surface_muted, DEFAULT_FLOATING_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            item_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            item_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                DEFAULT_HOVER_SURFACE,
            ),
            item_selected_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                DEFAULT_SELECTED_SURFACE,
            ),
            item_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                DEFAULT_TEXT_MUTED,
            ),
            badge_background: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
            badge_foreground: ColorIntent::new(tokens.accent_foreground, DEFAULT_ACCENT_FOREGROUND),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    /// Returns the docked sidebar surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns the floating sidebar surface color intent.
    pub const fn floating_surface(self) -> ColorIntent {
        self.floating_surface
    }

    /// Returns the foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns the item background color intent.
    pub const fn item_background(self) -> ColorIntent {
        self.item_background
    }

    /// Returns the item hover background color intent.
    pub const fn item_hover_background(self) -> ColorIntent {
        self.item_hover_background
    }

    /// Returns the selected item background color intent.
    pub const fn item_selected_background(self) -> ColorIntent {
        self.item_selected_background
    }

    /// Returns the disabled item foreground color intent.
    pub const fn item_disabled_foreground(self) -> ColorIntent {
        self.item_disabled_foreground
    }

    /// Returns the badge background color intent.
    pub const fn badge_background(self) -> ColorIntent {
        self.badge_background
    }

    /// Returns the badge foreground color intent.
    pub const fn badge_foreground(self) -> ColorIntent {
        self.badge_foreground
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved sidebar metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarMetrics {
    expanded_width: UiPx,
    collapsed_width: UiPx,
    resolved_width: UiPx,
    padding: UiPx,
    section_gap: UiPx,
    item_gap: UiPx,
    item_height: UiPx,
    item_padding_x: UiPx,
    item_padding_y: UiPx,
    icon_size: UiPx,
    badge_min_height: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl SidebarMetrics {
    /// Resolves metrics from size and effective collapsed state.
    pub const fn from_size(
        size: Size,
        collapse_mode: SidebarCollapseMode,
        collapsed: bool,
    ) -> Self {
        let expanded_width = match size {
            Size::XSmall => ui_px(216.0),
            Size::Small => ui_px(232.0),
            Size::Medium => ui_px(248.0),
            Size::Large => ui_px(272.0),
        };
        let collapsed_width = match size {
            Size::XSmall => ui_px(44.0),
            Size::Small => ui_px(48.0),
            Size::Medium => ui_px(52.0),
            Size::Large => ui_px(56.0),
        };
        let resolved_width = match (collapse_mode, collapsed) {
            (SidebarCollapseMode::Offcanvas, true) => ui_px(0.0),
            (SidebarCollapseMode::Icon, true) => collapsed_width,
            _ => expanded_width,
        };

        Self {
            expanded_width,
            collapsed_width,
            resolved_width,
            padding: match size {
                Size::XSmall | Size::Small => ui_px(8.0),
                Size::Medium | Size::Large => ui_px(10.0),
            },
            section_gap: ui_px(12.0),
            item_gap: ui_px(4.0),
            item_height: size.button_h(),
            item_padding_x: size.button_px(),
            item_padding_y: size.button_py(),
            icon_size: size.icon_size(),
            badge_min_height: match size {
                Size::XSmall => ui_px(16.0),
                Size::Small => ui_px(18.0),
                Size::Medium => ui_px(20.0),
                Size::Large => ui_px(22.0),
            },
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the expanded width.
    pub const fn expanded_width(self) -> UiPx {
        self.expanded_width
    }

    /// Returns the icon-collapsed width.
    pub const fn collapsed_width(self) -> UiPx {
        self.collapsed_width
    }

    /// Returns the effective layout width.
    pub const fn resolved_width(self) -> UiPx {
        self.resolved_width
    }

    /// Returns sidebar padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns the gap between sections.
    pub const fn section_gap(self) -> UiPx {
        self.section_gap
    }

    /// Returns the gap between items.
    pub const fn item_gap(self) -> UiPx {
        self.item_gap
    }

    /// Returns item height.
    pub const fn item_height(self) -> UiPx {
        self.item_height
    }

    /// Returns item horizontal padding.
    pub const fn item_padding_x(self) -> UiPx {
        self.item_padding_x
    }

    /// Returns item vertical padding.
    pub const fn item_padding_y(self) -> UiPx {
        self.item_padding_y
    }

    /// Returns icon glyph size.
    pub const fn icon_size(self) -> UiPx {
        self.icon_size
    }

    /// Returns badge minimum height.
    pub const fn badge_min_height(self) -> UiPx {
        self.badge_min_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved sidebar section state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSectionState {
    index: usize,
    value: String,
    label: String,
    item_start: usize,
    item_count: usize,
}

impl SidebarSectionState {
    /// Returns the zero-based section index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable section value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible section label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the first flattened item index in this section.
    pub const fn item_start(&self) -> usize {
        self.item_start
    }

    /// Returns the number of items in this section.
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Returns the accessibility role for the section.
    pub const fn role(&self) -> Role {
        Role::Section
    }
}

/// Resolved sidebar item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItemState {
    index: usize,
    section_index: usize,
    item_index: usize,
    value: String,
    label: String,
    icon: Option<ResolvedActionIcon>,
    badge: Option<String>,
    action_label: Option<String>,
    disabled: bool,
    disabled_reason: Option<String>,
    duplicate_value: bool,
    shortcut: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
    selected: bool,
    focused: bool,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl SidebarItemState {
    /// Returns the zero-based flattened item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the zero-based section index.
    pub const fn section_index(&self) -> usize {
        self.section_index
    }

    /// Returns the zero-based item index within the section.
    pub const fn item_index(&self) -> usize {
        self.item_index
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible or accessible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns app-resolved icon metadata.
    pub const fn icon(&self) -> Option<&ResolvedActionIcon> {
        self.icon.as_ref()
    }

    /// Returns the optional icon glyph or symbolic icon label.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns display-only badge text.
    pub fn badge_label(&self) -> Option<&str> {
        self.badge.as_deref()
    }

    /// Returns display-only trailing action label.
    pub fn action_label_text(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns whether this item shares its stable value with another sidebar item.
    ///
    /// Duplicate values fail closed because value-addressed focus and programmatic activation
    /// would otherwise be ambiguous across sections.
    pub const fn duplicate_value(&self) -> bool {
        self.duplicate_value
    }

    /// Returns the display shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns user-displayable tooltip metadata.
    pub fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }

    /// Returns the optional accessibility description.
    pub fn accessibility_description(&self) -> Option<&str> {
        self.accessibility_description.as_deref()
    }

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        !self.disabled && self.position_in_set.is_some()
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        self.focusable()
    }

    /// Returns the item's position among focusable navigation items.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total count of focusable navigation items.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }

    /// Returns the accessibility role for the item.
    pub const fn role(&self) -> Role {
        Role::Button
    }
}

/// Resolved sidebar activation payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarActivation {
    value: String,
    selected: bool,
}

impl SidebarActivation {
    /// Creates an activation payload from a resolved item.
    fn for_item(item: &SidebarItemState) -> Self {
        Self {
            value: item.value.clone(),
            selected: item.selected,
        }
    }

    /// Returns the activated item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns whether the item was already selected when activated.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Resolved sidebar state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarState {
    side: SidebarSide,
    variant: SidebarVariant,
    collapse_mode: SidebarCollapseMode,
    collapsed: bool,
    disabled: bool,
    label: String,
    size: Size,
    sections: Vec<SidebarSectionState>,
    items: Vec<SidebarItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
    metrics: SidebarMetrics,
    colors: SidebarColors,
    focus_ring: FocusRing,
}

impl SidebarState {
    /// Resolves public state for a sidebar.
    pub fn resolve(
        side: SidebarSide,
        variant: SidebarVariant,
        collapse_mode: SidebarCollapseMode,
        collapsed: bool,
        disabled: bool,
        label: impl Into<String>,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
        sections: impl IntoIterator<Item = SidebarSectionDescriptor>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let collapsed = collapsed && collapse_mode != SidebarCollapseMode::None;
        let offcanvas_collapsed = collapsed && collapse_mode == SidebarCollapseMode::Offcanvas;
        let section_descriptors: Vec<SidebarSectionDescriptor> = sections.into_iter().collect();
        let collection_disabled = disabled || offcanvas_collapsed;
        let collection = ChoiceCollection::resolve_unique(
            collection_disabled,
            sidebar_choice_items(collection_disabled, &section_descriptors),
            selected_value,
            focused_value,
            ChoiceInteractionPolicy::single_optional(Orientation::Vertical),
        );
        let selected_index = collection.selected_index();
        let focused_index = collection.active_index();
        let focusable_set_size = collection
            .items()
            .iter()
            .filter(|item| item.enabled())
            .count();
        let mut section_states = Vec::with_capacity(section_descriptors.len());
        let mut item_start = 0usize;

        for (section_index, section) in section_descriptors.iter().enumerate() {
            let item_count = section.items.len();
            section_states.push(SidebarSectionState {
                index: section_index,
                value: section.value.clone(),
                label: section.label.clone(),
                item_start,
                item_count,
            });
            item_start += item_count;
        }

        let mut focusable_position = 0usize;
        let item_states = collection
            .into_items()
            .into_iter()
            .map(|projection| {
                let index = projection.source_index();
                let item_disabled = !projection.enabled();
                let duplicate_value = projection.ambiguous_value();
                let (section_index, item_index, item) = projection.into_item();
                let position_in_set = if item_disabled {
                    None
                } else {
                    focusable_position += 1;
                    Some(focusable_position)
                };

                SidebarItemState {
                    index,
                    section_index,
                    item_index,
                    value: item.value,
                    label: item.label,
                    icon: item.icon,
                    badge: item.badge,
                    action_label: item.action_label,
                    disabled: item_disabled,
                    disabled_reason: item.disabled_reason,
                    duplicate_value,
                    shortcut: item.shortcut,
                    tooltip: item.tooltip,
                    accessibility_description: item.accessibility_description,
                    selected: Some(index) == selected_index,
                    focused: Some(index) == focused_index,
                    position_in_set,
                    size_of_set: position_in_set.map_or(0, |_| focusable_set_size),
                }
            })
            .collect();

        let metrics = SidebarMetrics::from_size(size, collapse_mode, collapsed);
        let colors = SidebarColors::from_tokens(tokens);

        Self {
            side,
            variant,
            collapse_mode,
            collapsed,
            disabled,
            label: label.into(),
            size,
            sections: section_states,
            items: item_states,
            selected_index,
            focused_index,
            metrics,
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the sidebar side.
    pub const fn side(&self) -> SidebarSide {
        self.side
    }

    /// Returns the sidebar variant.
    pub const fn variant(&self) -> SidebarVariant {
        self.variant
    }

    /// Returns the collapse mode.
    pub const fn collapse_mode(&self) -> SidebarCollapseMode {
        self.collapse_mode
    }

    /// Returns effective collapsed state.
    pub const fn collapsed(&self) -> bool {
        self.collapsed
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the sidebar is icon-collapsed.
    pub const fn icon_collapsed(&self) -> bool {
        self.collapsed && matches!(self.collapse_mode, SidebarCollapseMode::Icon)
    }

    /// Returns whether the sidebar is offcanvas-collapsed.
    pub const fn offcanvas_collapsed(&self) -> bool {
        self.collapsed && matches!(self.collapse_mode, SidebarCollapseMode::Offcanvas)
    }

    /// Returns whether the whole sidebar is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessible sidebar label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Navigation
    }

    /// Returns resolved section states.
    pub fn sections(&self) -> &[SidebarSectionState] {
        &self.sections
    }

    /// Returns flattened resolved item states.
    pub fn items(&self) -> &[SidebarItemState] {
        &self.items
    }

    /// Returns selected item index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns selected item value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(SidebarItemState::value)
    }

    /// Returns the selected item.
    pub fn selected_item(&self) -> Option<&SidebarItemState> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    /// Returns focused item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns focused item value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(SidebarItemState::value)
    }

    /// Returns the focused item.
    pub fn focused_item(&self) -> Option<&SidebarItemState> {
        self.focused_index.and_then(|index| self.items.get(index))
    }

    /// Returns tab-stop item index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns whether menu content should be scrollable.
    pub const fn scrollable(&self) -> bool {
        !self.offcanvas_collapsed()
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SidebarMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> SidebarColors {
        self.colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Resolves a focus target for APG-style vertical navigation.
    pub fn navigation_target(&self, key: &str) -> Option<&SidebarItemState> {
        let current = self.focused_index?;
        let disabled = self.disabled_map();
        sidebar_navigation_target(key, current, &disabled).and_then(|index| self.items.get(index))
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.items.iter().map(|item| !item.focusable()).collect()
    }
}

type SidebarChoiceItem = (usize, usize, SidebarItemDescriptor);

fn sidebar_choice_items(
    disabled: bool,
    sections: &[SidebarSectionDescriptor],
) -> Vec<ChoiceItemProjection<SidebarChoiceItem>> {
    sections
        .iter()
        .enumerate()
        .flat_map(|(section_index, section)| {
            section
                .items
                .iter()
                .cloned()
                .enumerate()
                .map(move |(item_index, item)| (section_index, item_index, item))
        })
        .enumerate()
        .map(|(index, (section_index, item_index, item))| {
            let value = item.value().to_owned();
            let label = item.label().to_owned();
            let item_disabled = disabled || item.disabled_state();

            ChoiceItemProjection::new(
                index,
                Some(section_index),
                value,
                label.clone(),
                item_disabled,
                (section_index, item_index, item),
            )
            .text_value(label)
        })
        .collect()
}

/// Resolves a sidebar roving-focus target from an APG-style key name.
pub fn sidebar_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    ChoiceInteractionPolicy::single_optional(Orientation::Vertical)
        .navigation_target_index(key, current, disabled)
}

/// A concrete GPUI sidebar navigation item.
#[derive(Clone)]
pub struct SidebarItem {
    descriptor: SidebarItemDescriptor,
    on_activate: Option<SidebarActivationHandler>,
}

type SidebarActivationHandler = Rc<dyn Fn(SidebarActivation, Activation, &mut Window, &mut App)>;

impl SidebarItem {
    /// Creates a sidebar navigation item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: SidebarItemDescriptor::new(value, label.to_string()),
            on_activate: None,
        }
    }

    /// Creates a sidebar navigation item from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        Self {
            descriptor: SidebarItemDescriptor::from_resolved_action(action),
            on_activate: None,
        }
    }

    /// Applies an icon glyph or symbolic icon label.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.descriptor = self.descriptor.icon(icon.into().to_string());
        self
    }

    /// Applies app-resolved icon metadata.
    pub fn resolved_icon(mut self, icon: ResolvedActionIcon) -> Self {
        self.descriptor = self.descriptor.resolved_icon(icon);
        self
    }

    /// Applies display-only badge text.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.descriptor = self.descriptor.badge(badge.into().to_string());
        self
    }

    /// Applies a display-only trailing action label.
    pub fn action_label(mut self, action_label: impl Into<SharedString>) -> Self {
        self.descriptor = self
            .descriptor
            .action_label(action_label.into().to_string());
        self
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Marks the item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.disabled_reason(reason);
        self
    }

    /// Applies a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip_text(mut self, tooltip: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.tooltip(tooltip);
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.accessibility_description(description);
        self
    }

    /// Registers this item's activation handler.
    ///
    /// An item handler takes precedence over the sidebar-level fallback so one activation invokes
    /// exactly one domain callback.
    pub fn on_activate(
        mut self,
        handler: impl Fn(SidebarActivation, Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> SidebarItemDescriptor {
        self.descriptor.clone()
    }
}

/// A concrete GPUI sidebar section.
#[derive(Clone)]
pub struct SidebarSection {
    descriptor: SidebarSectionDescriptor,
    items: Vec<SidebarItem>,
}

impl SidebarSection {
    /// Creates an empty sidebar section.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let value = value.into();
        let label = label.into();
        Self {
            descriptor: SidebarSectionDescriptor::new(value, label.to_string()),
            items: Vec::new(),
        }
    }

    /// Adds one navigation item.
    pub fn item(mut self, item: impl Into<SidebarItem>) -> Self {
        self.items.push(item.into());
        self
    }

    /// Replaces section items.
    pub fn items(mut self, items: impl IntoIterator<Item = impl Into<SidebarItem>>) -> Self {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    /// Returns a pure descriptor for this section.
    pub fn descriptor(&self) -> SidebarSectionDescriptor {
        self.descriptor.clone().items(
            self.items
                .iter()
                .map(SidebarItem::descriptor)
                .collect::<Vec<_>>(),
        )
    }

    fn item_models(&self) -> &[SidebarItem] {
        &self.items
    }
}

/// A concrete GPUI sidebar.
#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    label: SharedString,
    side: SidebarSide,
    variant: SidebarVariant,
    collapse_mode: SidebarCollapseMode,
    collapsed: bool,
    disabled: bool,
    selected_value: Option<String>,
    focused_value: Option<String>,
    size: Size,
    tokens: ThemeTokens,
    sections: Vec<SidebarSection>,
    on_activate: Option<SidebarActivationHandler>,
    activation_handles: BTreeMap<String, ActivationHandle>,
}

impl Sidebar {
    /// Creates an empty sidebar with an accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            side: SidebarSide::Left,
            variant: SidebarVariant::Docked,
            collapse_mode: SidebarCollapseMode::Icon,
            collapsed: false,
            disabled: false,
            selected_value: None,
            focused_value: None,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            sections: Vec::new(),
            on_activate: None,
            activation_handles: BTreeMap::new(),
        }
    }

    /// Applies the sidebar side.
    pub fn side(mut self, side: SidebarSide) -> Self {
        self.side = side;
        self
    }

    /// Attaches the sidebar to the left edge.
    pub fn left(self) -> Self {
        self.side(SidebarSide::Left)
    }

    /// Attaches the sidebar to the right edge.
    pub fn right(self) -> Self {
        self.side(SidebarSide::Right)
    }

    /// Applies the visual variant.
    pub fn variant(mut self, variant: SidebarVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies collapse behavior.
    pub fn collapse_mode(mut self, collapse_mode: SidebarCollapseMode) -> Self {
        self.collapse_mode = collapse_mode;
        self
    }

    /// Seeds collapsed state.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Marks the whole sidebar as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the selected item value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds one section.
    pub fn section(mut self, section: impl Into<SidebarSection>) -> Self {
        self.sections.push(section.into());
        self
    }

    /// Replaces sections.
    pub fn sections(
        mut self,
        sections: impl IntoIterator<Item = impl Into<SidebarSection>>,
    ) -> Self {
        self.sections = sections.into_iter().map(Into::into).collect();
        self
    }

    /// Registers the fallback activation handler for items without their own handler.
    pub fn on_activate(
        mut self,
        handler: impl Fn(SidebarActivation, Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Binds an application-owned activation handle to one stable item value.
    pub fn activation_handle(
        mut self,
        value: impl Into<String>,
        handle: &ActivationHandle,
    ) -> Self {
        self.activation_handles.insert(value.into(), handle.clone());
        self
    }

    /// Returns the resolved sidebar state.
    pub fn state(&self) -> SidebarState {
        SidebarState::resolve(
            self.side,
            self.variant,
            self.collapse_mode,
            self.collapsed,
            self.disabled,
            self.label.to_string(),
            self.selected_value.as_deref(),
            self.focused_value.as_deref(),
            self.sections.iter().map(SidebarSection::descriptor),
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Sidebar {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl From<SidebarItemDescriptor> for SidebarItem {
    fn from(descriptor: SidebarItemDescriptor) -> Self {
        Self {
            descriptor,
            on_activate: None,
        }
    }
}

impl From<SidebarSectionDescriptor> for SidebarSection {
    fn from(descriptor: SidebarSectionDescriptor) -> Self {
        let items = descriptor
            .item_descriptors()
            .iter()
            .cloned()
            .map(SidebarItem::from)
            .collect();

        Self { descriptor, items }
    }
}
