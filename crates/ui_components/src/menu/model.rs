use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPlacementAlignment, OverlayPlacementSide, Rect, Role, Size, ThemeTokens, Toggled, UiPx,
};

use crate::focus::FocusRing;
use crate::overlay::{OverlayDisclosureConfig, OverlayResolvedState};
use crate::roving_focus::{typeahead_target, vertical_roving_navigation_target};
use crate::theme::ThemeResolver;

use super::{
    DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD, MenuColors, MenuItemDescriptor, MenuItemKind,
    MenuMetrics, MenuOpenMode, MenuSubmenuSurface, menu_open_mode_from_disclosure,
};
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
    shortcut: Option<String>,
    when: Option<String>,
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

    /// Returns the display shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.when.as_deref()
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

fn menu_item_state_from_descriptor(
    index: usize,
    parent_value: Option<String>,
    path: Vec<String>,
    depth: usize,
    descriptor: &MenuItemDescriptor,
    focused_path: Option<&[String]>,
    open_path: &[String],
) -> MenuItemState {
    let child_parent = Some(descriptor.value().to_owned());
    let child_path_base = path.clone();
    let submenu_open = matches!(descriptor.kind(), MenuItemKind::Submenu)
        && !descriptor.children_ref().is_empty()
        && menu_path_is_open(&path, open_path);
    let children = descriptor
        .children_ref()
        .iter()
        .enumerate()
        .map(|(child_index, child)| {
            let mut child_path = child_path_base.clone();
            child_path.push(format!("{child_index}:{}", child.value()));
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
        value: descriptor.value().to_owned(),
        label: descriptor.label().to_owned(),
        kind: descriptor.kind(),
        disabled: descriptor.disabled_state(),
        checked: descriptor.checked_state(),
        shortcut: descriptor.shortcut_ref().map(str::to_owned),
        when: descriptor.when_ref().map(str::to_owned),
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
            let path = vec![format!("{index}:{}", item.value())];
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

pub(crate) fn menu_path_is_open(path: &[String], open_path: &[String]) -> bool {
    !path.is_empty() && open_path.len() >= path.len() && open_path.starts_with(path)
}

pub(crate) fn find_menu_item_state_by_path<'a>(
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
