//! Menu component and shared menu state.

use crate::geometry::gpui_px_from_ui;
use crate::geometry::{ui_point_from_gpui, ui_size_from_gpui_size};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementInput,
    OverlayPlacementSide, Rect, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_point, ui_px,
    ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::menu_runtime::{
    MenuRuntime, handle_menu_submenu_surface_hover, update_menu_hover_target,
};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayDisclosureConfig, OverlayDisclosureOpenMode, OverlayResolvedState,
    consume_overlay_event, emit_overlay_open_change, gpui_overlay_state, outside_press_open_change,
    resolve_overlay_open_state, restore_overlay_focus, set_overlay_open,
};
use crate::roving_focus::{typeahead_target, vertical_roving_navigation_target};
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;

/// Default threshold where menu surfaces become locally scrollable.
pub const DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD: usize = 8;

/// Menu open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl MenuOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

const fn menu_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> MenuOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => MenuOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => MenuOpenMode::Controlled,
    }
}

/// Menu item kind for the base menu model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    /// Activatable command item.
    Action,
    /// Checkable menu item. Checked state is caller-owned.
    Checkbox,
    /// Radio-style menu item. Checked state is caller-owned.
    Radio,
    /// Visual separator. Separators are not focusable or activatable.
    Separator,
    /// Submenu trigger item.
    Submenu,
}

impl MenuItemKind {
    /// Returns a stable item-kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Separator => "separator",
            Self::Submenu => "submenu",
        }
    }

    /// Returns whether this kind can be activated when enabled.
    pub const fn activatable(self) -> bool {
        matches!(self, Self::Action | Self::Checkbox | Self::Radio)
    }

    /// Returns whether this kind can receive roving focus when enabled.
    pub const fn focusable(self) -> bool {
        matches!(
            self,
            Self::Action | Self::Checkbox | Self::Radio | Self::Submenu
        )
    }
}

/// Pure descriptor for one menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemDescriptor {
    value: String,
    label: String,
    kind: MenuItemKind,
    disabled: bool,
    checked: bool,
    children: Vec<MenuItemDescriptor>,
}

