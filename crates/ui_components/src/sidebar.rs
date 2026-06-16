//! Sidebar component.

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    App, ClickEvent, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens};

use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::roving_focus::roving_navigation_target;
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;

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
    icon: Option<String>,
    badge: Option<String>,
    action_label: Option<String>,
    disabled: bool,
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
        }
    }

    /// Applies an icon glyph or symbolic icon label.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
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

    /// Returns the optional icon glyph or symbolic icon label.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_deref()
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
    expanded_width: open_gpui::Pixels,
    collapsed_width: open_gpui::Pixels,
    resolved_width: open_gpui::Pixels,
    padding: open_gpui::Pixels,
    section_gap: open_gpui::Pixels,
    item_gap: open_gpui::Pixels,
    item_height: open_gpui::Pixels,
    item_padding_x: open_gpui::Pixels,
    item_padding_y: open_gpui::Pixels,
    icon_size: open_gpui::Pixels,
    badge_min_height: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
}

impl SidebarMetrics {
    /// Resolves metrics from size and effective collapsed state.
    pub const fn from_size(
        size: Size,
        collapse_mode: SidebarCollapseMode,
        collapsed: bool,
    ) -> Self {
        let expanded_width = match size {
            Size::XSmall => px(216.0),
            Size::Small => px(232.0),
            Size::Medium => px(248.0),
            Size::Large => px(272.0),
        };
        let collapsed_width = match size {
            Size::XSmall => px(44.0),
            Size::Small => px(48.0),
            Size::Medium => px(52.0),
            Size::Large => px(56.0),
        };
        let resolved_width = match (collapse_mode, collapsed) {
            (SidebarCollapseMode::Offcanvas, true) => px(0.0),
            (SidebarCollapseMode::Icon, true) => collapsed_width,
            _ => expanded_width,
        };

        Self {
            expanded_width,
            collapsed_width,
            resolved_width,
            padding: match size {
                Size::XSmall | Size::Small => px(8.0),
                Size::Medium | Size::Large => px(10.0),
            },
            section_gap: px(12.0),
            item_gap: px(4.0),
            item_height: size.button_h(),
            item_padding_x: size.button_px(),
            item_padding_y: size.button_py(),
            icon_size: size.icon_size(),
            badge_min_height: match size {
                Size::XSmall => px(16.0),
                Size::Small => px(18.0),
                Size::Medium => px(20.0),
                Size::Large => px(22.0),
            },
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the expanded width.
    pub const fn expanded_width(self) -> open_gpui::Pixels {
        self.expanded_width
    }

    /// Returns the icon-collapsed width.
    pub const fn collapsed_width(self) -> open_gpui::Pixels {
        self.collapsed_width
    }

    /// Returns the effective layout width.
    pub const fn resolved_width(self) -> open_gpui::Pixels {
        self.resolved_width
    }

    /// Returns sidebar padding.
    pub const fn padding(self) -> open_gpui::Pixels {
        self.padding
    }

    /// Returns the gap between sections.
    pub const fn section_gap(self) -> open_gpui::Pixels {
        self.section_gap
    }

    /// Returns the gap between items.
    pub const fn item_gap(self) -> open_gpui::Pixels {
        self.item_gap
    }

    /// Returns item height.
    pub const fn item_height(self) -> open_gpui::Pixels {
        self.item_height
    }

    /// Returns item horizontal padding.
    pub const fn item_padding_x(self) -> open_gpui::Pixels {
        self.item_padding_x
    }

    /// Returns item vertical padding.
    pub const fn item_padding_y(self) -> open_gpui::Pixels {
        self.item_padding_y
    }

    /// Returns icon glyph size.
    pub const fn icon_size(self) -> open_gpui::Pixels {
        self.icon_size
    }

    /// Returns badge minimum height.
    pub const fn badge_min_height(self) -> open_gpui::Pixels {
        self.badge_min_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> open_gpui::Pixels {
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
    visible: bool,
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

    /// Returns whether this section is visible.
    pub const fn visible(&self) -> bool {
        self.visible
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
    icon: Option<String>,
    badge: Option<String>,
    action_label: Option<String>,
    disabled: bool,
    selected: bool,
    focused: bool,
    tab_stop: bool,
    visible: bool,
    text_visible: bool,
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

    /// Returns the optional icon glyph or symbolic icon label.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_deref()
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

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether the item should be the tab stop.
    pub const fn tab_stop(&self) -> bool {
        self.tab_stop
    }

    /// Returns whether the item should be rendered by the adapter.
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Returns whether visible text should be rendered by the adapter.
    pub const fn text_visible(&self) -> bool {
        self.text_visible
    }

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        self.visible && !self.disabled
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

/// Resolved sidebar selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSelection {
    index: usize,
    section_index: usize,
    item_index: usize,
    value: String,
    label: String,
    selected: bool,
}

impl SidebarSelection {
    /// Creates a selection payload from a resolved item.
    pub fn from_item(item: &SidebarItemState) -> Option<Self> {
        item.activation_enabled().then(|| Self {
            index: item.index,
            section_index: item.section_index,
            item_index: item.item_index,
            value: item.value.clone(),
            label: item.label.clone(),
            selected: item.selected,
        })
    }

    /// Returns the flattened item index.
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

    /// Returns the selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected item label.
    pub fn label(&self) -> &str {
        &self.label
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
    sections: Vec<SidebarSectionState>,
    items: Vec<SidebarItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
    tab_stop_index: Option<usize>,
    scrollable: bool,
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
        let icon_collapsed = collapsed && collapse_mode == SidebarCollapseMode::Icon;
        let offcanvas_collapsed = collapsed && collapse_mode == SidebarCollapseMode::Offcanvas;
        let visible = !offcanvas_collapsed;
        let text_visible = !icon_collapsed;
        let section_descriptors: Vec<SidebarSectionDescriptor> = sections.into_iter().collect();
        let mut section_states = Vec::with_capacity(section_descriptors.len());
        let mut item_states = Vec::new();

        for (section_index, section) in section_descriptors.into_iter().enumerate() {
            let item_start = item_states.len();
            let item_count = section.items.len();
            section_states.push(SidebarSectionState {
                index: section_index,
                value: section.value,
                label: section.label,
                item_start,
                item_count,
                visible,
            });

            for (item_index, item) in section.items.into_iter().enumerate() {
                let index = item_states.len();
                item_states.push(SidebarItemState {
                    index,
                    section_index,
                    item_index,
                    value: item.value,
                    label: item.label,
                    icon: item.icon,
                    badge: item.badge,
                    action_label: item.action_label,
                    disabled: disabled || item.disabled,
                    selected: false,
                    focused: false,
                    tab_stop: false,
                    visible,
                    text_visible,
                    position_in_set: None,
                    size_of_set: 0,
                });
            }
        }

        let disabled_map: Vec<bool> = item_states
            .iter()
            .map(|item| !visible || item.disabled)
            .collect();
        let values: Vec<String> = item_states.iter().map(|item| item.value.clone()).collect();
        let selected_index = selected_value.and_then(|selected| {
            values
                .iter()
                .position(|value| value == selected)
                .filter(|index| !disabled_map.get(*index).copied().unwrap_or(true))
        });
        let selected_seed = selected_index
            .and_then(|index| values.get(index))
            .map(String::as_str);
        let focused_index = if visible && !disabled {
            crate::roving_focus::selection_index_from_str_keys(
                &values,
                &disabled_map,
                focused_value,
                selected_seed,
            )
        } else {
            None
        };
        let tab_stop_index = focused_index;
        let focusable_set_size = disabled_map.iter().filter(|disabled| !**disabled).count();
        let mut focusable_position = 0usize;

        for item in &mut item_states {
            item.selected = Some(item.index) == selected_index;
            item.focused = Some(item.index) == focused_index;
            item.tab_stop = Some(item.index) == tab_stop_index;

            if item.focusable() {
                focusable_position += 1;
                item.position_in_set = Some(focusable_position);
                item.size_of_set = focusable_set_size;
            }
        }

        let metrics = SidebarMetrics::from_size(size, collapse_mode, collapsed);
        let colors = SidebarColors::from_tokens(tokens);

        Self {
            side,
            variant,
            collapse_mode,
            collapsed,
            disabled,
            label: label.into(),
            sections: section_states,
            items: item_states,
            selected_index,
            focused_index,
            tab_stop_index,
            scrollable: visible,
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
        self.tab_stop_index
    }

    /// Returns tab-stop item value.
    pub fn tab_stop_value(&self) -> Option<&str> {
        self.tab_stop_index
            .and_then(|index| self.items.get(index))
            .map(SidebarItemState::value)
    }

    /// Returns whether menu content should be scrollable.
    pub const fn scrollable(&self) -> bool {
        self.scrollable
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

    /// Resolves an activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<SidebarSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.focused_index
            .and_then(|index| self.items.get(index))
            .and_then(SidebarSelection::from_item)
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.items.iter().map(|item| !item.focusable()).collect()
    }
}

/// Resolves a sidebar roving-focus target from an APG-style key name.
pub fn sidebar_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    roving_navigation_target(Orientation::Vertical, key, current, disabled)
}

/// A concrete GPUI sidebar navigation item.
#[derive(Clone)]
pub struct SidebarItem {
    descriptor: SidebarItemDescriptor,
    on_select: Option<Rc<dyn Fn(SidebarSelection, &mut Window, &mut App)>>,
}

impl SidebarItem {
    /// Creates a sidebar navigation item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: SidebarItemDescriptor::new(value, label.to_string()),
            on_select: None,
        }
    }

    /// Applies an icon glyph or symbolic icon label.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.descriptor = self.descriptor.icon(icon.into().to_string());
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

    /// Registers an item selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(SidebarSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> SidebarItemDescriptor {
        self.descriptor.clone()
    }

    fn select_handler(&self) -> Option<Rc<dyn Fn(SidebarSelection, &mut Window, &mut App)>> {
        self.on_select.clone()
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
    on_selection_change: Option<Rc<dyn Fn(SidebarSelection, &mut Window, &mut App)>>,
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
            on_selection_change: None,
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

    /// Applies the focused item value.
    pub fn focused(mut self, value: impl Into<String>) -> Self {
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

    /// Registers a selection-change handler.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(SidebarSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
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

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Sidebar {
            id,
            label,
            side,
            variant,
            collapse_mode,
            collapsed,
            disabled,
            selected_value,
            focused_value,
            size,
            tokens,
            sections,
            on_selection_change,
        } = self;

        window.with_id(id.clone(), |window| {
            let descriptors: Vec<SidebarSectionDescriptor> =
                sections.iter().map(SidebarSection::descriptor).collect();
            let item_models: Vec<SidebarItem> = sections
                .iter()
                .flat_map(|section| section.item_models().iter().cloned())
                .collect();
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| SidebarRuntime {
                focused_value: focused_seed,
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                runtime.focused_value.clone()
            };
            let state = SidebarState::resolve(
                side,
                variant,
                collapse_mode,
                collapsed,
                disabled,
                label.to_string(),
                selected_value.as_deref(),
                runtime_snapshot.as_deref(),
                descriptors.clone(),
                size,
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, cx));

            let metrics = state.metrics();
            let colors = state.colors();
            let focus_ring = state.focus_ring();
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| !item.focusable())
                    .collect::<Vec<_>>(),
            );
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let icon_collapsed = state.icon_collapsed();
            let item_states = Rc::new(state.items().to_vec());
            let section_states = state.sections().to_vec();
            let sections_content = div()
                .flex()
                .flex_col()
                .gap(metrics.section_gap())
                .p(metrics.padding())
                .children(section_states.into_iter().map(|section| {
                    let section_items = item_states
                        .iter()
                        .filter(|item| item.section_index() == section.index())
                        .cloned()
                        .collect::<Vec<_>>();
                    let item_models = item_models.clone();
                    let on_selection_change = on_selection_change.clone();
                    let focus_handles = focus_handles.clone();
                    let runtime = runtime.clone();
                    let disabled_items = disabled_items.clone();
                    let item_states_for_section = item_states.clone();

                    div()
                        .id(sidebar_section_id(section.value()))
                        .role(section.role())
                        .aria_label(section.label().to_owned())
                        .flex()
                        .flex_col()
                        .gap(metrics.item_gap())
                        .when(!icon_collapsed, |this| {
                            this.child(
                                div()
                                    .px(metrics.item_padding_x())
                                    .text_xs()
                                    .line_height(metrics.text_size())
                                    .text_color(ThemeResolver::resolve(colors.muted_foreground()))
                                    .child(section.label().to_owned()),
                            )
                        })
                        .children(section_items.into_iter().map(move |item| {
                            let item_index = item.index();
                            let model = item_models[item_index].clone();
                            let click_item_handler = model.select_handler();
                            let key_item_handler = click_item_handler.clone();
                            let click_sidebar_handler = on_selection_change.clone();
                            let key_sidebar_handler = click_sidebar_handler.clone();
                            let selection = SidebarSelection::from_item(&item);
                            let click_selection = selection.clone();
                            let key_selection = selection.clone();
                            let focus_handle = focus_handles[item_index].clone();
                            let key_runtime = runtime.clone();
                            let click_runtime = runtime.clone();
                            let disabled_items = disabled_items.clone();
                            let key_item_states = item_states_for_section.clone();
                            let item_value = item.value().to_owned();
                            let key_item_value = item_value.clone();
                            let item_disabled = item.disabled();
                            let item_selected = item.selected();
                            let item_tab_stop = item.tab_stop();
                            let item_text_visible = item.text_visible();
                            let item_icon = item
                                .icon_label()
                                .map(SharedString::from)
                                .unwrap_or_else(|| fallback_icon_label(item.label()));
                            let item_label = item.label().to_owned();
                            let item_badge = item.badge_label().map(str::to_owned);
                            let item_action = item.action_label_text().map(str::to_owned);
                            let item_position = item.position_in_set();
                            let item_size_of_set = item.size_of_set();

                            div()
                                .id(sidebar_item_id(item.value()))
                                .focusable()
                                .tab_stop(item_tab_stop)
                                .role(item.role())
                                .aria_label(item.label().to_owned())
                                .aria_selected(item_selected)
                                .aria_disabled(item_disabled)
                                .when_some(item_position, |this, position| {
                                    this.aria_position_in_set(position)
                                        .aria_size_of_set(item_size_of_set)
                                })
                                .when_some(focus_handle, |this, focus_handle| {
                                    this.track_focus(&focus_handle)
                                })
                                .min_h(metrics.item_height())
                                .px(if item_text_visible {
                                    metrics.item_padding_x()
                                } else {
                                    px(0.0)
                                })
                                .py(metrics.item_padding_y())
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .rounded(metrics.radius())
                                .bg(ThemeResolver::resolve(if item_selected {
                                    colors.item_selected_background()
                                } else {
                                    colors.item_background()
                                }))
                                .text_size(metrics.text_size())
                                .line_height(metrics.text_size())
                                .text_color(ThemeResolver::resolve(if item_disabled {
                                    colors.item_disabled_foreground()
                                } else {
                                    colors.foreground()
                                }))
                                .focus_visible(move |style| {
                                    style.shadow(focus_ring_shadow(focus_ring))
                                })
                                .when(!item_disabled, |this| {
                                    this.cursor_pointer().hover(move |style| {
                                        style.bg(ThemeResolver::resolve(
                                            colors.item_hover_background(),
                                        ))
                                    })
                                })
                                .when(item_disabled, |this| {
                                    this.opacity(0.56).cursor_not_allowed()
                                })
                                .on_click({
                                    let item_value = item_value.clone();
                                    move |_event: &ClickEvent, window, cx| {
                                        if item_disabled {
                                            return;
                                        }

                                        cx.stop_propagation();
                                        let focus_handle = click_runtime
                                            .update(cx, |runtime, cx| {
                                                runtime.set_focused(&item_value, cx)
                                            });

                                        if let Some(selection) = click_selection.clone() {
                                            if let Some(handler) = click_item_handler.clone() {
                                                handler(selection.clone(), window, cx);
                                            }
                                            if let Some(handler) = click_sidebar_handler.clone() {
                                                handler(selection, window, cx);
                                            }
                                        }

                                        if let Some(focus_handle) = focus_handle {
                                            focus_handle.focus(window, cx);
                                        }
                                    }
                                })
                                .on_key_down({
                                    move |event: &KeyDownEvent, window, cx| {
                                        if item_disabled {
                                            return;
                                        }
                                        if event.keystroke.modifiers.modified() {
                                            return;
                                        }

                                        let key = event.keystroke.key.as_str();
                                        let Some(target_index) = sidebar_navigation_target(
                                            key,
                                            item_index,
                                            &disabled_items,
                                        ) else {
                                            if !matches!(key, "space" | "enter") {
                                                return;
                                            }

                                            key_runtime.update(cx, |runtime, cx| {
                                                runtime.set_focused(&key_item_value, cx)
                                            });
                                            if let Some(selection) = key_selection.clone() {
                                                if let Some(handler) = key_item_handler.clone() {
                                                    handler(selection.clone(), window, cx);
                                                }
                                                if let Some(handler) = key_sidebar_handler.clone() {
                                                    handler(selection, window, cx);
                                                }
                                            }
                                            cx.stop_propagation();
                                            return;
                                        };

                                        let target_value =
                                            key_item_states[target_index].value().to_owned();
                                        let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                            runtime.set_focused(&target_value, cx)
                                        });

                                        if let Some(focus_handle) = focus_handle {
                                            focus_handle.focus(window, cx);
                                        }

                                        cx.stop_propagation();
                                    }
                                })
                                .child(
                                    div()
                                        .min_w(metrics.icon_size())
                                        .text_size(metrics.icon_size())
                                        .line_height(metrics.icon_size())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(item_icon),
                                )
                                .when(item_text_visible, |this| {
                                    this.child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .child(item_label),
                                    )
                                    .when_some(item_badge, |this, badge| {
                                        this.child(
                                            div()
                                                .min_h(metrics.badge_min_height())
                                                .px(px(7.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(999.0))
                                                .bg(ThemeResolver::resolve(
                                                    colors.badge_background(),
                                                ))
                                                .text_color(ThemeResolver::resolve(
                                                    colors.badge_foreground(),
                                                ))
                                                .text_xs()
                                                .child(badge),
                                        )
                                    })
                                    .when_some(
                                        item_action,
                                        |this, action| {
                                            this.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(ThemeResolver::resolve(
                                                        colors.muted_foreground(),
                                                    ))
                                                    .child(action),
                                            )
                                        },
                                    )
                                })
                        }))
                }));

