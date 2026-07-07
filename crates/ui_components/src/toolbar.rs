//! Toolbar component.

use crate::geometry::gpui_px_from_ui;
use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyView, App, ClickEvent, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div,
};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::action::{ResolvedActionIcon, ResolvedActionState};
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;
use crate::tooltip::Tooltip;

/// Item kind for a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarItemKind {
    /// Activatable command button.
    Action,
    /// Pressed/unpressed command button.
    Toggle,
    /// Visual separator. Separators are not focusable or activatable.
    Separator,
}

impl ToolbarItemKind {
    /// Returns the stable item kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Toggle => "toggle",
            Self::Separator => "separator",
        }
    }
}

/// Pure descriptor for one toolbar item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolbarItemDescriptor {
    value: String,
    label: String,
    icon: Option<ResolvedActionIcon>,
    kind: ToolbarItemKind,
    disabled: bool,
    disabled_reason: Option<String>,
    pressed: bool,
    shortcut: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
}

impl ToolbarItemDescriptor {
    /// Creates an action item descriptor.
    pub fn action(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            kind: ToolbarItemKind::Action,
            disabled: false,
            disabled_reason: None,
            pressed: false,
            shortcut: None,
            tooltip: None,
            accessibility_description: None,
        }
    }

    /// Creates a toggle item descriptor.
    pub fn toggle(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            disabled_reason: None,
            pressed: false,
            shortcut: None,
            tooltip: None,
            accessibility_description: None,
        }
    }

    /// Creates an action item descriptor from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        let mut item = Self::action(action.value(), action.label()).disabled(action.disabled());
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

    /// Creates a separator descriptor.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: String::new(),
            icon: None,
            kind: ToolbarItemKind::Separator,
            disabled: true,
            disabled_reason: None,
            pressed: false,
            shortcut: None,
            tooltip: None,
            accessibility_description: None,
        }
    }

    /// Marks an action or toggle item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if self.kind != ToolbarItemKind::Separator {
            self.disabled = disabled;
            if !disabled {
                self.disabled_reason = None;
            }
        }
        self
    }

    /// Marks an action or toggle item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        if self.kind != ToolbarItemKind::Separator && !reason.is_empty() {
            self.disabled = true;
            self.disabled_reason = Some(reason);
        }
        self
    }

    /// Marks a toggle item as pressed.
    pub fn pressed(mut self, pressed: bool) -> Self {
        if self.kind == ToolbarItemKind::Toggle {
            self.pressed = pressed;
        }
        self
    }

    /// Applies app-resolved icon metadata.
    pub fn icon(mut self, icon: ResolvedActionIcon) -> Self {
        if self.kind != ToolbarItemKind::Separator {
            self.icon = Some(icon);
        }
        self
    }

    /// Applies a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        if self.kind != ToolbarItemKind::Separator {
            self.shortcut = Some(shortcut.into());
        }
        self
    }

    /// Applies user-displayable tooltip metadata.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        let tooltip = tooltip.into();
        if self.kind != ToolbarItemKind::Separator && !tooltip.is_empty() {
            self.tooltip = Some(tooltip);
        }
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        if self.kind != ToolbarItemKind::Separator && !description.is_empty() {
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

    /// Returns a concrete render label for the resolved icon.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns the item kind.
    pub const fn kind(&self) -> ToolbarItemKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns whether the item is pressed.
    pub const fn pressed_state(&self) -> bool {
        self.pressed
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

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        !matches!(self.kind, ToolbarItemKind::Separator) && !self.disabled
    }
}

/// Resolved toolbar color intents.
pub type ToolbarColors = ButtonColors;

/// Resolved toolbar metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarMetrics {
    item: ButtonMetrics,
    gap: UiPx,
    separator_length: UiPx,
    separator_thickness: UiPx,
    padding: UiPx,
    radius: UiPx,
}