impl MenuItemDescriptor {
    /// Creates an action item descriptor.
    pub fn action(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Action,
            disabled: false,
            checked: false,
            children: Vec::new(),
        }
    }

    /// Creates a checkbox item descriptor.
    pub fn checkbox(value: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Checkbox,
            disabled: false,
            checked,
            children: Vec::new(),
        }
    }

    /// Creates a radio item descriptor.
    pub fn radio(value: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Radio,
            disabled: false,
            checked,
            children: Vec::new(),
        }
    }

    /// Creates a separator descriptor.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: String::new(),
            kind: MenuItemKind::Separator,
            disabled: true,
            checked: false,
            children: Vec::new(),
        }
    }

    /// Creates a submenu descriptor.
    pub fn submenu(
        value: impl Into<String>,
        label: impl Into<String>,
        children: impl IntoIterator<Item = MenuItemDescriptor>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Submenu,
            disabled: false,
            checked: false,
            children: children.into_iter().collect(),
        }
    }

    /// Marks an activatable or submenu item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if self.kind != MenuItemKind::Separator {
            self.disabled = disabled;
        }
        self
    }

    /// Applies caller-owned checked state to checkbox and radio items.
    pub fn checked(mut self, checked: bool) -> Self {
        if matches!(self.kind, MenuItemKind::Checkbox | MenuItemKind::Radio) {
            self.checked = checked;
        }
        self
    }

    /// Adds one submenu child descriptor.
    pub fn child(mut self, child: MenuItemDescriptor) -> Self {
        if self.kind == MenuItemKind::Submenu {
            self.children.push(child);
        }
        self
    }

    /// Adds many submenu child descriptors.
    pub fn children(mut self, children: impl IntoIterator<Item = MenuItemDescriptor>) -> Self {
        if self.kind == MenuItemKind::Submenu {
            self.children.extend(children);
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

    /// Returns caller-owned checked state for checkbox and radio items.
    pub const fn checked_state(&self) -> bool {
        self.checked
    }

    /// Returns submenu child descriptors.
    pub fn children_ref(&self) -> &[MenuItemDescriptor] {
        &self.children
    }

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        self.kind.focusable()
            && !self.disabled
            && (!matches!(self.kind, MenuItemKind::Submenu) || !self.children.is_empty())
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
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    surface_padding: UiPx,
    item_height: UiPx,
    item_padding_x: UiPx,
    item_padding_y: UiPx,
    separator_height: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    submenu_indent: UiPx,
}

impl MenuMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            surface_padding: ui_px(6.0),
            item_height: size.button_h(),
            item_padding_x: size.button_px(),
            item_padding_y: ui_px(6.0),
            separator_height: ui_px(1.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(180.0),
            max_width: ui_px(320.0),
            max_height: ui_px(280.0),
            submenu_indent: match size {
                Size::XSmall | Size::Small => ui_px(14.0),
                Size::Medium | Size::Large => ui_px(18.0),
            },
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> UiPx {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> UiPx {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> UiPx {
        self.trigger_padding_y
    }

    /// Returns menu surface padding.
    pub const fn surface_padding(self) -> UiPx {
        self.surface_padding
    }

    /// Returns menu item height.
    pub const fn item_height(self) -> UiPx {
        self.item_height
    }

    /// Returns menu item horizontal padding.
    pub const fn item_padding_x(self) -> UiPx {
        self.item_padding_x
    }

    /// Returns menu item vertical padding.
    pub const fn item_padding_y(self) -> UiPx {
        self.item_padding_y
    }

    /// Returns separator height.
    pub const fn separator_height(self) -> UiPx {
        self.separator_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum menu width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum menu width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum menu surface height before local scrolling.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns additional indentation per submenu depth.
    pub const fn submenu_indent(self) -> UiPx {
        self.submenu_indent
    }
}

/// Resolved menu item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemState {
    index: usize,
    parent_value: Option<String>,
    path: Vec<String>,
    depth: usize,
    value: String,
    label: String,
    kind: MenuItemKind,
    disabled: bool,
    checked: bool,
    focused: bool,
    submenu_open: bool,
    child_count: usize,
    children: Vec<MenuItemState>,
}

impl MenuItemState {
    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the parent submenu value, if this item is nested.
    pub fn parent_value(&self) -> Option<&str> {
        self.parent_value.as_deref()
    }

    /// Returns the stable tree path for this item.
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Returns the stable tree path as a compact key.
    pub fn path_key(&self) -> String {
        self.path.join("/")
    }

    /// Returns zero-based menu depth.
    pub const fn depth(&self) -> usize {
        self.depth
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

    /// Returns caller-owned checked state for checkbox and radio items.
    pub const fn checked(&self) -> bool {
        self.checked
    }

    /// Returns toggle metadata for checkbox and radio rows.
    pub const fn toggled(&self) -> Option<Toggled> {
        match self.kind {
            MenuItemKind::Checkbox | MenuItemKind::Radio => Some(if self.checked {
                Toggled::True
            } else {
                Toggled::False
            }),
            _ => None,
        }
    }

    /// Returns whether the item can receive roving focus.
    pub const fn focusable(&self) -> bool {
        self.kind.focusable()
            && !self.disabled
            && (!matches!(self.kind, MenuItemKind::Submenu) || self.child_count > 0)
    }

    /// Returns whether the item has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns whether activation handlers should run for this item.
    pub const fn activation_enabled(&self) -> bool {
        self.focusable() && self.kind.activatable()
    }

    /// Returns whether this item owns a submenu.
    pub const fn has_submenu(&self) -> bool {
        matches!(self.kind, MenuItemKind::Submenu) && self.child_count > 0
    }

    /// Returns whether this submenu branch is open.
    pub const fn submenu_open(&self) -> bool {
        self.submenu_open
    }

    /// Returns number of direct submenu children.
    pub const fn child_count(&self) -> usize {
        self.child_count
    }

    /// Returns direct submenu child states.
    pub fn children(&self) -> &[MenuItemState] {
        &self.children
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Option<Role> {
        match self.kind {
            MenuItemKind::Action
            | MenuItemKind::Checkbox
            | MenuItemKind::Radio
            | MenuItemKind::Submenu => Some(Role::MenuItem),
            MenuItemKind::Separator => None,
        }
    }
}

/// Resolved menu selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSelection {
    index: usize,
    path: Vec<String>,
    value: String,
    label: String,
    kind: MenuItemKind,
    checked: bool,
}

impl MenuSelection {
    /// Creates a selection payload from an item state.
    pub fn from_item(item: &MenuItemState) -> Option<Self> {
        item.activation_enabled().then(|| Self {
            index: item.index,
            path: item.path.clone(),
            value: item.value.clone(),
            label: item.label.clone(),
            kind: item.kind,
            checked: item.checked,
        })
    }

    /// Returns the selected item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected item stable tree path.
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Returns the selected item stable tree path as a compact key.
    pub fn path_key(&self) -> String {
        self.path.join("/")
    }

    /// Returns the selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns selected item kind.
    pub const fn kind(&self) -> MenuItemKind {
        self.kind
    }

    /// Returns checked state at the time of activation for checkable rows.
    pub const fn checked(&self) -> bool {
        self.checked
    }
}

/// Pure submenu navigation target produced from a keyboard action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSubmenuNavigation {
    open_path: Vec<String>,
    focused_path: Vec<String>,
    focused_value: String,
}

impl MenuSubmenuNavigation {
    pub(crate) fn new(
        open_path: Vec<String>,
        focused_path: Vec<String>,
        focused_value: String,
    ) -> Self {
        Self {
            open_path,
            focused_path,
            focused_value,
        }
    }

    /// Returns the submenu branch path that should remain open.
    pub fn open_path(&self) -> &[String] {
        &self.open_path
    }

    /// Returns the submenu branch path as a compact key, or `None` when all branches close.
    pub fn open_path_key(&self) -> Option<String> {
        (!self.open_path.is_empty()).then(|| self.open_path.join("/"))
    }

    /// Returns the item path that should receive roving focus.
    pub fn focused_path(&self) -> &[String] {
        &self.focused_path
    }

    /// Returns the focused item path as a compact key.
    pub fn focused_path_key(&self) -> String {
        self.focused_path.join("/")
    }

    /// Returns the focused item value.
    pub fn focused_value(&self) -> &str {
        &self.focused_value
    }
}

/// Renderer-neutral keyboard intent for menu surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MenuKeyboardIntent {
    /// Escape should close the active submenu branch.
    DismissSubmenu(MenuSubmenuNavigation),
    /// Escape should dismiss the root menu surface.
    DismissRoot,
    /// Left or Right should move between submenu branches.
    NavigateSubmenu(MenuSubmenuNavigation),
    /// Roving focus should move to a visible menu item.
    FocusItem {
        focused_path: Vec<String>,
        focused_value: String,
    },
    /// Enter or Space should activate the focused menu item.
    Activate(MenuSelection),
}

/// Renderer-neutral surface plan for a submenu that may be rendered as a floating layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSubmenuSurface {
    trigger_bounds: Rect,
    content_bounds: Rect,
    placement_input: OverlayPlacementInput,
    hover_corridor: MenuSafeHoverCorridor,
}

impl MenuSubmenuSurface {
    /// Creates a submenu surface plan from resolved trigger bounds and content size.
    pub fn resolve(
        trigger_bounds: Rect,
        content_size: open_gpui_ui_core::OverlaySize,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
        offset: UiPx,
        safe_bounds: Option<Rect>,
    ) -> Self {
        let mut placement_input = OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(trigger_bounds),
            content_size,
        )
        .with_side(side)
        .with_alignment(alignment)
        .with_offset(offset);
        if let Some(safe_bounds) = safe_bounds {
            placement_input = placement_input.with_safe_bounds(safe_bounds);
        }

        let content_bounds =
            submenu_content_bounds(trigger_bounds, content_size, side, alignment, offset);
        let hover_corridor = MenuSafeHoverCorridor::between(trigger_bounds, content_bounds);

        Self {
            trigger_bounds,
            content_bounds,
            placement_input,
            hover_corridor,
        }
    }

    /// Returns bounds for the submenu trigger item.
    pub const fn trigger_bounds(self) -> Rect {
        self.trigger_bounds
    }

    /// Returns preferred bounds for the submenu content before renderer collision handling.
    pub const fn content_bounds(self) -> Rect {
        self.content_bounds
    }

    /// Returns renderer-neutral placement input for the submenu content.
    pub const fn placement_input(self) -> OverlayPlacementInput {
        self.placement_input
    }

    /// Returns the safe hover transition corridor between trigger and content.
    pub const fn hover_corridor(self) -> MenuSafeHoverCorridor {
        self.hover_corridor
    }
}