            div()
                .id(id.clone())
                .role(state.role())
                .aria_label(label.clone())
                .aria_disabled(state.disabled())
                .w(metrics.resolved_width())
                .h_full()
                .flex_none()
                .flex()
                .flex_col()
                .overflow_hidden()
                .border_color(ThemeResolver::resolve(colors.border()))
                .bg(ThemeResolver::resolve(match variant {
                    SidebarVariant::Docked => colors.surface(),
                    SidebarVariant::Floating | SidebarVariant::Inset => colors.floating_surface(),
                }))
                .text_color(ThemeResolver::resolve(colors.foreground()))
                .when(
                    variant == SidebarVariant::Docked && side == SidebarSide::Left,
                    |this| this.border_r_1(),
                )
                .when(
                    variant == SidebarVariant::Docked && side == SidebarSide::Right,
                    |this| this.border_l_1(),
                )
                .when(variant != SidebarVariant::Docked, |this| {
                    this.border_1().rounded(metrics.radius())
                })
                .when(!state.offcanvas_collapsed(), |this| {
                    this.child(
                        ScrollArea::new(format!("{id}-scroll"), sections_content)
                            .vertical()
                            .with_size(size),
                    )
                })
        })
    }
}

#[derive(Debug, Default)]
struct SidebarRuntime {
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl SidebarRuntime {
    fn sync(&mut self, state: &SidebarState, cx: &mut Context<Self>) {
        self.focus_handles
            .retain(|value, _| state.items().iter().any(|item| item.value() == value));

        for item in state.items().iter().filter(|item| item.focusable()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn set_focused(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }
}

fn fallback_icon_label(label: &str) -> SharedString {
    label
        .chars()
        .next()
        .map(|ch| ch.to_string())
        .unwrap_or_default()
        .into()
}

fn sidebar_section_id(value: &str) -> ElementId {
    format!("sidebar-section-{value}").into()
}

fn sidebar_item_id(value: &str) -> ElementId {
    format!("sidebar-item-{value}").into()
}

impl From<SidebarItemDescriptor> for SidebarItem {
    fn from(descriptor: SidebarItemDescriptor) -> Self {
        Self {
            descriptor,
            on_select: None,
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
