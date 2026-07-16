//! Toolbar component.

mod render;

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{AnyView, App, ElementId, IntoElement, SharedString, Window};
use open_gpui_ui_core::{Orientation, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px};

use crate::action::{ResolvedActionIcon, ResolvedActionState};
use crate::activation::{Activation, ActivationHandle, ActivationKeyPolicy};
use crate::button::{ButtonColors, ButtonMetrics, ButtonVariant};
use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};
use crate::focus::FocusRing;
use crate::theme::ThemeResolver;

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
    duplicate_value: bool,
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

    /// Returns whether this item shares its stable value with another item.
    ///
    /// Duplicate values fail closed because value-addressed focus and programmatic activation
    /// would otherwise be ambiguous.
    pub const fn duplicate_value(&self) -> bool {
        self.duplicate_value
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
pub struct ToolbarActivation {
    index: usize,
    value: String,
    label: String,
    kind: ToolbarItemKind,
    pressed: bool,
}

impl ToolbarActivation {
    fn for_item(item: &ToolbarItemState) -> Self {
        Self {
            index: item.index,
            value: item.value.clone(),
            label: item.label.clone(),
            kind: item.kind,
            pressed: item.pressed,
        }
    }

    /// Creates an activation payload from an item state.
    pub fn from_item(item: &ToolbarItemState) -> Option<Self> {
        item.activation_enabled().then(|| Self::for_item(item))
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

    /// Returns the caller-owned pressed state before activation for toggle items.
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
        let duplicate_values = {
            let value_counts = descriptors
                .iter()
                .fold(BTreeMap::new(), |mut counts, item| {
                    *counts.entry(item.value()).or_insert(0usize) += 1;
                    counts
                });
            descriptors
                .iter()
                .map(|item| {
                    value_counts
                        .get(item.value())
                        .is_some_and(|count| *count > 1)
                })
                .collect::<Vec<_>>()
        };
        let collection = ChoiceCollection::resolve(
            disabled,
            toolbar_choice_items(disabled, descriptors, duplicate_values),
            None,
            focused_value,
            ChoiceInteractionPolicy::roving(orientation),
        );
        let focused_index = collection.active_index();
        let colors = ThemeResolver::button_colors(tokens, ButtonVariant::Outline, false);

        let items = collection
            .into_items()
            .into_iter()
            .map(|projection| {
                let index = projection.source_index();
                let item_disabled = !projection.enabled();
                let (descriptor, duplicate_value) = projection.into_item();
                let focused = Some(index) == focused_index;

                ToolbarItemState {
                    index,
                    value: descriptor.value,
                    label: descriptor.label,
                    icon: descriptor.icon,
                    kind: descriptor.kind,
                    disabled: item_disabled,
                    disabled_reason: descriptor.disabled_reason,
                    duplicate_value,
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
    pub fn activation_for_key(&self, key: &str) -> Option<ToolbarActivation> {
        let item = self.focused_index.and_then(|index| self.items.get(index))?;
        let policy = toolbar_activation_key_policy(item.kind())?;
        (policy.accepts(key) && item.activation_enabled())
            .then(|| ToolbarActivation::for_item(item))
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
    items: Vec<ToolbarItemDescriptor>,
    duplicate_values: Vec<bool>,
) -> Vec<ChoiceItemProjection<(ToolbarItemDescriptor, bool)>> {
    items
        .into_iter()
        .zip(duplicate_values)
        .enumerate()
        .map(|(index, (item, duplicate_value))| {
            let value = item.value().to_owned();
            let label = item.label().to_owned();
            ChoiceItemProjection::new(
                index,
                None,
                value,
                label.clone(),
                disabled || !item.focusable() || duplicate_value,
                (item, duplicate_value),
            )
            .text_value(label)
        })
        .collect()
}

fn toolbar_activation_key_policy(kind: ToolbarItemKind) -> Option<ActivationKeyPolicy> {
    match kind {
        ToolbarItemKind::Action => Some(ActivationKeyPolicy::EnterOrSpace),
        ToolbarItemKind::Toggle => Some(ActivationKeyPolicy::Space),
        ToolbarItemKind::Separator => None,
    }
}

/// A concrete GPUI toolbar item.
#[derive(Clone)]
pub struct ToolbarItem {
    descriptor: ToolbarItemDescriptor,
    visible_label: Option<SharedString>,
    on_activate: Option<ToolbarActivationHandler>,
    tooltip: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
}

type ToolbarActivationHandler = Rc<dyn Fn(ToolbarActivation, Activation, &mut Window, &mut App)>;

impl ToolbarItem {
    /// Creates an action item.
    pub fn action(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ToolbarItemDescriptor::action(value, label.to_string()),
            visible_label: Some(label),
            on_activate: None,
            tooltip: None,
        }
    }

    /// Creates an action item from resolved action metadata.
    pub fn from_resolved_action(action: &ResolvedActionState) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::from_resolved_action(action),
            visible_label: action.icon_label().map(SharedString::from),
            on_activate: None,
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
            on_activate: None,
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
            on_activate: None,
            tooltip: None,
        }
    }

    /// Creates a toggle item.
    pub fn toggle(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ToolbarItemDescriptor::toggle(value, label.to_string()),
            visible_label: Some(label),
            on_activate: None,
            tooltip: None,
        }
    }

    /// Creates a separator item.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: ToolbarItemDescriptor::separator(value),
            visible_label: None,
            on_activate: None,
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

    /// Registers this item's activation handler.
    ///
    /// An item handler takes precedence over the toolbar-level fallback so one activation invokes
    /// exactly one domain callback.
    pub fn on_activate(
        mut self,
        handler: impl Fn(ToolbarActivation, Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
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
    on_activate: Option<ToolbarActivationHandler>,
    activation_handles: BTreeMap<String, ActivationHandle>,
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
            on_activate: None,
            activation_handles: BTreeMap::new(),
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

    /// Registers the fallback activation handler for items without their own handler.
    pub fn on_activate(
        mut self,
        handler: impl Fn(ToolbarActivation, Activation, &mut Window, &mut App) + 'static,
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