/// Renderer-neutral hover transition corridor between a submenu trigger and its floating surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuSafeHoverCorridor {
    bounds: Rect,
}

impl MenuSafeHoverCorridor {
    /// Creates the smallest axis-aligned corridor that connects trigger and submenu bounds.
    pub fn between(trigger_bounds: Rect, content_bounds: Rect) -> Self {
        Self {
            bounds: union_rect(trigger_bounds, content_bounds),
        }
    }

    /// Returns the corridor bounds.
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns whether a pointer position is inside the corridor.
    pub fn contains_point(self, point: open_gpui_ui_core::UiPoint) -> bool {
        rect_contains_point(self.bounds, point)
    }
}

fn submenu_content_bounds(
    trigger_bounds: Rect,
    content_size: open_gpui_ui_core::OverlaySize,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    offset: UiPx,
) -> Rect {
    let trigger_left = trigger_bounds.origin.x;
    let trigger_top = trigger_bounds.origin.y;
    let trigger_right = trigger_bounds.origin.x + trigger_bounds.size.width;
    let trigger_bottom = trigger_bounds.origin.y + trigger_bounds.size.height;
    let trigger_center_x = trigger_bounds.origin.x + trigger_bounds.size.width.half();
    let trigger_center_y = trigger_bounds.origin.y + trigger_bounds.size.height.half();

    let x = match side {
        OverlayPlacementSide::Right => trigger_right + offset,
        OverlayPlacementSide::Left => trigger_left - offset - content_size.width,
        OverlayPlacementSide::Top | OverlayPlacementSide::Bottom => match alignment {
            OverlayPlacementAlignment::Start => trigger_left,
            OverlayPlacementAlignment::Center => trigger_center_x - content_size.width.half(),
            OverlayPlacementAlignment::End => trigger_right - content_size.width,
        },
    };
    let y = match side {
        OverlayPlacementSide::Bottom => trigger_bottom + offset,
        OverlayPlacementSide::Top => trigger_top - offset - content_size.height,
        OverlayPlacementSide::Left | OverlayPlacementSide::Right => match alignment {
            OverlayPlacementAlignment::Start => trigger_top,
            OverlayPlacementAlignment::Center => trigger_center_y - content_size.height.half(),
            OverlayPlacementAlignment::End => trigger_bottom - content_size.height,
        },
    };

    open_gpui_ui_core::rect(ui_point(x, y), content_size)
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let left = a.origin.x.min(b.origin.x);
    let top = a.origin.y.min(b.origin.y);
    let right = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let bottom = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    open_gpui_ui_core::rect(ui_point(left, top), ui_size(right - left, bottom - top))
}

fn rect_contains_point(rect: Rect, point: open_gpui_ui_core::UiPoint) -> bool {
    let left = rect.origin.x.as_f32();
    let top = rect.origin.y.as_f32();
    let right = (rect.origin.x + rect.size.width).as_f32();
    let bottom = (rect.origin.y + rect.size.height).as_f32();
    let x = point.x.as_f32();
    let y = point.y.as_f32();

    x >= left && x <= right && y >= top && y <= bottom
}

fn menu_item_state_from_descriptor(
    index: usize,
    parent_value: Option<String>,
    path: Vec<String>,
    depth: usize,
    descriptor: &MenuItemDescriptor,
    focused_path: Option<&[String]>,
    open_path: &[String],
) -> MenuItemState {
    let child_parent = Some(descriptor.value.clone());
    let child_path_base = path.clone();
    let submenu_open = matches!(descriptor.kind, MenuItemKind::Submenu)
        && !descriptor.children.is_empty()
        && menu_path_is_open(&path, open_path);
    let children = descriptor
        .children
        .iter()
        .enumerate()
        .map(|(child_index, child)| {
            let mut child_path = child_path_base.clone();
            child_path.push(format!("{child_index}:{}", child.value));
            menu_item_state_from_descriptor(
                child_index,
                child_parent.clone(),
                child_path,
                depth.saturating_add(1),
                child,
                focused_path,
                open_path,
            )
        })
        .collect::<Vec<_>>();
    let child_count = children.len();
    let focused = focused_path.is_some_and(|focused_path| focused_path == path.as_slice());

    MenuItemState {
        index,
        parent_value,
        path,
        depth,
        value: descriptor.value.clone(),
        label: descriptor.label.clone(),
        kind: descriptor.kind,
        disabled: descriptor.disabled,
        checked: descriptor.checked,
        focused,
        submenu_open,
        child_count,
        children,
    }
}

fn menu_item_states_from_descriptors(
    descriptors: &[MenuItemDescriptor],
    focused_path: Option<&[String]>,
    open_path: &[String],
) -> Vec<MenuItemState> {
    descriptors
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let path = vec![format!("{index}:{}", item.value)];
            menu_item_state_from_descriptor(index, None, path, 0, item, focused_path, open_path)
        })
        .collect()
}

fn visible_menu_item_states(items: &[MenuItemState]) -> Vec<MenuItemState> {
    let mut visible = Vec::new();
    flatten_visible_menu_item_states(items, &mut visible);
    visible
}

fn flatten_visible_menu_item_states(items: &[MenuItemState], visible: &mut Vec<MenuItemState>) {
    for item in items {
        visible.push(item.clone());
        if item.submenu_open() {
            flatten_visible_menu_item_states(item.children(), visible);
        }
    }
}

fn first_focusable_menu_path(items: &[MenuItemState]) -> Option<Vec<String>> {
    visible_menu_item_states(items)
        .into_iter()
        .find(MenuItemState::focusable)
        .map(|item| item.path)
}

fn menu_path_for_value(items: &[MenuItemState], value: &str) -> Option<Vec<String>> {
    visible_menu_item_states(items)
        .into_iter()
        .find(|item| item.focusable() && item.value() == value)
        .map(|item| item.path)
}

fn menu_path_is_focusable(items: &[MenuItemState], path: &[String]) -> bool {
    visible_menu_item_states(items)
        .into_iter()
        .any(|item| item.path() == path && item.focusable())
}

