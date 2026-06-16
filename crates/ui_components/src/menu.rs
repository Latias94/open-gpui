//! Menu component and shared menu state.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, OverlayPresence, Role,
    Sizable, Size, ThemeTokens,
};

use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::overlay::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState,
    outside_press_open_change, ui_point_from_gpui, ui_px_from_gpui, ui_size_from_gpui,
};
use crate::roving_focus::{first_enabled, last_enabled, next_enabled};
use crate::theme::ThemeResolver;

/// Menu open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Menu item kind for the base menu model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    /// Activatable command item.
    Action,
    /// Visual separator. Separators are not focusable or activatable.
    Separator,
}

/// Pure descriptor for one menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemDescriptor {
    value: String,
    label: String,
    kind: MenuItemKind,
    disabled: bool,
}

impl MenuItemDescriptor {
    /// Creates an action item descriptor.
    pub fn action(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Action,
            disabled: false,
        }
    }

    /// Creates a separator descriptor.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: String::new(),
            kind: MenuItemKind::Separator,
            disabled: true,
        }
    }

    /// Marks an action item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if self.kind == MenuItemKind::Action {
            self.disabled = disabled;
        }
        self
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the menu item kind.
    pub const fn kind(&self) -> MenuItemKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        matches!(self.kind, MenuItemKind::Action) && !self.disabled
    }
}