impl ToolbarMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            item: ButtonMetrics::from_size(size),
            gap: ui_px(4.0),
            separator_length: size.button_h(),
            separator_thickness: ui_px(1.0),
            padding: ui_px(4.0),
            radius: size.control_radius(),
        }
    }

    /// Returns item metrics.
    pub const fn item(self) -> ButtonMetrics {
        self.item
    }

    /// Returns the gap between toolbar items.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns the visual separator length.
    pub const fn separator_length(self) -> UiPx {
        self.separator_length
    }

    /// Returns the visual separator thickness.
    pub const fn separator_thickness(self) -> UiPx {
        self.separator_thickness
    }

    /// Returns toolbar padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns the toolbar corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }
}

/// Resolved toolbar item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolbarItemState {
    index: usize,
    value: String,
    label: String,
    icon: Option<ResolvedActionIcon>,
    kind: ToolbarItemKind,
    disabled: bool,
    disabled_reason: Option<String>,
    pressed: bool,
    focused: bool,
    shortcut: Option<String>,
    tooltip: Option<String>,
    accessibility_description: Option<String>,
}

impl ToolbarItemState {
    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
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
    pub const fn icon(&self) -> Option<&ResolvedActionIcon> {
        self.icon.as_ref()
    }

    /// Returns a concrete render label for the resolved icon.
    pub fn icon_label(&self) -> Option<&str> {
        self.icon.as_ref().and_then(ResolvedActionIcon::label)
    }

    /// Returns the item kind.
    pub const fn kind(&self) -> ToolbarItemKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the optional disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns whether the item can receive roving focus.
    pub const fn focusable(&self) -> bool {
        !matches!(self.kind, ToolbarItemKind::Separator) && !self.disabled
    }

    /// Returns whether the item is pressed.
    pub const fn pressed(&self) -> bool {
        self.pressed
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

    /// Returns whether the item has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        self.focusable()
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Option<Role> {
        match self.kind {
            ToolbarItemKind::Action | ToolbarItemKind::Toggle => Some(Role::Button),
            ToolbarItemKind::Separator => None,
        }
    }

    /// Returns the accessibility toggled state.
    pub const fn toggled(&self) -> Option<Toggled> {
        match self.kind {
            ToolbarItemKind::Toggle if self.pressed => Some(Toggled::True),
            ToolbarItemKind::Toggle => Some(Toggled::False),
            _ => None,
        }
    }
}

/// Resolved toolbar activation payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolbarSelection {
    index: usize,
    value: String,
    label: String,
    kind: ToolbarItemKind,
    pressed: bool,
}

impl ToolbarSelection {
    /// Creates a selection payload from an item state.
    pub fn from_item(item: &ToolbarItemState) -> Option<Self> {
        item.activation_enabled().then(|| Self {
            index: item.index,
            value: item.value.clone(),
            label: item.label.clone(),
            kind: item.kind,
            pressed: item.pressed,
        })
    }

    /// Returns the activated item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the activated item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the activated item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the activated item kind.
    pub const fn kind(&self) -> ToolbarItemKind {
        self.kind
    }

    /// Returns the current pressed state for toggle items.
    pub const fn pressed(&self) -> bool {
        self.pressed
    }
}

/// Resolved toolbar state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarState {
    orientation: Orientation,
    size: Size,
    disabled: bool,
    label: String,
    items: Vec<ToolbarItemState>,
    focused_index: Option<usize>,
    metrics: ToolbarMetrics,
    colors: ToolbarColors,
    focus_ring: FocusRing,
}