fn menu_path_is_openable(items: &[MenuItemState], path: &[String]) -> bool {
    find_menu_item_state_by_path(items, path)
        .is_some_and(|item| item.has_submenu() && item.focusable())
}

fn menu_path_is_open(path: &[String], open_path: &[String]) -> bool {
    !path.is_empty() && open_path.len() >= path.len() && open_path.starts_with(path)
}

fn find_menu_item_state_by_path<'a>(
    items: &'a [MenuItemState],
    path: &[String],
) -> Option<&'a MenuItemState> {
    for item in items {
        if item.path() == path {
            return Some(item);
        }
        if let Some(child) = find_menu_item_state_by_path(item.children(), path) {
            return Some(child);
        }
    }
    None
}

fn first_focusable_child_path(item: &MenuItemState) -> Option<Vec<String>> {
    item.children()
        .iter()
        .find(|child| child.focusable())
        .map(|child| child.path().to_vec())
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
    visible_items: Vec<MenuItemState>,
    focused_index: Option<usize>,
    focused_path: Option<Vec<String>>,
    open_path: Vec<String>,
    metrics: MenuMetrics,
    colors: MenuColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
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
        Self::resolve_with_paths(
            size,
            disabled,
            open,
            default_open,
            focused_value,
            None,
            &[],
            items,
            placement_side,
            placement_alignment,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    /// Resolves menu state with adapter-owned submenu and focus paths applied.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_with_paths(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        focused_value: Option<&str>,
        focused_path: Option<&[String]>,
        open_path: &[String],
        items: impl IntoIterator<Item = MenuItemDescriptor>,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<MenuItemDescriptor> = items.into_iter().collect();
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::Menu)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .openable(!descriptors.is_empty())
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = menu_open_mode_from_disclosure(disclosure.open_mode());
        let mut open_path = if open { open_path.to_vec() } else { Vec::new() };
        let provisional_items = menu_item_states_from_descriptors(&descriptors, None, &open_path);
        if !menu_path_is_openable(&provisional_items, &open_path) {
            open_path.clear();
        }
        let provisional_items = if open_path.is_empty() {
            menu_item_states_from_descriptors(&descriptors, None, &[])
        } else {
            provisional_items
        };
        let focused_path = if open {
            focused_path
                .filter(|path| menu_path_is_focusable(&provisional_items, path))
                .map(|path| path.to_vec())
                .or_else(|| {
                    focused_value.and_then(|value| menu_path_for_value(&provisional_items, value))
                })
                .or_else(|| first_focusable_menu_path(&provisional_items))
        } else {
            None
        };
        let items =
            menu_item_states_from_descriptors(&descriptors, focused_path.as_deref(), &open_path);
        let visible_items = visible_menu_item_states(&items);
        let focused_index = focused_path.as_ref().and_then(|focused_path| {
            visible_items
                .iter()
                .position(|item| item.path() == focused_path.as_slice())
        });
        let overlay = disclosure.overlay().clone();
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
            visible_items,
            focused_index,
            focused_path,
            open_path,
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

    /// Returns visible menu rows after submenu expansion is applied.
    pub fn visible_items(&self) -> &[MenuItemState] {
        &self.visible_items
    }

    /// Returns focused visible item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns focused item stable path.
    pub fn focused_path(&self) -> Option<&[String]> {
        self.focused_path.as_deref()
    }

    /// Returns focused item stable path as a compact key.
    pub fn focused_path_key(&self) -> Option<String> {
        self.focused_path.as_ref().map(|path| path.join("/"))
    }

    /// Returns the deepest open submenu path.
    pub fn open_path(&self) -> &[String] {
        &self.open_path
    }

    /// Returns the deepest open submenu path as a compact key.
    pub fn open_path_key(&self) -> Option<String> {
        (!self.open_path.is_empty()).then(|| self.open_path.join("/"))
    }

    /// Resolves a default floating surface plan for a submenu trigger path.
    pub fn submenu_surface_for_trigger(
        &self,
        trigger_path: &[String],
        trigger_bounds: Rect,
        content_size: open_gpui_ui_core::OverlaySize,
        safe_bounds: Option<Rect>,
    ) -> Option<MenuSubmenuSurface> {
        let trigger = self
            .visible_items
            .iter()
            .find(|item| item.path() == trigger_path)?;
        if !self.open || !trigger.has_submenu() || !trigger.focusable() {
            return None;
        }

        Some(MenuSubmenuSurface::resolve(
            trigger_bounds,
            content_size,
            OverlayPlacementSide::Right,
            OverlayPlacementAlignment::Start,
            UiPx::ZERO,
            safe_bounds,
        ))
    }

    /// Returns focused item value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.visible_items.get(index))
            .map(MenuItemState::value)
    }

    /// Resolves a focus target for an APG-style menu navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<&MenuItemState> {
        let current = self.focused_index?;
        let disabled = self.disabled_map();
        menu_navigation_target(key, current, &disabled)
            .and_then(|index| self.visible_items.get(index))
    }

    /// Resolves a typeahead target for a caller-owned text buffer.
    pub fn typeahead_target(&self, query: &str) -> Option<&MenuItemState> {
        typeahead_target(
            self.visible_items.as_slice(),
            self.focused_index,
            query,
            MenuItemState::focusable,
            MenuItemState::label,
        )
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<MenuSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.focused_index
            .and_then(|index| self.visible_items.get(index))
            .and_then(MenuSelection::from_item)
    }

    /// Resolves submenu open/close targets for Right and Left keys.
    pub fn submenu_navigation_target(&self, key: &str) -> Option<MenuSubmenuNavigation> {
        let current = self
            .focused_index
            .and_then(|index| self.visible_items.get(index))?;

        match key {
            "right" if current.has_submenu() => {
                let focused_path = first_focusable_child_path(current)?;
                let focused_item = find_menu_item_state_by_path(&self.items, &focused_path)?;
                Some(MenuSubmenuNavigation::new(
                    current.path().to_vec(),
                    focused_path,
                    focused_item.value().to_owned(),
                ))
            }
            "left" => self.close_submenu_target(),
            _ => None,
        }
    }

    /// Resolves the renderer-neutral keyboard intent for a menu surface key.
    pub(crate) fn keyboard_intent_for_key(&self, key: &str) -> Option<MenuKeyboardIntent> {
        if key == "escape" {
            return Some(
                self.close_submenu_target()
                    .map(MenuKeyboardIntent::DismissSubmenu)
                    .unwrap_or(MenuKeyboardIntent::DismissRoot),
            );
        }

        if let Some(target) = self.submenu_navigation_target(key) {
            return Some(MenuKeyboardIntent::NavigateSubmenu(target));
        }

        if let Some(target) = self.navigation_target(key) {
            return Some(MenuKeyboardIntent::FocusItem {
                focused_path: target.path().to_vec(),
                focused_value: target.value().to_owned(),
            });
        }

        self.activation_for_key(key)
            .map(MenuKeyboardIntent::Activate)
    }

    /// Resolves the next branch/focus target when closing an active submenu branch.
    pub fn close_submenu_target(&self) -> Option<MenuSubmenuNavigation> {
        let current = self
            .focused_index
            .and_then(|index| self.visible_items.get(index))?;

        if current.depth() == 0 {
            return current.submenu_open().then(|| {
                MenuSubmenuNavigation::new(
                    Vec::new(),
                    current.path().to_vec(),
                    current.value().to_owned(),
                )
            });
        }

        let parent_path = current.path()[..current.path().len().saturating_sub(1)].to_vec();
        let parent = find_menu_item_state_by_path(&self.items, &parent_path)?;
        let next_open_path = parent_path[..parent_path.len().saturating_sub(1)].to_vec();
        Some(MenuSubmenuNavigation::new(
            next_open_path,
            parent_path,
            parent.value().to_owned(),
        ))
    }

    /// Returns whether the menu surface should use a local scroll viewport.
    pub fn scrollable_content(&self) -> bool {
        self.visible_items.len() > DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD
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

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.visible_items
            .iter()
            .map(|item| !item.focusable())
            .collect()
    }
}

