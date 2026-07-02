//! Menu component and shared menu state.

mod descriptor;
mod model;
mod render_plan;
pub(crate) mod runtime;
mod style;

use crate::geometry::gpui_px_from_ui;
use crate::geometry::{ui_point_from_gpui, ui_size_from_gpui_size};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayAnchorInput, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Rect, Role, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::focus::focus_ring_shadow;

use crate::overlay::{
    GpuiOverlayPlacement, GpuiOverlayState, consume_overlay_event, emit_overlay_open_change,
    gpui_overlay_state, gpui_positioned_overlay_layer, gpui_relative_overlay_layer,
    outside_press_open_change, resolve_overlay_open_state, restore_overlay_focus, set_overlay_open,
};
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;
use runtime::{MenuRuntime, handle_menu_submenu_surface_hover, update_menu_hover_target};

pub(crate) use descriptor::menu_open_mode_from_disclosure;
pub use descriptor::{MenuItemDescriptor, MenuItemKind, MenuOpenMode};
pub use model::{
    MenuItemState, MenuSelection, MenuState, MenuSubmenuNavigation, menu_navigation_target,
};
pub(crate) use model::{MenuKeyboardIntent, find_menu_item_state_by_path, menu_path_is_open};
pub use render_plan::{MenuSafeHoverCorridor, MenuSubmenuSurface};
pub use style::{MenuColors, MenuMetrics};

/// Default threshold where menu surfaces become locally scrollable.
pub const DEFAULT_SCROLLABLE_MENU_ITEM_COUNT_THRESHOLD: usize = 8;

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
        if self.descriptor.kind() == MenuItemKind::Submenu {
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
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    menu_content_element(
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
                    ),
                ))
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

    Some(gpui_positioned_overlay_layer(
        &GpuiOverlayState::resolve(
            state.overlay().policy().clone(),
            deferred_priority,
            snap_margin,
        ),
        &placement,
        placement.position().unwrap_or_default(),
        submenu_surface,
    ))
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