impl ToolbarState {
    /// Resolves public state for a toolbar.
    pub fn resolve(
        orientation: Orientation,
        size: Size,
        disabled: bool,
        label: impl Into<String>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = ToolbarItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<ToolbarItemDescriptor> = items.into_iter().collect();
        let collection = ChoiceCollection::resolve(
            disabled,
            toolbar_choice_items(disabled, &descriptors),
            None,
            focused_value,
            ChoiceInteractionPolicy::roving(orientation),
        );
        let focused_index = collection.active_index();
        let colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, false);

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| {
                let item_disabled = disabled || descriptor.disabled;
                let focused = Some(index) == focused_index;

                ToolbarItemState {
                    index,
                    value: descriptor.value,
                    label: descriptor.label,
                    icon: descriptor.icon,
                    kind: descriptor.kind,
                    disabled: item_disabled,
                    disabled_reason: descriptor.disabled_reason,
                    pressed: descriptor.pressed,
                    focused,
                    shortcut: descriptor.shortcut,
                    tooltip: descriptor.tooltip,
                    accessibility_description: descriptor.accessibility_description,
                }
            })
            .collect();

        Self {
            orientation,
            size,
            disabled,
            label: label.into(),
            items,
            focused_index,
            metrics: ToolbarMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the toolbar orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the whole toolbar is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether any activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the accessible toolbar label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Toolbar
    }

    /// Returns resolved toolbar items.
    pub fn items(&self) -> &[ToolbarItemState] {
        &self.items
    }

    /// Returns focused item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns focused item value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(ToolbarItemState::value)
    }

    /// Returns the current tab-stop index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Resolves a focus target for an APG-style toolbar navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<&ToolbarItemState> {
        let current = self.focused_index?;
        let disabled = self.disabled_map();
        toolbar_navigation_target(self.orientation, key, current, &disabled)
            .and_then(|index| self.items.get(index))
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<ToolbarSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.focused_index
            .and_then(|index| self.items.get(index))
            .and_then(ToolbarSelection::from_item)
    }

    /// Returns resolved toolbar metrics.
    pub const fn metrics(&self) -> ToolbarMetrics {
        self.metrics
    }

    /// Returns resolved toolbar colors.
    pub const fn colors(&self) -> ToolbarColors {
        self.colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.items.iter().map(|item| !item.focusable()).collect()
    }
}

/// Resolves a toolbar roving-focus target from an APG-style key name.
pub fn toolbar_navigation_target(
    orientation: Orientation,
    key: &str,
    current: usize,
    disabled: &[bool],
) -> Option<usize> {
    ChoiceInteractionPolicy::roving(orientation).navigation_target_index(key, current, disabled)
}

fn toolbar_choice_items(
    disabled: bool,
    items: &[ToolbarItemDescriptor],
) -> Vec<ChoiceItemProjection<()>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let label = item.label().to_owned();
            ChoiceItemProjection::new(
                index,
                None,
                item.value(),
                label.clone(),
                disabled || !item.focusable(),
                (),
            )
            .text_value(label)
        })
        .collect()
}

/// A concrete GPUI toolbar item.
#[derive(Clone)]
pub struct ToolbarItem {
    descriptor: ToolbarItemDescriptor,
    visible_label: Option<SharedString>,
    on_select: Option<Rc<dyn Fn(ToolbarSelection, &mut Window, &mut App)>>,
    tooltip: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
}

impl ToolbarItem {
    /// Creates an action item.
    pub fn action(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ToolbarItemDescriptor::action(value, label.to_string()),
            visible_label: Some(label),
            on_select: None,
            tooltip: None,
        }
    }

    /// Creates an action item from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::from_resolved_action(action),
            visible_label: action.icon_label().map(SharedString::from),
            on_select: None,
            tooltip: None,
        }
    }

    /// Creates an icon-only action item with an explicit accessible label.
    pub fn icon(
        value: impl Into<String>,
        icon: impl Into<SharedString>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::action(value, label),
            visible_label: Some(icon.into()),
            on_select: None,
            tooltip: None,
        }
    }

    /// Creates an icon-only toggle item with an explicit accessible label.
    pub fn toggle_icon(
        value: impl Into<String>,
        icon: impl Into<SharedString>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::toggle(value, label),
            visible_label: Some(icon.into()),
            on_select: None,
            tooltip: None,
        }
    }

    /// Creates a toggle item.
    pub fn toggle(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ToolbarItemDescriptor::toggle(value, label.to_string()),
            visible_label: Some(label),
            on_select: None,
            tooltip: None,
        }
    }

    /// Creates a separator item.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::separator(value),
            visible_label: None,
            on_select: None,
            tooltip: None,
        }
    }

    /// Marks the toolbar item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Marks the toolbar item as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.disabled_reason(reason);
        self
    }

    /// Applies app-resolved icon metadata.
    pub fn resolved_icon(mut self, icon: ResolvedActionIcon) -> Self {
        self.visible_label = icon.label().map(SharedString::from);
        self.descriptor = self.descriptor.icon(icon);
        self
    }

    /// Marks a toggle item as pressed.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.descriptor = self.descriptor.pressed(pressed);
        self
    }

    /// Applies a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Registers an item selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ToolbarSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Adds a hover/focus tooltip to a non-separator toolbar item.
    pub fn tooltip(mut self, tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        if self.descriptor.kind() != ToolbarItemKind::Separator {
            self.tooltip = Some(Rc::new(tooltip));
        }
        self
    }

    /// Adds a text tooltip to a non-separator toolbar item.
    pub fn tooltip_text(mut self, tooltip: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.tooltip(tooltip);
        self
    }

    /// Applies an accessibility description in addition to the visible label.
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.accessibility_description(description);
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> ToolbarItemDescriptor {
        self.descriptor.clone()
    }

    pub(crate) fn select_handler(
        &self,
    ) -> Option<Rc<dyn Fn(ToolbarSelection, &mut Window, &mut App)>> {
        self.on_select.clone()
    }
}