/// Resolves a menu roving-focus target from an APG-style key name.
pub fn menu_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    vertical_roving_navigation_target(key, current, disabled)
}

fn menu_item_element(
    item: MenuItem,
    item_state: MenuItemState,
    debug_prefix: &'static str,
    debug_id: String,
    metrics: MenuMetrics,
    colors: MenuColors,
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> AnyElement {
    match item_state.kind() {
        MenuItemKind::Separator => div()
            .id(format!("{debug_prefix}-separator:{}", item_state.index()))
            .debug_selector({
                let separator_debug_id = debug_id.clone();
                let separator_index = item_state.index();
                move || format!("{debug_prefix}:{separator_debug_id}:separator:{separator_index}")
            })
            .h(gpui_px_from_ui(metrics.separator_height()))
            .my_1()
            .bg(ThemeResolver::resolve(colors.separator()))
            .into_any_element(),
        MenuItemKind::Action
        | MenuItemKind::Checkbox
        | MenuItemKind::Radio
        | MenuItemKind::Submenu => {
            let selection = MenuSelection::from_item(&item_state);
            let item_handler = item.on_select.clone();
            let global_handler = on_select.clone();
            let item_label = item_state.label().to_owned();
            let item_path_key = item_state.path_key();
            let left_padding = metrics.item_padding_x();
            let focused = item_state.focused();
            let disabled = item_state.disabled();
            let toggled = item_state.toggled();
            let has_submenu = item_state.has_submenu();
            let submenu_navigation = if has_submenu {
                item_state
                    .children()
                    .iter()
                    .find(|child| child.focusable())
                    .map(|child| {
                        MenuSubmenuNavigation::new(
                            item_state.path().to_vec(),
                            child.path().to_vec(),
                            child.value().to_owned(),
                        )
                    })
            } else {
                None
            };
            let hover_focusable = item_state.focusable();
            let hover_path = item_state.path().to_vec();
            let hover_value = item_state.value().to_owned();
            let hover_submenu_navigation = submenu_navigation.clone();
            let hover_runtime = runtime.clone();

            div()
                .id(format!("{debug_prefix}-item:{item_path_key}"))
                .debug_selector({
                    let item_debug_id = debug_id.clone();
                    move || format!("{debug_prefix}:{item_debug_id}:item:{item_path_key}")
                })
                .min_h(gpui_px_from_ui(metrics.item_height()))
                .pl(gpui_px_from_ui(left_padding))
                .pr(gpui_px_from_ui(metrics.item_padding_x()))
                .py(gpui_px_from_ui(metrics.item_padding_y()))
                .flex()
                .items_center()
                .justify_between()
                .rounded(gpui_px_from_ui(metrics.radius()))
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
                .ui_role(Role::MenuItem)
                .aria_label(item_label.clone())
                .aria_disabled(disabled)
                .when_some(toggled, |this, toggled| this.ui_aria_toggled(toggled))
                .when(has_submenu, |this| {
                    this.aria_expanded(item_state.submenu_open())
                })
                .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                .when(!disabled, |this| {
                    this.cursor_pointer()
                        .hover(move |style| {
                            style.bg(ThemeResolver::resolve(colors.item_hover_background()))
                        })
                        .on_hover(move |hovered, window, cx| {
                            if hover_focusable {
                                update_menu_hover_target(
                                    hover_runtime.clone(),
                                    hover_path.clone(),
                                    hover_value.clone(),
                                    hover_submenu_navigation.clone(),
                                    *hovered,
                                    window,
                                    cx,
                                );
                            }
                        })
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            if let Some(submenu_navigation) = submenu_navigation.clone() {
                                runtime.update(cx, |runtime, _| {
                                    runtime.open_path = submenu_navigation.open_path().to_vec();
                                    runtime.focused_path =
                                        Some(submenu_navigation.focused_path().to_vec());
                                    runtime.focused_value =
                                        Some(submenu_navigation.focused_value().to_owned());
                                });
                                return;
                            }
                            let Some(selection) = selection.clone() else {
                                return;
                            };
                            if let Some(item_handler) = item_handler.as_ref() {
                                item_handler(selection.clone(), window, cx);
                            }
                            if let Some(global_handler) = global_handler.as_ref() {
                                global_handler(selection, window, cx);
                            }
                            close_menu(
                                runtime.clone(),
                                trigger_focus.clone(),
                                focus_restore.clone(),
                                on_open_change.clone(),
                                window,
                                cx,
                            );
                        })
                })
                .child(item_label)
                .when_some(toggled, |this, toggled| {
                    let marker = if toggled == Toggled::True {
                        "checked"
                    } else {
                        ""
                    };
                    this.child(div().ml_2().child(marker))
                })
                .when(has_submenu, |this| this.child(div().ml_2().child(">")))
                .into_any_element()
        }
    }
}