/// Resolved menu color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuColors {
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) item_background: ColorIntent,
    pub(crate) item_hover_background: ColorIntent,
    pub(crate) item_focus_background: ColorIntent,
    pub(crate) item_disabled_foreground: ColorIntent,
    pub(crate) separator: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl MenuColors {
    /// Returns menu surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns menu foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns menu border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns default menu item background color intent.
    pub const fn item_background(self) -> ColorIntent {
        self.item_background
    }

    /// Returns hovered menu item background color intent.
    pub const fn item_hover_background(self) -> ColorIntent {
        self.item_hover_background
    }

    /// Returns focused menu item background color intent.
    pub const fn item_focus_background(self) -> ColorIntent {
        self.item_focus_background
    }

    /// Returns disabled menu item foreground color intent.
    pub const fn item_disabled_foreground(self) -> ColorIntent {
        self.item_disabled_foreground
    }

    /// Returns separator color intent.
    pub const fn separator(self) -> ColorIntent {
        self.separator
    }

    /// Returns trigger background color intent.
    pub const fn trigger_background(self) -> ColorIntent {
        self.trigger_background
    }

    /// Returns trigger hover background color intent.
    pub const fn trigger_hover_background(self) -> ColorIntent {
        self.trigger_hover_background
    }

    /// Returns trigger foreground color intent.
    pub const fn trigger_foreground(self) -> ColorIntent {
        self.trigger_foreground
    }

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved menu metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuMetrics {
    trigger_height: open_gpui::Pixels,
    trigger_padding_x: open_gpui::Pixels,
    trigger_padding_y: open_gpui::Pixels,
    surface_padding: open_gpui::Pixels,
    item_height: open_gpui::Pixels,
    item_padding_x: open_gpui::Pixels,
    item_padding_y: open_gpui::Pixels,
    separator_height: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
    min_width: open_gpui::Pixels,
    max_width: open_gpui::Pixels,
}

impl MenuMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            surface_padding: px(6.0),
            item_height: size.button_h(),
            item_padding_x: size.button_px(),
            item_padding_y: px(6.0),
            separator_height: px(1.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: px(180.0),
            max_width: px(320.0),
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> open_gpui::Pixels {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> open_gpui::Pixels {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> open_gpui::Pixels {
        self.trigger_padding_y
    }

    /// Returns menu surface padding.
    pub const fn surface_padding(self) -> open_gpui::Pixels {
        self.surface_padding
    }

    /// Returns menu item height.
    pub const fn item_height(self) -> open_gpui::Pixels {
        self.item_height
    }

    /// Returns menu item horizontal padding.
    pub const fn item_padding_x(self) -> open_gpui::Pixels {
        self.item_padding_x
    }

    /// Returns menu item vertical padding.
    pub const fn item_padding_y(self) -> open_gpui::Pixels {
        self.item_padding_y
    }

    /// Returns separator height.
    pub const fn separator_height(self) -> open_gpui::Pixels {
        self.separator_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> open_gpui::Pixels {
        self.text_size
    }

    /// Returns minimum menu width.
    pub const fn min_width(self) -> open_gpui::Pixels {
        self.min_width
    }

    /// Returns maximum menu width.
    pub const fn max_width(self) -> open_gpui::Pixels {
        self.max_width
    }
}

/// Resolved menu item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemState {
    index: usize,
    value: String,
    label: String,
    kind: MenuItemKind,
    disabled: bool,
    focused: bool,
    tab_stop: bool,
}

impl MenuItemState {
    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the item kind.
    pub const fn kind(&self) -> MenuItemKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item can receive roving focus.
    pub const fn focusable(&self) -> bool {
        matches!(self.kind, MenuItemKind::Action) && !self.disabled
    }

    /// Returns whether the item has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether the item is the current tab stop.
    pub const fn tab_stop(&self) -> bool {
        self.tab_stop
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        self.focusable()
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Option<Role> {
        match self.kind {
            MenuItemKind::Action => Some(Role::MenuItem),
            MenuItemKind::Separator => None,
        }
    }
}

/// Resolved menu selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSelection {
    index: usize,
    value: String,
    label: String,
}

impl MenuSelection {
    /// Creates a selection payload from an item state.
    pub fn from_item(item: &MenuItemState) -> Option<Self> {
        item.activation_enabled().then(|| Self {
            index: item.index,
            value: item.value.clone(),
            label: item.label.clone(),
        })
    }

    /// Returns the selected item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected item label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved menu state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: MenuOpenMode,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    items: Vec<MenuItemState>,
    focused_index: Option<usize>,
    metrics: MenuMetrics,
    colors: MenuColors,
    focus_ring: FocusRing,
    overlay: GpuiOverlayState,
}

impl MenuState {
    /// Resolves public state for a menu.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = MenuItemDescriptor>,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = if open.is_some() {
            MenuOpenMode::Controlled
        } else {
            MenuOpenMode::Uncontrolled
        };
        let descriptors: Vec<MenuItemDescriptor> = items.into_iter().collect();
        let requested_open = open.unwrap_or(default_open);
        let open = requested_open && !disabled && !descriptors.is_empty();
        let disabled_map: Vec<bool> = descriptors.iter().map(|item| !item.focusable()).collect();
        let focused_index = if open {
            focused_value
                .and_then(|value| {
                    descriptors
                        .iter()
                        .position(|item| item.value() == value && item.focusable())
                })
                .or_else(|| first_enabled(&disabled_map))
        } else {
            None
        };
        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let focused = focused_index == Some(index);
                let focusable = item.focusable();
                MenuItemState {
                    index,
                    value: item.value,
                    label: item.label,
                    kind: item.kind,
                    disabled: item.disabled,
                    focused,
                    tab_stop: focused && focusable,
                }
            })
            .collect();
        let presence = if open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay = GpuiOverlayAdapterConfig::new(OverlayLayerKind::Menu, presence)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .snap_margin(DEFAULT_OVERLAY_SAFE_MARGIN)
            .state();
        let colors = ThemeResolver::menu_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            placement_side,
            placement_alignment,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            items,
            focused_index,
            metrics: MenuMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns menu size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the menu trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether menu content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns the uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> MenuOpenMode {
        self.open_mode
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

    /// Returns Escape-key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key_policy
    }

    /// Returns initial focus intent.
    pub const fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub const fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns whether the trigger should appear selected.
    pub const fn trigger_selected(&self) -> bool {
        self.trigger_selected
    }

    /// Returns resolved menu items.
    pub fn items(&self) -> &[MenuItemState] {
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
            .map(MenuItemState::value)
    }

    /// Returns current tab-stop item value.
    pub fn tab_stop_value(&self) -> Option<&str> {
        self.items
            .iter()
            .find(|item| item.tab_stop())
            .map(MenuItemState::value)
    }

    /// Resolves a focus target for an APG-style menu navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<&MenuItemState> {
        let current = self.focused_index?;
        let disabled = self.disabled_map();
        menu_navigation_target(key, current, &disabled).and_then(|index| self.items.get(index))
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<MenuSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.focused_index
            .and_then(|index| self.items.get(index))
            .and_then(MenuSelection::from_item)
    }

    /// Returns trigger accessibility role.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns content accessibility role.
    pub const fn content_role(&self) -> Role {
        Role::Menu
    }

    /// Returns resolved menu metrics.
    pub const fn metrics(&self) -> MenuMetrics {
        self.metrics
    }

    /// Returns resolved menu colors.
    pub const fn colors(&self) -> MenuColors {
        self.colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns resolved overlay adapter state.
    pub const fn overlay(&self) -> &GpuiOverlayState {
        &self.overlay
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.items.iter().map(|item| !item.focusable()).collect()
    }
}

/// Resolves a menu roving-focus target from an APG-style key name.
pub fn menu_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    match key {
        "home" => first_enabled(disabled),
        "end" => last_enabled(disabled),
        "up" => next_enabled(disabled, current, false, true),
        "down" => next_enabled(disabled, current, true, true),
        _ => None,
    }
}