/// A concrete GPUI toolbar component.
#[derive(IntoElement)]
pub struct Toolbar {
    id: ElementId,
    label: SharedString,
    orientation: Orientation,
    focused_value: Option<String>,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<ToolbarItem>,
    on_select: Option<Rc<dyn Fn(ToolbarSelection, &mut Window, &mut App)>>,
}

impl Toolbar {
    /// Creates a new toolbar with an accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            orientation: Orientation::Horizontal,
            focused_value: None,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_select: None,
        }
    }

    /// Sets the toolbar orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Applies the default focused toolbar item value for adapter-owned runtime state.
    pub fn default_focused(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
        self
    }

    /// Marks the whole toolbar as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds one toolbar item.
    pub fn item(mut self, item: ToolbarItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many toolbar items.
    pub fn items(mut self, items: impl IntoIterator<Item = ToolbarItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Registers a toolbar-level selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ToolbarSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved state.
    pub fn state(&self) -> ToolbarState {
        ToolbarState::resolve(
            self.orientation,
            self.size,
            self.disabled,
            self.label.to_string(),
            self.focused_value.as_deref(),
            self.items.iter().map(ToolbarItem::descriptor),
            self.tokens,
        )
    }
}

impl Sizable for Toolbar {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let Toolbar {
            id,
            label,
            orientation,
            focused_value,
            disabled,
            size,
            tokens,
            items,
            on_select,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = id.to_string();
            let descriptors: Vec<ToolbarItemDescriptor> =
                items.iter().map(ToolbarItem::descriptor).collect();
            let focused_seed = focused_value.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| ToolbarRuntime {
                focused_value: focused_seed,
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = {
                let runtime = runtime.read(cx);
                runtime.focused_value.clone()
            };
            let state = ToolbarState::resolve(
                orientation,
                size,
                disabled,
                label.to_string(),
                runtime_snapshot.as_deref(),
                descriptors.clone(),
                tokens,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, &descriptors, cx));

            let item_descriptors = Rc::new(descriptors);
            let disabled_items = Rc::new(
                state
                    .items()
                    .iter()
                    .map(|item| !item.focusable())
                    .collect::<Vec<_>>(),
            );
            let metrics = state.metrics();
            let colors = state.colors();
            let pressed_colors = ThemeResolver::button_colors(tokens, ButtonVariant::Ghost, true);
            let focus_ring = state.focus_ring();
            let is_vertical = matches!(orientation, Orientation::Vertical);
            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let focusable_set_size = state.items().iter().filter(|item| item.focusable()).count();
            let mut focusable_position = 0usize;
            let tab_stop_index = state.tab_stop_index();

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("toolbar:{debug_id}")
                })
                .ui_role(state.role())
                .aria_label(label.clone())
                .ui_aria_orientation(orientation)
                .aria_disabled(state.disabled())
                .flex()
                .gap(gpui_px_from_ui(metrics.gap()))
                .p(gpui_px_from_ui(metrics.padding()))
                .rounded(gpui_px_from_ui(metrics.radius()))
                .border_1()
                .border_color(theme.resolve(colors.border()))
                .bg(theme.resolve(colors.background()))
                .when(is_vertical, |this| this.flex_col().items_stretch())
                .when(!is_vertical, |this| {
                    this.flex_row().items_center().flex_wrap()
                })
                .children(state.items().iter().enumerate().map(|(index, item)| {
                    let descriptor = item_descriptors[index].clone();
                    let visible_label = items[index]
                        .visible_label
                        .clone()
                        .or_else(|| item.icon_label().map(SharedString::from));
                    let item_tooltip = items[index].tooltip.clone();
                    let item_tooltip_text = item.tooltip().map(str::to_owned);
                    let click_item_handler = items[index].select_handler();
                    let key_item_handler = click_item_handler.clone();
                    let click_toolbar_handler = on_select.clone();
                    let key_toolbar_handler = click_toolbar_handler.clone();
                    let key_item_descriptors = item_descriptors.clone();
                    let disabled_items = disabled_items.clone();
                    let focus_handle = focus_handles[index].clone();
                    let key_runtime = runtime.clone();
                    let click_runtime = runtime.clone();
                    let item_index = index;
                    let item_kind = item.kind();
                    let item_disabled = item.disabled();
                    let item_tab_stop = Some(index) == tab_stop_index;
                    let item_pressed = item.pressed();
                    let item_value = item.value().to_owned();
                    let item_accessibility_description =
                        item.accessibility_description().map(str::to_owned);
                    let item_disabled_reason = item.disabled_reason_ref().map(str::to_owned);
                    let item_aria_label = item_accessibility_description
                        .as_ref()
                        .or(item_disabled_reason.as_ref())
                        .map_or_else(
                            || descriptor.label().to_owned(),
                            |description| format!("{}, {description}", descriptor.label()),
                        );
                    let item_position = if item.focusable() {
                        focusable_position += 1;
                        Some(focusable_position)
                    } else {
                        None
                    };
                    let separator_color = theme.resolve(colors.border());
                    let item_border = separator_color;
                    let item_background = theme.resolve(toolbar_item_background(
                        colors,
                        pressed_colors,
                        item_kind,
                        item_pressed,
                    ));
                    let item_foreground = theme.resolve(colors.foreground());
                    let item_hover_background = theme.resolve(colors.hover_background());
                    let item_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

                    if item.kind() == ToolbarItemKind::Separator {
                        return div()
                            .id(toolbar_item_id(item.value()))
                            .debug_selector({
                                let debug_id = debug_id.clone();
                                let item_value = item_value.clone();
                                move || format!("toolbar:{debug_id}:item:{item_value}")
                            })
                            .flex_none()
                            .bg(separator_color)
                            .when(is_vertical, |this| {
                                this.w_full()
                                    .h(gpui_px_from_ui(metrics.separator_thickness()))
                            })
                            .when(!is_vertical, |this| {
                                this.w(gpui_px_from_ui(metrics.separator_thickness()))
                                    .h(gpui_px_from_ui(metrics.separator_length()))
                            })
                            .into_any_element();
                    }

                    div()
                        .id(toolbar_item_id(item.value()))
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            let item_value = item_value.clone();
                            move || format!("toolbar:{debug_id}:item:{item_value}")
                        })
                        .focusable()
                        .tab_stop(item_tab_stop)
                        .ui_role(item.role().unwrap_or(Role::Button))
                        .aria_label(item_aria_label)
                        .aria_disabled(item_disabled)
                        .when_some(item_position, |this, position| {
                            this.aria_position_in_set(position)
                                .aria_size_of_set(focusable_set_size)
                        })
                        .when_some(item.toggled(), |this, toggled| {
                            this.ui_aria_toggled(toggled)
                        })
                        .when_some(focus_handle, |this, focus_handle| {
                            this.track_focus(&focus_handle)
                        })
                        .min_h(gpui_px_from_ui(metrics.item().height()))
                        .px(gpui_px_from_ui(metrics.item().padding_x()))
                        .py(gpui_px_from_ui(metrics.item().padding_y()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .rounded(gpui_px_from_ui(metrics.item().radius()))
                        .border_1()
                        .border_color(item_border)
                        .bg(item_background)
                        .text_size(gpui_px_from_ui(metrics.item().text_size()))
                        .line_height(gpui_px_from_ui(metrics.item().text_size()))
                        .text_color(item_foreground)
                        .focus_visible(move |style| style.shadow(item_focus_shadow.clone()))
                        .when(!item_disabled, |this| {
                            this.cursor_pointer()
                                .hover(move |style| style.bg(item_hover_background))
                        })
                        .when(item_disabled, |this| {
                            this.opacity(0.56).cursor_not_allowed()
                        })
                        .on_click({
                            let descriptor = descriptor.clone();
                            move |_event: &ClickEvent, window, cx| {
                                if disabled || descriptor.disabled_state() {
                                    return;
                                }

                                cx.stop_propagation();
                                let focus_handle = click_runtime.update(cx, |runtime, cx| {
                                    runtime.set_focused(descriptor.value(), cx)
                                });

                                if let Some(selection) =
                                    ToolbarSelection::from_descriptor(item_index, &descriptor)
                                {
                                    if let Some(handler) = click_item_handler.clone() {
                                        handler(selection.clone(), window, cx);
                                    }
                                    if let Some(handler) = click_toolbar_handler.clone() {
                                        handler(selection, window, cx);
                                    }
                                }

                                if let Some(focus_handle) = focus_handle {
                                    focus_handle.focus(window, cx);
                                }
                            }
                        })
                        .on_key_down({
                            let descriptor = descriptor.clone();
                            let disabled_items = disabled_items.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                if disabled || descriptor.disabled_state() {
                                    return;
                                }
                                if event.keystroke.modifiers.modified() {
                                    return;
                                }

                                let key = event.keystroke.key.as_str();
                                let Some(target_index) = toolbar_navigation_target(
                                    orientation,
                                    key,
                                    item_index,
                                    &disabled_items,
                                ) else {
                                    if !matches!(key, "space" | "enter") {
                                        return;
                                    }

                                    if let Some(selection) =
                                        ToolbarSelection::from_descriptor(item_index, &descriptor)
                                    {
                                        if let Some(handler) = key_item_handler.clone() {
                                            handler(selection.clone(), window, cx);
                                        }
                                        if let Some(handler) = key_toolbar_handler.clone() {
                                            handler(selection, window, cx);
                                        }
                                    }
                                    cx.stop_propagation();
                                    return;
                                };

                                let target = &key_item_descriptors[target_index];
                                let target_value = target.value().to_owned();
                                let focus_handle = key_runtime.update(cx, |runtime, cx| {
                                    runtime.set_focused(&target_value, cx)
                                });

                                if let Some(focus_handle) = focus_handle {
                                    focus_handle.focus(window, cx);
                                }

                                cx.stop_propagation();
                            }
                        })
                        .when_some(item_tooltip, |this, tooltip| {
                            this.tooltip(move |window, cx| tooltip(window, cx))
                        })
                        .when_some(item_tooltip_text, |this, tooltip| {
                            this.tooltip(Tooltip::text(tooltip))
                        })
                        .child(visible_label.unwrap_or_else(|| descriptor.label().into()))
                        .into_any_element()
                }))
        })
    }
}

#[derive(Debug, Default)]
struct ToolbarRuntime {
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl ToolbarRuntime {
    fn sync(
        &mut self,
        state: &ToolbarState,
        items: &[ToolbarItemDescriptor],
        cx: &mut Context<Self>,
    ) {
        self.focus_handles.retain(|value, _| {
            items
                .iter()
                .any(|item| item.value() == value && item.focusable())
        });

        for item in items.iter().filter(|item| item.focusable()) {
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

fn toolbar_item_background(
    colors: ToolbarColors,
    pressed_colors: ToolbarColors,
    kind: ToolbarItemKind,
    pressed: bool,
) -> ColorIntent {
    match kind {
        ToolbarItemKind::Toggle if pressed => pressed_colors.background(),
        _ => colors.background(),
    }
}

impl ToolbarSelection {
    fn from_descriptor(index: usize, descriptor: &ToolbarItemDescriptor) -> Option<Self> {
        descriptor.focusable().then(|| Self {
            index,
            value: descriptor.value.clone(),
            label: descriptor.label.clone(),
            kind: descriptor.kind,
            pressed: descriptor.pressed,
        })
    }
}

fn toolbar_item_id(value: &str) -> ElementId {
    format!("toolbar-item-{value}").into()
}