/// A concrete GPUI menu item.
#[derive(Clone)]
pub struct MenuItem {
    descriptor: MenuItemDescriptor,
    children: Vec<MenuItem>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
}

impl MenuItem {
    /// Creates a menu item from a pure descriptor.
    pub fn from_descriptor(descriptor: MenuItemDescriptor) -> Self {
        let children = descriptor
            .children_ref()
            .iter()
            .cloned()
            .map(MenuItem::from_descriptor)
            .collect();
        Self {
            descriptor,
            children,
            on_select: None,
        }
    }

    /// Creates an action menu item.
    pub fn action(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::action(value, label.to_string()),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a checkbox menu item.
    pub fn checkbox(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::checkbox(value, label.to_string(), checked),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a radio-style menu item.
    pub fn radio(value: impl Into<String>, label: impl Into<SharedString>, checked: bool) -> Self {
        let label = label.into();
        Self {
            descriptor: MenuItemDescriptor::radio(value, label.to_string(), checked),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a separator item.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: MenuItemDescriptor::separator(value),
            children: Vec::new(),
            on_select: None,
        }
    }

    /// Creates a submenu trigger item.
    pub fn submenu(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        children: impl IntoIterator<Item = MenuItem>,
    ) -> Self {
        let label = label.into();
        let children: Vec<MenuItem> = children.into_iter().collect();
        Self {
            descriptor: MenuItemDescriptor::submenu(
                value,
                label.to_string(),
                children.iter().map(MenuItem::descriptor),
            ),
            children,
            on_select: None,
        }
    }

    /// Marks the menu item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Applies caller-owned checked state to checkbox and radio items.
    pub fn checked(mut self, checked: bool) -> Self {
        self.descriptor = self.descriptor.checked(checked);
        self
    }

    /// Adds one submenu child.
    pub fn child(mut self, child: MenuItem) -> Self {
        if self.descriptor.kind() == MenuItemKind::Submenu {
            let child_descriptor = child.descriptor();
            self.children.push(child.clone());
            self.descriptor = self.descriptor.child(child_descriptor);
        }
        self
    }

    /// Adds many submenu children.
    pub fn children(mut self, children: impl IntoIterator<Item = MenuItem>) -> Self {
        if self.descriptor.kind() == MenuItemKind::Submenu {
            let children: Vec<MenuItem> = children.into_iter().collect();
            self.descriptor = self
                .descriptor
                .children(children.iter().map(MenuItem::descriptor));
            self.children.extend(children);
        }
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
        if self.descriptor.kind == MenuItemKind::Submenu {
            return MenuItemDescriptor::submenu(
                self.descriptor.value(),
                self.descriptor.label(),
                self.children.iter().map(MenuItem::descriptor),
            )
            .disabled(self.descriptor.disabled_state());
        }

        self.descriptor.clone()
    }

    pub(crate) fn select_handler(
        &self,
    ) -> Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>> {
        self.on_select.clone()
    }

    pub(crate) fn child_items(&self) -> &[MenuItem] {
        &self.children
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
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
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

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused_value(mut self, value: impl Into<String>) -> Self {
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
        let trigger_focus = cx.focus_handle();
        let content_focus = cx.focus_handle();
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            MenuRuntime::new(
                self.default_open,
                trigger_focus.clone(),
                content_focus.clone(),
                self.focused_value.clone(),
            )
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.sync_controlled_open(resolved_open);
            });
        }

        let focused_value = runtime_state.resolved_focused_value(self.focused_value.as_deref());
        let state = MenuState::resolve_with_paths(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            focused_value,
            runtime_state.focused_path.as_deref(),
            &runtime_state.open_path,
            descriptors.clone(),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let runtime_state = runtime.read(cx).clone();
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_label = self.trigger_label;
        let items = self.items;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let focus_restore = state.focus_restore_intent().clone();
        let initial_focus = state.initial_focus_intent().clone();
        let trigger_focus = runtime_state.trigger_focus.clone();
        let content_focus = runtime_state.content_focus.clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let trigger_focus_for_escape = trigger_focus.clone();
        let focus_restore_for_escape = focus_restore.clone();
        let trigger_focus_for_content = trigger_focus.clone();
        let focus_restore_for_content = focus_restore.clone();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                open_gpui_ui_core::OverlayAnchorInput::from_layout_bounds(open_gpui_ui_core::rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(metrics.min_width(), metrics.trigger_height()),
                )),
                ui_size(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );

        if open && !runtime_state.did_initial_focus {
            runtime.update(cx, |runtime, _| {
                runtime.did_initial_focus = true;
            });
            if let Some(focus) = menu_initial_focus_handle(&runtime, &initial_focus, cx) {
                window.defer(cx, move |window, cx| focus.focus(window, cx));
            }
        }

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("menu:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(trigger_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("menu:{debug_id}:trigger")
                    })
                    .min_h(gpui_px_from_ui(metrics.trigger_height()))
                    .px(gpui_px_from_ui(metrics.trigger_padding_x()))
                    .py(gpui_px_from_ui(metrics.trigger_padding_y()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.trigger_border()))
                    .bg(ThemeResolver::resolve(colors.trigger_background()))
                    .text_color(ThemeResolver::resolve(colors.trigger_foreground()))
                    .text_size(gpui_px_from_ui(metrics.text_size()))
                    .line_height(gpui_px_from_ui(metrics.text_size()))
                    .focusable()
                    .tab_stop(!disabled)
                    .ui_role(state.trigger_role())
                    .aria_label(trigger_label.clone())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .track_focus(&trigger_focus)
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let trigger_focus = trigger_focus_for_escape.clone();
                        let focus_restore = focus_restore_for_escape.clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape" {
                                consume_overlay_event(window, cx);
                                close_menu(
                                    runtime.clone(),
                                    trigger_focus.clone(),
                                    focus_restore.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            }
                        })
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    set_overlay_open(&mut runtime.open, next_open);
                                    if !next_open {
                                        runtime.reset_closed_state();
                                    }
                                });
                                emit_overlay_open_change(
                                    next_open,
                                    on_open_change.as_deref(),
                                    window,
                                    cx,
                                );
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
                                debug_id.clone(),
                                state.clone(),
                                runtime.clone(),
                                trigger_focus_for_content.clone(),
                                content_focus.clone(),
                                scroll_handle.clone(),
                                focus_restore_for_content.clone(),
                                on_open_change.clone(),
                                on_select.clone(),
                                cx,
                                overlay_adapter.snap_margin(),
                                overlay_adapter.deferred_priority(),
                            )),
                    )
                    .priority(overlay_adapter.deferred_priority()),
                )
            })
    }
}