/// A concrete GPUI menu item.
#[derive(Clone)]
pub struct MenuItem {
    descriptor: MenuItemDescriptor,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

impl MenuItem {
    /// Creates an action menu item.
    pub fn action(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::action(value, label.to_string()),
            on_select: None,
        }
    }

    /// Creates a separator item.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: MenuItemDescriptor::separator(value),
            on_select: None,
        }
    }

    /// Marks the menu item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Registers an item selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(MenuSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns a pure descriptor for this item.
    pub fn descriptor(&self) -> MenuItemDescriptor {
        self.descriptor.clone()
    }

    pub(crate) fn select_handler(
        &self,
    ) -> Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>> {
        self.on_select.clone()
    }
}

/// A concrete GPUI menu component.
#[derive(IntoElement)]
pub struct Menu {
    id: ElementId,
    trigger_label: SharedString,
    items: Vec<MenuItem>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    focused_value: Option<String>,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

#[derive(Debug, Clone)]
struct MenuRuntime {
    open: bool,
    focused_value: Option<String>,
}

impl Menu {
    /// Creates a menu with a trigger label.
    pub fn new(id: impl Into<ElementId>, trigger_label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            items: Vec::new(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            focused_value: None,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_escape_close: None,
            on_open_change: None,
            on_select: None,
        }
    }

    /// Adds one menu item.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many menu items.
    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Marks the menu trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies the initially focused item value.
    pub fn focused_value(mut self, value: impl Into<String>) -> Self {
        self.focused_value = Some(value.into());
        self
    }

    /// Applies preferred placement.
    pub fn placement(
        mut self,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
    ) -> Self {
        self.placement_side = side;
        self.placement_alignment = alignment;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape-key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        let handler = Rc::new(handler);
        self.on_escape_close = Some(handler.clone());
        self.on_open_change = Some(handler);
        self
    }

    /// Registers a menu selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(MenuSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved menu state.
    pub fn state(&self) -> MenuState {
        MenuState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.focused_value.as_deref(),
            self.items.iter().map(MenuItem::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Menu {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Menu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let descriptors: Vec<MenuItemDescriptor> =
            self.items.iter().map(MenuItem::descriptor).collect();
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| MenuRuntime {
            open: self.default_open,
            focused_value: self.focused_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_state.open);

        if controlled_open.is_some() && runtime_state.open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let focused_value = self
            .focused_value
            .as_deref()
            .or(runtime_state.focused_value.as_deref());
        let state = MenuState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            focused_value,
            descriptors.clone(),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let first_focusable_value = first_focusable_descriptor_value(&descriptors);
        let id = self.id;
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_label = self.trigger_label;
        let items = self.items;
        let on_escape_close = self.on_escape_close;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                open_gpui_ui_core::OverlayAnchorInput::from_layout_bounds(open_gpui_ui_core::rect(
                    ui_point_from_gpui(point(px(0.0), px(0.0))),
                    ui_size_from_gpui(metrics.min_width(), metrics.trigger_height()),
                )),
                ui_size_from_gpui(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px_from_gpui(px(4.0))),
            state.overlay().snap_margin(),
        );

        div()
            .id(id)
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(trigger_id)
                    .min_h(metrics.trigger_height())
                    .px(metrics.trigger_padding_x())
                    .py(metrics.trigger_padding_y())
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(metrics.radius())
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.trigger_border()))
                    .bg(ThemeResolver::resolve(colors.trigger_background()))
                    .text_color(ThemeResolver::resolve(colors.trigger_foreground()))
                    .text_size(metrics.text_size())
                    .line_height(metrics.text_size())
                    .focusable()
                    .tab_stop(!disabled)
                    .role(state.trigger_role())
                    .aria_label(trigger_label.clone())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_escape_close = on_escape_close.clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                cx.stop_propagation();
                                window.prevent_default();
                                close_menu(runtime.clone(), on_escape_close.clone(), window, cx);
                            }
                        })
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let first_focusable_value = first_focusable_value.clone();
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = next_open;
                                    runtime.focused_value =
                                        next_open.then(|| first_focusable_value.clone()).flatten();
                                });
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(next_open, window, cx);
                                }
                            })
                    })
                    .child(trigger_label),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .anchor(placement.anchor())
                            .offset(placement.offset())
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(menu_content_element(
                                items,
                                content_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                on_escape_close.clone(),
                                on_open_change.clone(),
                                on_select.clone(),
                            )),
                    )
                    .priority(state.overlay().deferred_priority()),
                )
            })
    }
}