fn menu_content_element(
    items: Vec<MenuItem>,
    content_id: ElementId,
    debug_id: String,
    state: MenuState,
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
    scroll_handle: ScrollHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let key_state = state.clone();
    let key_runtime = runtime.clone();
    let key_open_change = on_open_change.clone();
    let key_select = on_select.clone();
    let trigger_focus_for_keydown = trigger_focus.clone();
    let focus_restore_for_keydown = focus_restore.clone();
    let trigger_focus_for_outside = trigger_focus.clone();
    let focus_restore_for_outside = focus_restore.clone();
    let key_items = visible_menu_items(&items, state.open_path());
    let root_branch = menu_branch_surface(
        &items,
        &state,
        &[],
        None,
        debug_id.clone(),
        runtime.clone(),
        trigger_focus.clone(),
        focus_restore.clone(),
        on_open_change.clone(),
        on_select.clone(),
        Some(scroll_handle),
        cx,
        snap_margin,
        deferred_priority,
    );

    div()
        .id(content_id)
        .debug_selector({
            let content_debug_id = debug_id.clone();
            move || format!("menu:{content_debug_id}:content")
        })
        .focusable()
        .relative()
        .tab_group()
        .track_focus(&content_focus)
        .ui_role(state.content_role())
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let Some(intent) = key_state.keyboard_intent_for_key(event.keystroke.key.as_str())
            else {
                return;
            };

            match intent {
                MenuKeyboardIntent::DismissSubmenu(target) => {
                    consume_overlay_event(window, cx);
                    key_runtime.update(cx, |runtime, _| {
                        runtime.apply_submenu_target(&target);
                    });
                }
                MenuKeyboardIntent::DismissRoot => {
                    consume_overlay_event(window, cx);
                    close_menu(
                        key_runtime.clone(),
                        trigger_focus_for_keydown.clone(),
                        focus_restore_for_keydown.clone(),
                        key_open_change.clone(),
                        window,
                        cx,
                    );
                }
                MenuKeyboardIntent::NavigateSubmenu(target) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.apply_submenu_target(&target);
                    });
                }
                MenuKeyboardIntent::FocusItem {
                    focused_path,
                    focused_value,
                } => {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.focused_value = Some(focused_value);
                        runtime.focused_path = Some(focused_path);
                    });
                }
                MenuKeyboardIntent::Activate(selection) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    if let Some(item_handler) = key_items
                        .iter()
                        .zip(key_state.visible_items())
                        .find(|(_, item_state)| item_state.path() == selection.path())
                        .and_then(|(item, _)| item.select_handler())
                        .as_ref()
                    {
                        item_handler(selection.clone(), window, cx);
                    }
                    if let Some(on_select) = key_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                    close_menu(
                        key_runtime.clone(),
                        trigger_focus_for_keydown.clone(),
                        focus_restore_for_keydown.clone(),
                        key_open_change.clone(),
                        window,
                        cx,
                    );
                }
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_menu(
                    runtime.clone(),
                    trigger_focus_for_outside.clone(),
                    focus_restore_for_outside.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .child(root_branch)
}

fn menu_branch_surface(
    items: &[MenuItem],
    state: &MenuState,
    branch_path: &[String],
    surface_id: Option<ElementId>,
    debug_id: String,
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    scroll_handle: Option<ScrollHandle>,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> AnyElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let branch_key = if branch_path.is_empty() {
        "root".to_string()
    } else {
        branch_path.join("/")
    };
    let Some((branch_items, branch_states)) =
        menu_branch_items_and_states(items, state.items(), branch_path)
    else {
        return div().into_any_element();
    };
    let scroll_handle = match (branch_path.is_empty(), scroll_handle) {
        (true, Some(scroll_handle)) => scroll_handle,
        (true, None) => ScrollHandle::new(),
        (false, _) => runtime.update(cx, |runtime, _| runtime.submenu_scroll_handle(&branch_key)),
    };
    let branch_scrollable_content =
        branch_states.len() > DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD;
    let scroll_viewport_id = if branch_path.is_empty() {
        format!("menu:{debug_id}:content-scroll")
    } else {
        format!("menu:{debug_id}:submenu:{branch_key}:scroll")
    };
    let row_path_keys: Vec<String> = branch_states.iter().map(MenuItemState::path_key).collect();
    let rows = div()
        .flex()
        .flex_col()
        .gap_1()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let row_path_keys = row_path_keys.clone();
            move |row_bounds, _window, cx| {
                runtime.update(cx, |runtime, _| {
                    for (path_key, bounds) in row_path_keys.iter().zip(row_bounds.into_iter()) {
                        runtime
                            .submenu_trigger_bounds
                            .insert(path_key.clone(), menu_bounds_to_rect(bounds));
                    }
                });
            }
        })
        .children(menu_item_elements(
            branch_items,
            branch_states.clone(),
            debug_id.clone(),
            metrics,
            colors,
            runtime.clone(),
            trigger_focus.clone(),
            focus_restore.clone(),
            on_open_change.clone(),
            on_select.clone(),
        ));
    let submenu_layer = menu_submenu_layer(
        items,
        state,
        &branch_states,
        debug_id.clone(),
        runtime.clone(),
        trigger_focus.clone(),
        focus_restore.clone(),
        on_open_change.clone(),
        on_select.clone(),
        cx,
        snap_margin,
        deferred_priority,
    );
    let surface_id =
        surface_id.unwrap_or_else(|| format!("menu:{debug_id}:panel:{branch_key}").into());
    let shell = div()
        .id(surface_id)
        .debug_selector({
            let branch_debug_id = debug_id.clone();
            let branch_key = branch_key.clone();
            move || format!("menu:{branch_debug_id}:panel:{branch_key}")
        })
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .when(branch_scrollable_content, |this| {
            this.h(gpui_px_from_ui(metrics.max_height()))
        })
        .when(!branch_scrollable_content, |this| {
            this.max_h(gpui_px_from_ui(metrics.max_height()))
        })
        .p(gpui_px_from_ui(metrics.surface_padding()))
        .flex()
        .flex_col()
        .gap_1()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.surface()))
        .shadow_lg()
        .occlude()
        .overflow_hidden()
        .when(!branch_path.is_empty(), |this| {
            let runtime = runtime.clone();
            this.on_hover(move |hovered, window, cx| {
                handle_menu_submenu_surface_hover(runtime.clone(), *hovered, window, cx);
            })
        })
        .child(
            ScrollArea::new(scroll_viewport_id, rows)
                .vertical()
                .preserve_scroll()
                .scroll_handle(&scroll_handle)
                .with_size(state.size()),
        );

    div()
        .relative()
        .child(shell)
        .when_some(submenu_layer, |this, submenu_layer| {
            this.child(submenu_layer)
        })
        .into_any_element()
}