fn menu_content_element(
    items: Vec<MenuItem>,
    content_id: ElementId,
    state: MenuState,
    runtime: open_gpui::Entity<MenuRuntime>,
    on_escape_close: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let key_state = state.clone();
    let key_runtime = runtime.clone();
    let key_open_change = on_open_change.clone();
    let key_select = on_select.clone();
    let escape_runtime = runtime.clone();
    let escape_open_change = on_escape_close.clone();

    div()
        .id(content_id)
        .min_w(metrics.min_width())
        .max_w(metrics.max_width())
        .p(metrics.surface_padding())
        .flex()
        .flex_col()
        .gap_1()
        .rounded(metrics.radius())
        .border_1()
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.surface()))
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .text_size(metrics.text_size())
        .line_height(metrics.text_size())
        .shadow_lg()
        .occlude()
        .tab_group()
        .focusable()
        .role(state.content_role())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if key == "escape" {
                cx.stop_propagation();
                window.prevent_default();
                close_menu(
                    escape_runtime.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
                return;
            }

            if let Some(target) = key_state.navigation_target(key) {
                cx.stop_propagation();
                window.prevent_default();
                let value = target.value().to_owned();
                key_runtime.update(cx, |runtime, _| {
                    runtime.focused_value = Some(value);
                });
                return;
            }

            if let Some(selection) = key_state.activation_for_key(key) {
                cx.stop_propagation();
                window.prevent_default();
                if let Some(on_select) = key_select.as_ref() {
                    on_select(selection, window, cx);
                }
                close_menu(key_runtime.clone(), key_open_change.clone(), window, cx);
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_menu(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .children(menu_item_elements(
            items,
            state,
            runtime,
            on_open_change,
            on_select,
        ))
}

fn menu_item_elements(
    items: Vec<MenuItem>,
    state: MenuState,
    runtime: open_gpui::Entity<MenuRuntime>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> Vec<AnyElement> {
    let metrics = state.metrics();
    let colors = state.colors();
    let states = state.items().to_vec();

    items
        .into_iter()
        .zip(states)
        .map(|(item, item_state)| match item_state.kind() {
            MenuItemKind::Separator => div()
                .id(format!("menu-separator:{}", item_state.index()))
                .h(metrics.separator_height())
                .my_1()
                .bg(ThemeResolver::resolve(colors.separator()))
                .into_any_element(),
            MenuItemKind::Action => {
                let selection = MenuSelection::from_item(&item_state);
                let item_handler = item.on_select.clone();
                let global_handler = on_select.clone();
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let focused = item_state.focused();
                let disabled = item_state.disabled();
                div()
                    .id(format!("menu-item:{}", item_state.value()))
                    .min_h(metrics.item_height())
                    .px(metrics.item_padding_x())
                    .py(metrics.item_padding_y())
                    .flex()
                    .items_center()
                    .rounded(metrics.radius())
                    .bg(ThemeResolver::resolve(if focused {
                        colors.item_focus_background()
                    } else {
                        colors.item_background()
                    }))
                    .text_color(ThemeResolver::resolve(if disabled {
                        colors.item_disabled_foreground()
                    } else {
                        colors.foreground()
                    }))
                    .role(Role::MenuItem)
                    .aria_label(item_state.label().to_owned())
                    .aria_disabled(disabled)
                    .focusable()
                    .tab_stop(item_state.tab_stop())
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.item_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let Some(selection) = selection.clone() else {
                                    return;
                                };
                                if let Some(item_handler) = item_handler.as_ref() {
                                    item_handler(selection.clone(), window, cx);
                                }
                                if let Some(global_handler) = global_handler.as_ref() {
                                    global_handler(selection, window, cx);
                                }
                                close_menu(runtime.clone(), on_open_change.clone(), window, cx);
                            })
                    })
                    .child(item_state.label().to_owned())
                    .into_any_element()
            }
        })
        .collect()
}

fn close_menu(
    runtime: open_gpui::Entity<MenuRuntime>,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
        runtime.focused_value = None;
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
}

fn first_focusable_descriptor_value(items: &[MenuItemDescriptor]) -> Option<String> {
    items
        .iter()
        .find(|item| item.focusable())
        .map(|item| item.value().to_owned())
}

impl ThemeResolver {
    pub(crate) const fn menu_colors(tokens: ThemeTokens, open: bool) -> MenuColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        MenuColors {
            surface: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            border: ColorIntent::new(tokens.border, 0xcfd5cc),
            item_background: ColorIntent::new(tokens.surface, 0xffffff),
            item_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            item_focus_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::FocusVisible,
                0xe8ede6,
            ),
            item_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                0x7a8491,
            ),
            separator: ColorIntent::new(tokens.border, 0xcfd5cc),
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                0xf6f7f2,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, 0x18202a),
            trigger_border: ColorIntent::new(tokens.border, 0xcfd5cc),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }
}