fn menu_submenu_layer(
    items: &[MenuItem],
    state: &MenuState,
    branch_states: &[MenuItemState],
    debug_id: String,
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
    cx: &mut App,
    snap_margin: open_gpui::Pixels,
    deferred_priority: usize,
) -> Option<AnyElement> {
    let open_child = branch_states
        .iter()
        .find(|item| item.submenu_open() && item.has_submenu())?;
    let trigger_bounds = runtime
        .read(cx)
        .submenu_trigger_bounds
        .get(&open_child.path_key())
        .copied()?;
    let placement = GpuiOverlayPlacement::resolve(
        OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(trigger_bounds),
            ui_size(
                state.metrics().min_width(),
                state.metrics().trigger_height(),
            ),
        )
        .with_side(OverlayPlacementSide::Right)
        .with_alignment(OverlayPlacementAlignment::Start)
        .with_offset(UiPx::ZERO),
        snap_margin,
    );
    let child_branch_path = open_child.path().to_vec();
    let submenu_surface = menu_branch_surface(
        items,
        state,
        &child_branch_path,
        None,
        debug_id,
        runtime,
        trigger_focus,
        focus_restore,
        on_open_change,
        on_select,
        None,
        cx,
        snap_margin,
        deferred_priority,
    );

    Some(
        deferred(
            anchored()
                .position(placement.position().unwrap_or_default())
                .anchor(placement.anchor())
                .offset(placement.offset())
                .snap_to_window_with_margin(placement.snap_margin())
                .child(submenu_surface),
        )
        .priority(deferred_priority)
        .into_any_element(),
    )
}

fn menu_item_elements(
    items: Vec<MenuItem>,
    states: Vec<MenuItemState>,
    debug_id: String,
    metrics: MenuMetrics,
    colors: MenuColors,
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_select: Option<Rc<dyn Fn(MenuSelection, &mut Window, &mut App)>>,
) -> Vec<AnyElement> {
    items
        .into_iter()
        .zip(states)
        .map(|(item, item_state)| {
            menu_item_element(
                item,
                item_state,
                "menu",
                debug_id.clone(),
                metrics,
                colors,
                runtime.clone(),
                trigger_focus.clone(),
                focus_restore.clone(),
                on_open_change.clone(),
                on_select.clone(),
            )
        })
        .collect()
}

fn menu_branch_items_and_states(
    items: &[MenuItem],
    states: &[MenuItemState],
    branch_path: &[String],
) -> Option<(Vec<MenuItem>, Vec<MenuItemState>)> {
    if branch_path.is_empty() {
        return Some((items.to_vec(), states.to_vec()));
    }

    let branch_item = find_menu_item_by_path(items, branch_path)?;
    let branch_state = find_menu_item_state_by_path(states, branch_path)?;

    Some((
        branch_item.child_items().to_vec(),
        branch_state.children().to_vec(),
    ))
}

fn find_menu_item_by_path<'a>(items: &'a [MenuItem], path: &[String]) -> Option<&'a MenuItem> {
    let mut current = items;
    let mut resolved = None;

    for segment in path {
        let index = segment.split_once(':')?.0.parse::<usize>().ok()?;
        let item = current.get(index)?;
        resolved = Some(item);
        current = item.child_items();
    }

    resolved
}

fn menu_bounds_to_rect(bounds: open_gpui::Bounds<open_gpui::Pixels>) -> Rect {
    open_gpui_ui_core::rect(
        ui_point_from_gpui(bounds.origin),
        ui_size_from_gpui_size(bounds.size),
    )
}

/// Returns concrete menu items that are visible for the current submenu path.
pub(crate) fn visible_menu_items(items: &[MenuItem], open_path: &[String]) -> Vec<MenuItem> {
    let mut visible = Vec::new();
    let mut parent_path = Vec::new();
    flatten_visible_menu_items(items, &mut parent_path, open_path, &mut visible);
    visible
}

fn flatten_visible_menu_items(
    items: &[MenuItem],
    parent_path: &mut Vec<String>,
    open_path: &[String],
    visible: &mut Vec<MenuItem>,
) {
    for (index, item) in items.iter().enumerate() {
        parent_path.push(format!("{index}:{}", item.descriptor.value()));
        visible.push(item.clone());
        if item.descriptor.kind() == MenuItemKind::Submenu
            && !item.child_items().is_empty()
            && menu_path_is_open(parent_path, open_path)
        {
            flatten_visible_menu_items(item.child_items(), parent_path, open_path, visible);
        }
        parent_path.pop();
    }
}

fn close_menu(
    runtime: open_gpui::Entity<MenuRuntime>,
    trigger_focus: FocusHandle,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        set_overlay_open(&mut runtime.open, false);
        runtime.reset_closed_state();
    });
    emit_overlay_open_change(false, on_open_change.as_deref(), window, cx);
    restore_overlay_focus(&focus_restore, Some(trigger_focus), true, window, cx);
}

fn menu_initial_focus_handle(
    runtime: &open_gpui::Entity<MenuRuntime>,
    intent: &InitialFocusIntent,
    cx: &App,
) -> Option<FocusHandle> {
    match intent {
        InitialFocusIntent::None => None,
        InitialFocusIntent::FirstFocusable => Some(runtime.read(cx).content_focus.clone()),
        InitialFocusIntent::Target(_) => None,
        InitialFocusIntent::TargetOrFirstFocusable(_) => {
            Some(runtime.read(cx).content_focus.clone())
        }
    }
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
